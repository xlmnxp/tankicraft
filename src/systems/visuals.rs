/// Tank 3-D rendering using Minecraft Display Entities.
///
/// Each player gets:
///  • 3 BlockDisplay entities  – hull body + two tracks   (rotate with hull_yaw)
///  • 2 BlockDisplay entities  – turret box + barrel      (rotate with look.yaw)
///  • 1 TextDisplay entity     – name tag (billboard)
///  • 1 TextDisplay entity     – health bar (billboard)
///  • 1 Marker entity          – camera anchor (above + behind hull)
///  • 1 ArmorStand entity      – invisible seat inside the hull
///
/// Transformation math
/// ───────────────────
/// Minecraft: world_point = entity_pos + Translation + LeftRotation × (Scale × model_point)
///
/// Hull parts:   LeftRotation = q_hull,  Translation = q_hull * (centre − scale/2)
/// Turret parts: LeftRotation = q_turret, Translation = q_hull*pivot + q_turret*(corner−pivot)

use crate::components::{Dead, PendingCamera, Tank, TankVisuals};
use crate::config::{BILLBOARD_CENTER, CAM_BEHIND, CAM_HEIGHT, DISPLAY_INTERP_TICKS};
use valence::entity::armor_stand::{ArmorStandEntityBundle, ArmorStandFlags};
use valence::entity::block_display::{
    BlockDisplayEntityBundle, BlockState as BlockDisplayBlockState,
};
use valence::entity::display::{
    Billboard, InterpolationDuration, LeftRotation, Scale, StartInterpolation, Translation,
    ViewRange,
};
use valence::entity::entity::{Flags as EntityFlags, NoGravity};
use valence::entity::marker::MarkerEntityBundle;
use valence::entity::text_display::{
    Background as TextBackground, LineWidth, Text as TextDisplayText, TextDisplayEntityBundle,
    TextOpacity,
};
use valence::entity::{EntityId, EntityLayerId, OldPosition};
use valence::math::{DVec3, Quat, Vec3};
use valence::prelude::*;
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::SetCameraEntityS2c;
use valence::protocol::VarInt;
use valence::text::{Color, IntoText};

// ─── Tank geometry ────────────────────────────────────────────────────────────

/// Hull parts – rotate with hull_yaw.  Tank faces South (+Z) at yaw=0.
const HULL_PARTS: &[([f32; 3], [f32; 3], fn() -> BlockState)] = &[
    ([0.0, 0.2, 0.0], [0.85, 0.3, 1.2], || BlockState::GRAY_CONCRETE),
    ([-0.5, 0.15, 0.0], [0.2, 0.3, 1.4], || BlockState::BLACK_CONCRETE),
    ([0.5, 0.15, 0.0], [0.2, 0.3, 1.4], || BlockState::BLACK_CONCRETE),
];

/// Turret parts – rotate around TURRET_PIVOT with look.yaw (mouse).
const TURRET_PARTS: &[([f32; 3], [f32; 3], fn() -> BlockState)] = &[
    ([0.0, 0.50, -0.1], [0.55, 0.3, 0.55], || BlockState::WHITE_CONCRETE),
    ([0.0, 0.55, 0.525], [0.12, 0.12, 0.7], || BlockState::WHITE_CONCRETE),
];

/// Pivot point in hull-local space around which the turret rotates.
const TURRET_PIVOT: [f32; 3] = [0.0, 0.55, 0.0];

fn corner(centre: [f32; 3], scale: [f32; 3]) -> Vec3 {
    Vec3::new(
        centre[0] - scale[0] * 0.5,
        centre[1] - scale[1] * 0.5,
        centre[2] - scale[2] * 0.5,
    )
}

// ─── Spawn ────────────────────────────────────────────────────────────────────

