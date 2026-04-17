use std::time::Instant;
use valence::math::DVec3;
use valence::prelude::*;

// ─── Tank ─────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Tank {
    pub health: f32,
    pub max_health: f32,
    pub kills: u32,
    pub deaths: u32,
    /// Ticks until the player can fire again
    pub shoot_cooldown: u32,
    /// Hull facing direction (Minecraft yaw convention: 0=South, 90=West).
    /// Updated from horizontal movement delta each tick.
    pub hull_yaw: f32,
}

impl Default for Tank {
    fn default() -> Self {
        Self {
            health: crate::config::TANK_MAX_HEALTH,
            max_health: crate::config::TANK_MAX_HEALTH,
            kills: 0,
            deaths: 0,
            shoot_cooldown: 0,
            hull_yaw: 0.0,
        }
    }
}

// ─── Dead ─────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Dead {
    pub respawn_at: Instant,
}

// ─── Bullet ───────────────────────────────────────────────────────────────────

/// Server-side projectile – position advanced every tick by velocity.
#[derive(Component)]
pub struct Bullet {
    pub shooter: Entity,
    pub velocity: DVec3,
    pub damage: f32,
    pub lifetime: u32,
}

// ─── Visual handles ───────────────────────────────────────────────────────────

/// Entity IDs of every Display Entity and the camera anchor belonging to one tank.
/// Stored on the player (Tank) entity so we can move them each tick.
#[derive(Component)]
pub struct TankVisuals {
    /// Block Display entities for hull body + two tracks (rotate with hull_yaw)
    pub hull_parts: Vec<Entity>,
    /// Block Display entities for turret + barrel (rotate with look/turret yaw)
    pub turret_parts: Vec<Entity>,
    /// TextDisplay shown above the tank – player username
    pub name_tag: Entity,
    /// TextDisplay shown below the name tag – coloured health bar
    pub health_bar: Entity,
    /// Invisible Marker entity that the player's camera is attached to.
    /// Sits above and behind the hull; moves every tick.
    pub camera_anchor: Entity,
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct KillEvent {
    pub killer: Entity,
}
