/// Tanki Online – Minecraft Edition
///
/// Tanks, bullets, and health bars are rendered in 3-D inside Minecraft
/// using Display Entities (available since MC 1.19.4).
///
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  Player entity     →  physics avatar (Adventure mode)           │
/// │  BlockDisplay ×5   →  hull, tracks, turret, barrel              │
/// │  TextDisplay ×2    →  name tag (team colour) + health bar       │
/// │  Marker entity     →  camera anchor (above + behind hull)       │
/// │  ArmorStand entity →  invisible seat inside hull                │
/// │  BlockDisplay ×2   →  CTF flags (red + blue wool)               │
/// └─────────────────────────────────────────────────────────────────┘

mod arena;
mod components;
mod config;
mod systems;

use components::{KillEvent, TeamCounter, TeamScores};
use systems::{
    cleanup_tank_visuals, drop_flag_on_kill, drop_flag_on_sneak, grant_kill_credits,
    handle_respawn, handle_shooting, init_client, setup_camera, setup_flags,
    tick_pending_camera, tick_shoot_cooldowns, update_bullets, update_ctf,
    update_tank_visuals,
};
use valence::ecs::schedule::apply_deferred;
use valence::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Events
        .add_event::<KillEvent>()
        // Resources
        .init_resource::<TeamScores>()
        .init_resource::<TeamCounter>()
        // Systems
        // apply_deferred flushes setup_arena's Commands so the layer entity
        // exists by the time setup_flags queries for it.
        .add_systems(Startup, (arena::setup_arena, apply_deferred, setup_flags).chain())
        .add_systems(
            Update,
            (
                init_client,
                setup_camera,
                tick_pending_camera,
                tick_shoot_cooldowns,
                handle_shooting,
                update_bullets,
                drop_flag_on_kill,
                drop_flag_on_sneak,
                grant_kill_credits,
                handle_respawn,
                update_ctf,
                update_tank_visuals,
                cleanup_tank_visuals,
            )
                .chain(),
        )
        .run();
}
