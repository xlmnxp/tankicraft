/// Tanki Online – Minecraft Edition
///
/// Tanks, bullets, and health bars are rendered in 3-D inside Minecraft
/// using Display Entities (available since MC 1.19.4).
///
/// ┌─────────────────────────────────────────────────────┐
/// │  Player entity    →  physics avatar (Adventure mode) │
/// │  BlockDisplay ×5  →  hull, tracks, turret, barrel    │
/// │  TextDisplay ×2   →  name tag + health bar           │
/// └─────────────────────────────────────────────────────┘

mod arena;
mod components;
mod config;
mod systems;

use components::KillEvent;
use systems::{
    cleanup_tank_visuals, grant_kill_credits, handle_respawn, handle_shooting, init_client,
    setup_camera, tick_shoot_cooldowns, update_bullets, update_tank_visuals,
};
use valence::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Events
        .add_event::<KillEvent>()
        // Systems
        .add_systems(Startup, arena::setup_arena)
        .add_systems(
            Update,
            (
                // Initialise new players and spawn their display entities
                init_client,
                // Attach each new player's camera to their anchor (needs commands flush after init_client)
                setup_camera,
                // Game logic
                tick_shoot_cooldowns,
                handle_shooting,
                update_bullets,
                grant_kill_credits,
                handle_respawn,
                // Update all Display Entity positions/rotations to match players
                // (must run AFTER handle_respawn so respawned tanks are visible)
                update_tank_visuals,
                // Despawn display entities for players who disconnected
                cleanup_tank_visuals,
            )
                .chain(),
        )
        .run();
}
