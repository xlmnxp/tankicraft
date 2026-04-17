/// Capture-the-Flag game mode.
///
/// Two teams (Red / Blue) each have a 3-part flag (stand + pole + cloth) at a
/// base corner of the arena.  Drive over the enemy flag to pick it up, then
/// return to your own base corner to score.

use crate::components::{Dead, Flag, FlagCarrier, Tank, Team, TeamScores};
use crate::config::{
    BLUE_BASE, FLAG_CAPTURE_RADIUS, FLAG_DROP_PICKUP_COOLDOWN_SECS, FLAG_PICKUP_RADIUS, RED_BASE,
    SCORE_BROADCAST_TICKS,
};
use std::borrow::Cow;
use std::time::{Duration, Instant};
use valence::entity::block_display::{
    BlockDisplayEntityBundle, BlockState as BlockDisplayBlockState,
};
use valence::entity::display::{Scale, Translation, ViewRange};
use valence::entity::entity::Flags as EntityFlags;
use valence::entity::EntityLayerId;
use valence::math::{DVec3, Vec3};
use valence::prelude::*;
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::GameMessageS2c;
use valence::protocol::sound::{Sound, SoundCategory};
use valence::text::IntoText;

// ─── Flag geometry ────────────────────────────────────────────────────────────
//
//   stand  : 0.4×0.08×0.4 slab centred at (0, 0.04, 0)
//   pole   : 0.06×2.0×0.06 column centred at (0, 1.0, 0)
//   cloth  : 0.7×0.45×0.04 panel; left edge flush with pole right face,
//            top edge flush with pole top → centre (0.38, 1.775, 0)
//
// Translation = centre − scale/2  (display-entity corner offset from root)

const FLAG_PARTS: &[([f32; 3], [f32; 3])] = &[
    ([0.00, 0.04, 0.00], [0.40, 0.08, 0.40]),   // 0 stand
    ([0.00, 1.00, 0.00], [0.06, 2.00, 0.06]),   // 1 pole
    ([0.38, 1.775, 0.00], [0.70, 0.45, 0.04]),  // 2 cloth
];

fn corner(c: [f32; 3], s: [f32; 3]) -> Vec3 {
    Vec3::new(c[0] - s[0] * 0.5, c[1] - s[1] * 0.5, c[2] - s[2] * 0.5)
}

fn cloth_block(team: Team) -> BlockState {
    match team {
        Team::Red => BlockState::RED_WOOL,
        Team::Blue => BlockState::BLUE_WOOL,
    }
}

fn part_block(i: usize, team: Team) -> BlockState {
    match i {
        0 => BlockState::SMOOTH_STONE,
        1 => BlockState::IRON_BLOCK,
        _ => cloth_block(team),
    }
}

// ─── Startup ──────────────────────────────────────────────────────────────────

pub fn setup_flags(
    mut commands: Commands,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    let Ok(layer) = layers.get_single() else {
        return;
    };
    spawn_flag(&mut commands, layer, Team::Red, DVec3::from(RED_BASE));
    spawn_flag(&mut commands, layer, Team::Blue, DVec3::from(BLUE_BASE));
}

fn spawn_flag(commands: &mut Commands, layer: Entity, team: Team, base_pos: DVec3) {
    let mut part_entities = Vec::with_capacity(FLAG_PARTS.len());
    for (i, &(centre, scale)) in FLAG_PARTS.iter().enumerate() {
        let id = commands
            .spawn(BlockDisplayEntityBundle {
                position: Position(base_pos),
                layer: EntityLayerId(layer),
                block_display_block_state: BlockDisplayBlockState(part_block(i, team)),
                display_scale: Scale(Vec3::from(scale)),
                display_translation: Translation(corner(centre, scale)),
                display_view_range: ViewRange(64.0),
                ..Default::default()
            })
            .id();
        part_entities.push(id);
    }

    // Logical flag entity (no Position component – root tracked in Flag::base_pos)
    commands.spawn(Flag {
        team,
        base_pos,
        carrier: None,
        part_entities,
        dropped_pos: None,
        dropped_at: None,
        dropped_by: None,
    });
}

// ─── Per-tick update ──────────────────────────────────────────────────────────

