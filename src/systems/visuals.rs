/// Tank 3-D rendering using Minecraft Display Entities.
///
/// Each player gets a cluster of Block Display entities that form a tank shape,
/// plus two Text Display entities (name tag and health bar) and one Marker entity
/// that acts as the camera anchor.
///
/// Transformation math
/// ───────────────────
/// Minecraft display entity transformation order (rightmost applied first):
///   world_point = entity_pos + Translation + LeftRotation × (Scale × model_point)
///
/// Hull parts (hull body + tracks):
///   LeftRotation = q_hull  (from hull_yaw)
///   Translation  = q_hull * (centre - scale/2)
///
/// Turret parts (turret box + barrel):
///   LeftRotation = q_turret  (from look.yaw / mouse)
///   Translation  = q_hull * TURRET_PIVOT + q_turret * (corner - TURRET_PIVOT)
///   where corner = centre - scale/2  (in hull/turret-local space)

use crate::components::{Dead, Tank, TankVisuals};
use crate::config::{BILLBOARD_CENTER, CAM_BEHIND, CAM_HEIGHT, DISPLAY_INTERP_TICKS};
use valence::entity::block_display::{
    BlockDisplayEntityBundle, BlockState as BlockDisplayBlockState,
};
use valence::entity::display::{
    Billboard, InterpolationDuration, LeftRotation, Scale, StartInterpolation, Translation,
    ViewRange,
};
use valence::entity::marker::MarkerEntityBundle;
use valence::entity::text_display::{
    Background as TextBackground, LineWidth, Text as TextDisplayText, TextDisplayEntityBundle,
    TextOpacity,
};
use valence::entity::{EntityId, EntityLayerId, OldPosition};
use valence::math::{DVec3, Quat, Vec3};
use valence::prelude::*;
use valence::protocol::packets::play::SetCameraEntityS2c;
use valence::protocol::VarInt;
use valence::protocol::encode::WritePacket;
use valence::text::{Color, IntoText};

// ─── Tank geometry ────────────────────────────────────────────────────────────

/// Hull parts: rotate with hull_yaw.
/// (local_centre_in_tank_space, scale, block_state)
/// Tank faces South (+Z) at yaw=0.  X=right, Y=up, Z=forward.
const HULL_PARTS: &[([f32; 3], [f32; 3], fn() -> BlockState)] = &[
    // Hull body
    ([0.0, 0.2, 0.0], [0.85, 0.3, 1.2], || BlockState::GRAY_CONCRETE),
    // Left track (player-left = -X at yaw=0)
    ([-0.5, 0.15, 0.0], [0.2, 0.3, 1.4], || BlockState::BLACK_CONCRETE),
    // Right track
    ([0.5, 0.15, 0.0], [0.2, 0.3, 1.4], || BlockState::BLACK_CONCRETE),
];

/// Turret parts: rotate independently around TURRET_PIVOT with look.yaw (mouse).
const TURRET_PARTS: &[([f32; 3], [f32; 3], fn() -> BlockState)] = &[
    // Turret box (on hull top, slightly rearward)
    ([0.0, 0.50, -0.1], [0.55, 0.3, 0.55], || BlockState::WHITE_CONCRETE),
    // Barrel (extends forward from turret front)
    ([0.0, 0.55, 0.525], [0.12, 0.12, 0.7], || BlockState::WHITE_CONCRETE),
];

/// Pivot point in hull-local space around which the turret rotates.
const TURRET_PIVOT: [f32; 3] = [0.0, 0.55, 0.0];

/// Precompute the corner offset for each part: `centre - scale/2`.
fn corner(centre: [f32; 3], scale: [f32; 3]) -> Vec3 {
    Vec3::new(
        centre[0] - scale[0] * 0.5,
        centre[1] - scale[1] * 0.5,
        centre[2] - scale[2] * 0.5,
    )
}

// ─── Spawn ────────────────────────────────────────────────────────────────────

