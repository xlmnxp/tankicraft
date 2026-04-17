use crate::components::{Bullet, Dead, FlagCarrier, KillEvent, Tank};
use crate::config::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};
use valence::entity::block_display::{
    BlockDisplayEntityBundle, BlockState as BlockDisplayBlockState,
};
use valence::entity::display::{Scale, ViewRange};
use valence::entity::EntityLayerId;
use valence::hand_swing::HandSwingEvent;
use valence::prelude::Despawned;
use valence::math::DVec3;
use valence::prelude::*;
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::GameMessageS2c;
use valence::protocol::sound::{Sound, SoundCategory};
use valence::text::IntoText;

pub fn tick_shoot_cooldowns(mut tanks: Query<&mut Tank, Without<Dead>>) {
    for mut tank in &mut tanks {
        if tank.shoot_cooldown > 0 {
            tank.shoot_cooldown -= 1;
        }
    }
}

/// Left-click (arm swing) fires the cannon.
pub fn handle_shooting(
    mut commands: Commands,
    mut events: EventReader<HandSwingEvent>,
    mut tanks: Query<(&mut Tank, &Position, &Look, &mut Client), Without<Dead>>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    let Ok(layer) = layers.get_single() else {
        return;
    };

    for event in events.iter() {
        let Ok((mut tank, pos, look, mut client)) = tanks.get_mut(event.client) else {
            continue;
        };
        if tank.shoot_cooldown > 0 {
            continue;
        }
        tank.shoot_cooldown = SHOOT_COOLDOWN_TICKS;

        let yaw = look.yaw.to_radians() as f64;
        let pitch = look.pitch.to_radians() as f64;
        let dir = DVec3::new(
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        );

        let origin = pos.0 + DVec3::new(0.0, 1.62, 0.0) + dir * 1.2;

        // Spawn bullet + tiny visible iron-block display as projectile
        commands.spawn((
            Bullet {
                shooter: event.client,
                velocity: dir * BULLET_SPEED,
                damage: BULLET_DAMAGE,
                lifetime: BULLET_MAX_TICKS,
            },
            BlockDisplayEntityBundle {
                position: Position(origin),
                layer: EntityLayerId(layer),
                block_display_block_state: BlockDisplayBlockState(BlockState::IRON_BLOCK),
                display_scale: Scale(valence::math::Vec3::splat(0.12)),
                display_view_range: ViewRange(64.0),
                ..Default::default()
            },
        ));

        // Action bar confirmation + cannon-fire sound
        client.write_packet(&GameMessageS2c {
            chat: Cow::Owned("§c⚡ FIRE!".into_text()),
            overlay: true,
        });
        client.play_sound(
            Sound::EntityBlazeShoot,
            SoundCategory::Master,
            pos.0,
            1.0,
            0.8,
        );
    }
}

pub fn update_bullets(
    mut commands: Commands,
    mut bullets: Query<(Entity, &mut Position, &mut Bullet)>,
    mut tanks: Query<
        (Entity, &mut Tank, &Position, &mut Client),
        (Without<Bullet>, Without<Dead>),
    >,
    mut kill_events: EventWriter<KillEvent>,
) {
    for (bullet_entity, mut bullet_pos, mut bullet) in &mut bullets {
        bullet_pos.0 += bullet.velocity;

        if bullet.lifetime == 0 {
            commands.entity(bullet_entity).insert(Despawned);
            continue;
        }
        bullet.lifetime -= 1;

        // Out-of-arena despawn
        if bullet_pos.0.x.abs() > ARENA_RADIUS as f64
            || bullet_pos.0.z.abs() > ARENA_RADIUS as f64
        {
            commands.entity(bullet_entity).insert(Despawned);
            continue;
        }

        let bp = bullet_pos.0;
        let shooter = bullet.shooter;
        let damage = bullet.damage;

        for (tank_entity, mut tank, tank_pos, mut client) in &mut tanks {
            if tank_entity == shooter {
                continue;
            }
            if (bp - tank_pos.0).length() <= BULLET_HIT_RADIUS {
                tank.health = (tank.health - damage).max(0.0);

                // Hit sound + chat feedback
                client.play_sound(
                    Sound::EntityGenericExplode,
                    SoundCategory::Master,
                    tank_pos.0,
                    1.0,
                    1.2,
                );
                client.send_chat_message(format!(
                    "§c§lHIT! §r§cHP: {:.0}/{:.0}",
                    tank.health, tank.max_health
                ));
                commands.entity(bullet_entity).insert(Despawned);

                if tank.health <= 0.0 {
                    tank.deaths += 1;
                    kill_events.send(KillEvent { killer: shooter });
                    commands.entity(tank_entity).insert(Dead {
                        respawn_at: Instant::now()
                            + Duration::from_secs_f64(RESPAWN_DELAY_SECS),
                    });
                    client.send_chat_message(format!(
                        "§4§l✕ Destroyed! §r§7Respawning in {RESPAWN_DELAY_SECS:.0}s…"
                    ));
                }
                break;
            }
        }
    }
}

/// When a carrier is killed, drop the flag at the death position with a 15-second return timer.
pub fn drop_flag_on_kill(
    mut commands: Commands,
    mut queries: ParamSet<(
        Query<(Entity, &FlagCarrier, &Position), Added<Dead>>,
        Query<&mut Position, Without<crate::components::Tank>>,
    )>,
    mut flags: Query<&mut crate::components::Flag>,
) {
    // Collect dead carrier data before borrowing part_positions mutably.
    let dead_carriers: Vec<(Entity, Entity, DVec3)> = queries
        .p0()
        .iter()
        .map(|(e, fc, pos)| (e, fc.flag_entity, pos.0))
        .collect();

    for (player_entity, flag_entity, drop_pos) in dead_carriers {
        let part_entities: Vec<Entity> = if let Ok(mut flag) = flags.get_mut(flag_entity) {
            flag.carrier = None;
            flag.dropped_pos = Some(drop_pos);
            flag.dropped_at = Some(Instant::now());
            flag.dropped_by = Some(player_entity);
            flag.part_entities.clone()
        } else {
            vec![]
        };

        for part in part_entities {
            if let Ok(mut p) = queries.p1().get_mut(part) {
                p.0 = drop_pos;
            }
        }

        commands.entity(player_entity).remove::<FlagCarrier>();
    }
}