pub fn update_ctf(
    mut commands: Commands,
    mut players: Query<
        (Entity, &Position, &Team, Option<&FlagCarrier>, &mut Tank, &mut Client),
        Without<Dead>,
    >,
    carrier_pos: Query<&Position, (With<Tank>, Without<Dead>)>,
    mut flags: Query<(Entity, &mut Flag)>,
    mut part_positions: Query<&mut Position, Without<Tank>>,
    mut scores: ResMut<TeamScores>,
) {
    // ── Return dropped flags to base after 15 seconds ─────────────────────────
    for (_, mut flag) in &mut flags {
        if flag.carrier.is_none() {
            if let (Some(dropped_at), Some(_)) = (flag.dropped_at, flag.dropped_pos) {
                if dropped_at.elapsed() >= Flag::RETURN_DELAY {
                    flag.dropped_pos = None;
                    flag.dropped_at = None;
                    flag.dropped_by = None;
                }
            }
        }
    }

    // ── Snapshot: (flag_e, team, carrier, root, dropped_pos, dropped_by, dropped_at) ─
    let snap: Vec<(Entity, Team, Option<Entity>, DVec3, Option<DVec3>, Option<Entity>, Option<Instant>)> = flags
        .iter()
        .map(|(flag_e, flag)| {
            let root = flag
                .carrier
                .and_then(|e| carrier_pos.get(e).ok())
                .map(|p| p.0)
                .unwrap_or_else(|| flag.dropped_pos.unwrap_or(flag.base_pos));
            (flag_e, flag.team, flag.carrier, root, flag.dropped_pos, flag.dropped_by, flag.dropped_at)
        })
        .collect();

    // Which flags are currently in flight?
    let red_flag_carried = snap.iter().any(|(_, t, c, _, _, _, _)| *t == Team::Red && c.is_some());
    let blue_flag_carried = snap.iter().any(|(_, t, c, _, _, _, _)| *t == Team::Blue && c.is_some());

    // ── Move flag visuals ─────────────────────────────────────────────────────
    for &(flag_e, _, _, root, _, _, _) in &snap {
        if let Ok((_, flag)) = flags.get(flag_e) {
            for &part in &flag.part_entities {
                if let Ok(mut p) = part_positions.get_mut(part) {
                    p.0 = root;
                }
            }
        }
    }

    // ── Collect player actions ────────────────────────────────────────────────
    let mut pickups: Vec<(Entity, Entity, Team, DVec3)> = vec![]; // (player, flag_e, flag_team, player_pos)
    let mut captures: Vec<(Entity, Entity)> = vec![];             // (player, flag_e)
    let mut returns: Vec<Entity> = vec![];                        // flag_e to return to base

    for (player_e, player_pos, &player_team, carrier_opt, mut tank, mut client) in &mut players {
        // Score bar (throttled, with flashing flag indicators)
        tank.score_tick += 1;
        if tank.score_tick >= SCORE_BROADCAST_TICKS {
            tank.score_tick = 0;
            tank.score_flash = !tank.score_flash;

            let flash_on = tank.score_flash;
            let red_flag_str = flag_indicator(red_flag_carried, flash_on);
            let blue_flag_str = flag_indicator(blue_flag_carried, flash_on);

            client.write_packet(&GameMessageS2c {
                chat: Cow::Owned(
                    format!(
                        "§c{} Red {}  §f|  §9Blue {} §9{}",
                        red_flag_str, scores.red, scores.blue, blue_flag_str
                    )
                    .into_text(),
                ),
                overlay: true,
            });
        }

        if let Some(fc) = carrier_opt {
            // Check capture at home base
            let home = if player_team == Team::Red {
                DVec3::from(RED_BASE)
            } else {
                DVec3::from(BLUE_BASE)
            };
            if (player_pos.0 - home).length() < FLAG_CAPTURE_RADIUS {
                captures.push((player_e, fc.flag_entity));
                match player_team {
                    Team::Red => scores.red += 1,
                    Team::Blue => scores.blue += 1,
                }
                // Capture sound + message
                client.play_sound(
                    Sound::EntityPlayerLevelup,
                    SoundCategory::Master,
                    player_pos.0,
                    1.0,
                    1.0,
                );
                client.send_chat_message(format!(
                    "§a§l★ CAPTURED!  §r§7Red {} : Blue {}",
                    scores.red, scores.blue
                ));
            }
        } else {
            for &(flag_e, flag_team, flag_carrier, flag_root, flag_dropped_pos, flag_dropped_by, flag_dropped_at) in &snap {
                if flag_carrier.is_some() {
                    continue;
                }
                if (player_pos.0 - flag_root).length() >= FLAG_PICKUP_RADIUS {
                    continue;
                }

                if flag_team != player_team {
                    // Dropper cooldown — same player cannot immediately re-pick their own drop
                    let in_cooldown = flag_dropped_by == Some(player_e)
                        && flag_dropped_at.map_or(false, |t| {
                            t.elapsed() < Duration::from_secs(FLAG_DROP_PICKUP_COOLDOWN_SECS)
                        });
                    if in_cooldown {
                        break;
                    }

                    // Enemy flag — pick it up
                    pickups.push((player_e, flag_e, flag_team, player_pos.0));
                    client.play_sound(
                        Sound::EntityExperienceOrbPickup,
                        SoundCategory::Master,
                        player_pos.0,
                        1.0,
                        0.8,
                    );
                    client.send_chat_message("§e§l⚑ Flag picked up! Return to your base!");
                } else if flag_dropped_pos.is_some() {
                    // Own team's dropped flag — return it to base
                    returns.push(flag_e);
                    client.play_sound(
                        Sound::BlockEnderChestOpen,
                        SoundCategory::Master,
                        player_pos.0,
                        1.0,
                        1.2,
                    );
                    client.send_chat_message("§a⚑ Flag returned to base!");
                }
                break;
            }
        }
    }

    // ── Apply captures ────────────────────────────────────────────────────────
    for (player_e, flag_e) in captures {
        commands.entity(player_e).remove::<FlagCarrier>();
        if let Ok((_, mut flag)) = flags.get_mut(flag_e) {
            let base = flag.base_pos;
            flag.carrier = None;
            flag.dropped_pos = None;
            flag.dropped_at = None;
            flag.dropped_by = None;
            for &part in &flag.part_entities {
                if let Ok(mut p) = part_positions.get_mut(part) {
                    p.0 = base;
                }
            }
        }
    }

    // ── Apply pickups ─────────────────────────────────────────────────────────
    for (player_e, flag_e, flag_team, _) in pickups {
        if let Ok((_, mut flag)) = flags.get_mut(flag_e) {
            if flag.carrier.is_none() {
                flag.carrier = Some(player_e);
                flag.dropped_pos = None;
                flag.dropped_at = None;
                flag.dropped_by = None;
                commands.entity(player_e).insert(FlagCarrier {
                    flag_entity: flag_e,
                    flag_team,
                });
            }
        }
    }

    // ── Apply same-team flag returns ──────────────────────────────────────────
    for flag_e in returns {
        if let Ok((_, mut flag)) = flags.get_mut(flag_e) {
            let base = flag.base_pos;
            flag.dropped_pos = None;
            flag.dropped_at = None;
            flag.dropped_by = None;
            for &part in &flag.part_entities {
                if let Ok(mut p) = part_positions.get_mut(part) {
                    p.0 = base;
                }
            }
        }
    }
}