/// Spawns all Display Entities + camera anchor for a new tank and returns the handle struct.
pub fn spawn_tank_visuals(
    commands: &mut Commands,
    layer: Entity,
    player_pos: DVec3,
    username: &str,
) -> TankVisuals {
    const VIEW: ViewRange = ViewRange(64.0);

    // ── Hull parts ───────────────────────────────────────────────────────────
    let mut hull_parts = Vec::with_capacity(HULL_PARTS.len());
    for &(_centre, scale, block_fn) in HULL_PARTS {
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

    // ── Turret parts ─────────────────────────────────────────────────────────
    let mut turret_parts = Vec::with_capacity(TURRET_PARTS.len());
    for &(_centre, scale, block_fn) in TURRET_PARTS {
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

    // ── Name tag (billboard, always faces viewer) ─────────────────────────
    let name_owned = username.to_owned();
    let name_tag = commands
        .spawn(TextDisplayEntityBundle {
            position: Position(player_pos + DVec3::new(0.0, 2.0, 0.0)),
            layer: EntityLayerId(layer),
            text_display_text: TextDisplayText(
                name_owned.into_text().bold().color(Color::WHITE),
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

    // ── Health bar ────────────────────────────────────────────────────────
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

    // ── Camera anchor (Marker entity – invisible, just a position) ────────
    let cam_pos = player_pos + DVec3::new(0.0, CAM_HEIGHT, -CAM_BEHIND);
    let camera_anchor = commands
        .spawn(MarkerEntityBundle {
            position: Position(cam_pos),
            layer: EntityLayerId(layer),
            ..Default::default()
        })
        .id();

    TankVisuals {
        hull_parts,
        turret_parts,
        name_tag,
        health_bar,
        camera_anchor,
    }
}

// ─── Camera setup (runs once per player after TankVisuals is attached) ────────

/// Sends the SetCamera packet once so each client views the world from their
/// camera anchor entity instead of their own eyes.
pub fn setup_camera(
    mut players: Query<(&mut Client, &TankVisuals), Added<TankVisuals>>,
    anchor_ids: Query<&EntityId>,
) {
    for (mut client, visuals) in &mut players {
        if let Ok(anchor_id) = anchor_ids.get(visuals.camera_anchor) {
            client.write_packet(&SetCameraEntityS2c {
                entity_id: VarInt(anchor_id.get()),
            });
        }
    }
}

// ─── Per-tick update ──────────────────────────────────────────────────────────

/// Moves and rotates every tank's display entities and the camera anchor to
/// match the player position and look direction, and refreshes the health bar.
pub fn update_tank_visuals(
    // Alive players (mutable Tank to update hull_yaw)
    mut alive: Query<(&TankVisuals, &Position, &OldPosition, &Look, &mut Tank), Without<Dead>>,
    // Dead players (hide underground)
    dead: Query<&TankVisuals, With<Dead>>,
    // Mutable access to display entity components (never have Tank component)
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
    // ── Alive tanks ──────────────────────────────────────────────────────────
    for (visuals, player_pos, old_pos, look, mut tank) in &mut alive {
        // ── Update hull_yaw from horizontal movement direction ─────────────
        let delta = player_pos.0 - old_pos.get();
        let horiz = DVec3::new(delta.x, 0.0, delta.z);
        if horiz.length_squared() > 1e-6 {
            // atan2(-dx, dz) gives 0 when moving South (+Z), positive CCW (West)
            // – same convention as Minecraft's yaw
            tank.hull_yaw = f64::atan2(-delta.x, delta.z).to_degrees() as f32;
        }

        // ── Rotation quaternions ──────────────────────────────────────────
        let q_hull = Quat::from_rotation_y(-tank.hull_yaw.to_radians());
        let q_turret = Quat::from_rotation_y(-(look.yaw.to_radians()));
        let turret_pivot = Vec3::new(TURRET_PIVOT[0], TURRET_PIVOT[1], TURRET_PIVOT[2]);

        // ── Hull parts (hull body + tracks) ───────────────────────────────
        for (i, &part_entity) in visuals.hull_parts.iter().enumerate() {
            if let Ok((mut pos, Some(mut trans), Some(mut left_rot), _)) =
                display.get_mut(part_entity)
            {
                pos.0 = player_pos.0;
                let (centre, scale, _) = HULL_PARTS[i];
                trans.0 = q_hull * corner(centre, scale);
                left_rot.0 = q_hull;
            }
        }

        // ── Turret parts (turret box + barrel) ────────────────────────────
        let pivot_world = q_hull * turret_pivot;
        for (i, &part_entity) in visuals.turret_parts.iter().enumerate() {
            if let Ok((mut pos, Some(mut trans), Some(mut left_rot), _)) =
                display.get_mut(part_entity)
            {
                pos.0 = player_pos.0;
                let (centre, scale, _) = TURRET_PARTS[i];
                // Corner in local space, measured from turret pivot
                let corner_from_pivot = corner(centre, scale) - turret_pivot;
                // Pivot follows hull; corner rotates with turret
                trans.0 = pivot_world + q_turret * corner_from_pivot;
                left_rot.0 = q_turret;
            }
        }

        // ── Name tag (above tank, billboard) ─────────────────────────────
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.name_tag) {
            pos.0 = player_pos.0 + DVec3::new(0.0, 2.0, 0.0);
        }

        // ── Health bar ────────────────────────────────────────────────────
        if let Ok((mut pos, _, _, Some(mut text))) = display.get_mut(visuals.health_bar) {
            pos.0 = player_pos.0 + DVec3::new(0.0, 1.65, 0.0);
            text.0 = health_text(tank.health / tank.max_health);
        }

        // ── Camera anchor (above + behind hull) ───────────────────────────
        if let Ok((mut pos, _, _, _)) = display.get_mut(visuals.camera_anchor) {
            // Behind = -Z in hull local space; up = +Y
            let cam_local = Vec3::new(0.0, CAM_HEIGHT as f32, -(CAM_BEHIND as f32));
            let cam_world = q_hull * cam_local;
            pos.0 = player_pos.0
                + DVec3::new(cam_world.x as f64, cam_world.y as f64, cam_world.z as f64);
        }
    }

    // ── Dead tanks – sink underground so they disappear ───────────────────
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
        // Camera anchor stays at player position (dead players can still look around)
    }
}

// ─── Cleanup on disconnect ────────────────────────────────────────────────────

/// Despawns display entities and camera anchor when a player leaves.
pub fn cleanup_tank_visuals(
    mut commands: Commands,
    mut removed: RemovedComponents<Client>,
    visuals: Query<&TankVisuals>,
) {
    for entity in removed.iter() {
        if let Ok(vis) = visuals.get(entity) {
            for &part in vis.hull_parts.iter().chain(vis.turret_parts.iter()) {
                commands.entity(part).despawn();
            }
            commands.entity(vis.name_tag).despawn();
            commands.entity(vis.health_bar).despawn();
            commands.entity(vis.camera_anchor).despawn();
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Renders a 10-segment Unicode block bar coloured by health percentage.
fn health_text(pct: f32) -> valence::text::Text {
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