pub fn spawn_tank_visuals(
    commands: &mut Commands,
    layer: Entity,
    player_pos: DVec3,
    username: &str,
    team_color: Color,
) -> TankVisuals {
    const VIEW: ViewRange = ViewRange(64.0);

    // Hull parts
    let mut hull_parts = Vec::with_capacity(HULL_PARTS.len());
    for &(_c, scale, block_fn) in HULL_PARTS {
        let id = commands
            .spawn(BlockDisplayEntityBundle {
                position: Position(player_pos),
                layer: EntityLayerId(layer),
                block_display_block_state: BlockDisplayBlockState(block_fn()),
                display_scale: Scale(Vec3::from(scale)),
                display_interpolation_duration: InterpolationDuration(DISPLAY_INTERP_TICKS),
                display_start_interpolation: StartInterpolation(0),
                display_view_range: VIEW,
                ..Default::default()
            })
            .id();
        hull_parts.push(id);
    }

    // Turret parts
    let mut turret_parts = Vec::with_capacity(TURRET_PARTS.len());
    for &(_c, scale, block_fn) in TURRET_PARTS {
        let id = commands
            .spawn(BlockDisplayEntityBundle {
                position: Position(player_pos),
                layer: EntityLayerId(layer),
                block_display_block_state: BlockDisplayBlockState(block_fn()),
                display_scale: Scale(Vec3::from(scale)),
                display_interpolation_duration: InterpolationDuration(DISPLAY_INTERP_TICKS),
                display_start_interpolation: StartInterpolation(0),
                display_view_range: VIEW,
                ..Default::default()
            })
            .id();
        turret_parts.push(id);
    }

    // Name tag
    let name_owned = username.to_owned();
    let name_tag = commands
        .spawn(TextDisplayEntityBundle {
            position: Position(player_pos + DVec3::new(0.0, 2.0, 0.0)),
            layer: EntityLayerId(layer),
            text_display_text: TextDisplayText(
                name_owned.into_text().bold().color(team_color),
            ),
            display_billboard: Billboard(BILLBOARD_CENTER),
            display_scale: Scale(Vec3::splat(0.55)),
            display_view_range: VIEW,
            text_display_line_width: LineWidth(200),
            text_display_background: TextBackground(0x40_000000u32 as i32),
            text_display_text_opacity: TextOpacity(-1),
            ..Default::default()
        })
        .id();

    // Health bar
    let health_bar = commands
        .spawn(TextDisplayEntityBundle {
            position: Position(player_pos + DVec3::new(0.0, 1.65, 0.0)),
            layer: EntityLayerId(layer),
            text_display_text: TextDisplayText(health_text(1.0)),
            display_billboard: Billboard(BILLBOARD_CENTER),
            display_scale: Scale(Vec3::splat(0.45)),
            display_view_range: VIEW,
            text_display_line_width: LineWidth(200),
            text_display_background: TextBackground(0x40_000000u32 as i32),
            text_display_text_opacity: TextOpacity(-1),
            ..Default::default()
        })
        .id();

    // Camera anchor – invisible Marker entity above+behind the tank
    let cam_pos = player_pos + DVec3::new(0.0, CAM_HEIGHT, -CAM_BEHIND);
    let camera_anchor = commands
        .spawn(MarkerEntityBundle {
            position: Position(cam_pos),
            layer: EntityLayerId(layer),
            ..Default::default()
        })
        .id();

    // Invisible armor-stand seat hidden inside the hull.
    // Other clients see the player "seated" here; the player's own camera
    // is at the camera_anchor above the tank.
    let mut seat_flags = EntityFlags::default();
    seat_flags.set_invisible(true);
    let seat = commands
        .spawn(ArmorStandEntityBundle {
            position: Position(player_pos),
            layer: EntityLayerId(layer),
            entity_flags: seat_flags,
            // 0x10 = Marker flag → no hit-box, no physics
            armor_stand_armor_stand_flags: ArmorStandFlags(0x10),
            entity_no_gravity: NoGravity(true),
            ..Default::default()
        })
        .id();

    TankVisuals {
        hull_parts,
        turret_parts,
        name_tag,
        health_bar,
        camera_anchor,
        seat,
    }
}

// ─── Camera setup (once per player) ──────────────────────────────────────────

/// Inserts a PendingCamera countdown so the camera packet is sent a few ticks
/// after joining — guaranteeing the anchor entity spawn has reached the client.
pub fn setup_camera(
    mut commands: Commands,
    new_players: Query<Entity, Added<TankVisuals>>,
) {
    for entity in &new_players {
        commands.entity(entity).insert(PendingCamera(4));
    }
}

/// Counts down PendingCamera each tick and sends SetCameraEntityS2c on zero.
pub fn tick_pending_camera(
    mut commands: Commands,
    mut players: Query<(Entity, &mut Client, &TankVisuals, &mut PendingCamera)>,
    anchor_ids: Query<&EntityId>,
) {
    for (entity, mut client, visuals, mut pending) in &mut players {
        if pending.0 == 0 {
            if let Ok(anchor_id) = anchor_ids.get(visuals.camera_anchor) {
                client.write_packet(&SetCameraEntityS2c {
                    entity_id: VarInt(anchor_id.get()),
                });
            }
            commands.entity(entity).remove::<PendingCamera>();
        } else {
            pending.0 -= 1;
        }
    }
}

// ─── Per-tick update ──────────────────────────────────────────────────────────