// ─── Shift to drop flag ───────────────────────────────────────────────────────

/// Pressing Shift (sneak) while carrying a flag drops it back to its base.
pub fn drop_flag_on_sneak(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut Tank, &EntityFlags, Option<&FlagCarrier>, &mut Client, &Position),
        Without<Dead>,
    >,
    mut flags: Query<&mut Flag>,
    mut part_positions: Query<&mut Position, Without<Tank>>,
) {
    for (player_e, mut tank, entity_flags, carrier_opt, mut client, player_pos) in &mut players {
        let sneaking = entity_flags.sneaking();
        let just_pressed = sneaking && !tank.prev_sneaking;
        tank.prev_sneaking = sneaking;

        if just_pressed {
            if let Some(fc) = carrier_opt {
                if let Ok(mut flag) = flags.get_mut(fc.flag_entity) {
                    let drop_pos = player_pos.0;
                    flag.carrier = None;
                    flag.dropped_pos = Some(drop_pos);
                    flag.dropped_at = Some(Instant::now());
                    flag.dropped_by = Some(player_e);
                    for &part in &flag.part_entities {
                        if let Ok(mut p) = part_positions.get_mut(part) {
                            p.0 = drop_pos;
                        }
                    }
                }
                commands.entity(player_e).remove::<FlagCarrier>();
                client.play_sound(
                    Sound::BlockWoodenPressurePlateClickOff,
                    SoundCategory::Master,
                    player_pos.0,
                    1.0,
                    1.2,
                );
                client.send_chat_message("§7⚑ Flag dropped. Returns to base in 15 seconds if unclaimed.");
            }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns a coloured flag glyph (flashing when the flag is in play).
fn flag_indicator(carried: bool, flash_on: bool) -> &'static str {
    if !carried {
        return "⚑";
    }
    if flash_on { "§e⚑" } else { "§8⚑" }
}
