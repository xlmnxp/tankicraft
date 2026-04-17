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
    pub shoot_cooldown: u32,
    /// Hull facing direction (Minecraft yaw convention: 0=South, 90=West).
    pub hull_yaw: f32,
    /// Accumulates ticks for the score-bar broadcast throttle.
    pub score_tick: u32,
    /// Alternates each score broadcast; used to flash flag indicators.
    pub score_flash: bool,
    /// Sneak state from last tick — used for rising-edge detection.
    pub prev_sneaking: bool,
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
            score_tick: 0,
            score_flash: false,
            prev_sneaking: false,
        }
    }
}

// ─── Team ─────────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Team {
    Red,
    Blue,
}

// ─── Dead ─────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Dead {
    pub respawn_at: Instant,
}

// ─── Bullet ───────────────────────────────────────────────────────────────────

/// Server-side projectile – also carries a visible Block Display entity.
#[derive(Component)]
pub struct Bullet {
    pub shooter: Entity,
    pub velocity: DVec3,
    pub damage: f32,
    pub lifetime: u32,
}

// ─── Visual handles ───────────────────────────────────────────────────────────

/// Entity IDs of every Display Entity, the camera anchor, and the seat.
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
    /// Invisible Marker entity the camera is attached to (above + behind hull).
    pub camera_anchor: Entity,
    /// Invisible ArmorStand hidden inside the hull – purely cosmetic seat.
    pub seat: Entity,
}

// ─── CTF ──────────────────────────────────────────────────────────────────────

/// Data for a CTF flag. The flag itself is a logical ECS entity with no Position;
/// the three visual Block-Display entities (stand + pole + cloth) are stored in
/// `part_entities` and each has their own Position updated every tick.
#[derive(Component)]
pub struct Flag {
    pub team: Team,
    pub base_pos: DVec3,
    /// Which player is currently carrying this flag (None = at base).
    pub carrier: Option<Entity>,
    /// [stand, pole, cloth] Block Display entity handles.
    pub part_entities: Vec<Entity>,
}

/// Added to a player while they carry the enemy flag.
#[derive(Component)]
pub struct FlagCarrier {
    pub flag_entity: Entity,
    pub flag_team: Team,
}

// ─── Camera ───────────────────────────────────────────────────────────────────

/// Countdown before sending SetCameraEntityS2c.
/// Ensures the camera-anchor entity spawn packet reaches the client first.
#[derive(Component)]
pub struct PendingCamera(pub u8);

// ─── Events ───────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct KillEvent {
    pub killer: Entity,
}

// ─── Resources ────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct TeamScores {
    pub red: u32,
    pub blue: u32,
}

/// Round-robin counter used to alternate team assignments on join.
#[derive(Resource, Default)]
pub struct TeamCounter {
    pub count: u32,
}