pub fn update_tank_visuals(
    mut alive: Query<(&TankVisuals, &Position, &OldPosition, &Look, &mut Tank), Without<Dead>>,
    dead: Query<&TankVisuals, With<Dead>>,
    mut display: Query<
        (
            &mut Position,
            Option<&mut Translation>,
            Option<&mut LeftRotation>,
            Option<&mut TextDisplayText>,
        ),
        Without<Tank>,
    >,
) {
    for (visuals, player_pos, old_pos, look, mut tank) in &mut alive {
        // Hull yaw from horizontal movement delta
        let delta = player_pos.0 - old_pos.get();
        let horiz = DVec3::new(delta.x, 0.0, delta.z);
        if horiz.length_squared() > 1e-6 {
            tank.hull_yaw = f64::atan2(-delta.x, delta.z).to_degrees() as f32;
        }

        let q_hull = Quat::from_rotation_y(-tank.hull_yaw.to_radians());
        let q_turret = Quat::from_rotation_y(-(look.yaw.to_radians()));
        let turret_pivot = Vec3::new(TURRET_PIVOT[0], TURRET_PIVOT[1], TURRET_PIVOT[2]);

        // Hull parts
        for (i, &part) in visuals.hull_parts.iter().enumerate() {
            if let Ok((mut pos, Some(mut trans), Some(mut lr), _)) = display.get_mut(part) {
                pos.0 = player_pos.0;
                let (c, s, _) = HULL_PARTS[i];
                trans.0 = q_hull * corner(c, s);
                lr.0 = q_hull;
            }
        }

        // Turret parts
        let pivot_world = q_hull * turret_pivot;
        for (i, &part) in visuals.turret_parts.iter().enumerate() {
            if let Ok((mut pos, Some(mut trans), Some(mut lr), _)) = display.get_mut(part) {
                pos.0 = player_pos.0;
                let (c, s, _) = TURRET_PARTS[i];
                let corner_from_pivot = corner(c, s) - turret_pivot;
                trans.0 = pivot_world + q_turret * corner_from_pivot;
                lr.0 = q_turret;
            }
        }

        // Name tag
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.name_tag) {
            pos.0 = player_pos.0 + DVec3::new(0.0, 2.0, 0.0);
        }

        // Health bar
        if let Ok((mut pos, _, _, Some(mut text))) = display.get_mut(visuals.health_bar) {
            pos.0 = player_pos.0 + DVec3::new(0.0, 1.65, 0.0);
            text.0 = health_text(tank.health / tank.max_health);
        }

        // Camera anchor – above + behind hull
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.camera_anchor) {
            let cam_local = Vec3::new(0.0, CAM_HEIGHT as f32, -(CAM_BEHIND as f32));
            let cam_world = q_hull * cam_local;
            pos.0 = player_pos.0
                + DVec3::new(cam_world.x as f64, cam_world.y as f64, cam_world.z as f64);
        }

        // Seat armor-stand – inside the hull centre
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.seat) {
            pos.0 = player_pos.0;
        }
    }

    // Dead tanks – sink underground
    for visuals in &dead {
        for &part in visuals.hull_parts.iter().chain(visuals.turret_parts.iter()) {
            if let Ok((mut pos, _, _, _)) = display.get_mut(part) {
                pos.0.y = -200.0;
            }
        }
        for &label in &[visuals.name_tag, visuals.health_bar] {
            if let Ok((mut pos, _, _, _)) = display.get_mut(label) {
                pos.0.y = -200.0;
            }
        }
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.seat) {
            pos.0.y = -200.0;
        }
    }
}

// ─── Cleanup on disconnect ────────────────────────────────────────────────────

pub fn cleanup_tank_visuals(
    mut commands: Commands,
    mut removed: RemovedComponents<Client>,
    visuals: Query<&TankVisuals>,
) {
    for entity in removed.iter() {
        if let Ok(vis) = visuals.get(entity) {
            for &part in vis.hull_parts.iter().chain(vis.turret_parts.iter()) {
                commands.entity(part).insert(Despawned);
            }
            commands.entity(vis.name_tag).insert(Despawned);
            commands.entity(vis.health_bar).insert(Despawned);
            commands.entity(vis.camera_anchor).insert(Despawned);
            commands.entity(vis.seat).insert(Despawned);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

pub fn health_text(pct: f32) -> valence::text::Text {
    let filled = (pct.clamp(0.0, 1.0) * 10.0).round() as usize;
    let empty = 10 - filled;
    let color = if pct > 0.6 {
        Color::GREEN
    } else if pct > 0.3 {
        Color::GOLD
    } else {
        Color::RED
    };
    format!("{}{}", "█".repeat(filled), "▒".repeat(empty))
        .into_text()
        .color(color)
}
