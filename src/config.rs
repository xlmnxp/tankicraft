// ─── Arena ────────────────────────────────────────────────────────────────────
pub const ARENA_RADIUS: i32 = 40;
pub const ARENA_FLOOR_Y: i32 = 64;
pub const WALL_HEIGHT: i32 = 255;
pub const OBSTACLE_COUNT: usize = 25;
pub const CHUNK_RADIUS: i32 = 6;

// ─── Tank / Combat ────────────────────────────────────────────────────────────
pub const TANK_MAX_HEALTH: f32 = 100.0;
pub const BULLET_DAMAGE: f32 = 25.0;
/// Bullet travel speed in blocks per tick
pub const BULLET_SPEED: f64 = 1.5;
/// Ticks before a bullet auto-despawns
pub const BULLET_MAX_TICKS: u32 = 60;
/// Sphere radius used for bullet–tank hit detection
pub const BULLET_HIT_RADIUS: f64 = 1.2;
/// Ticks the player must wait between shots
pub const SHOOT_COOLDOWN_TICKS: u32 = 15;

// ─── Respawn ──────────────────────────────────────────────────────────────────
pub const RESPAWN_DELAY_SECS: f64 = 3.0;

// ─── Display entity visual constants ─────────────────────────────────────────
/// Billboard mode 3 = CENTER (always faces the viewer)
pub const BILLBOARD_CENTER: i8 = 3;
/// Tick-to-tick interpolation duration for smooth tank movement
pub const DISPLAY_INTERP_TICKS: i32 = 3;

// ─── Camera ───────────────────────────────────────────────────────────────────
/// How many blocks above the player the camera anchor sits
pub const CAM_HEIGHT: f64 = 14.0;
/// How many blocks behind the hull (in hull-local -Z) the camera anchor sits
pub const CAM_BEHIND: f64 = 2.0;

// ─── CTF ──────────────────────────────────────────────────────────────────────
pub const RED_BASE: [f64; 3] = [-30.0, 65.0, -30.0];
pub const BLUE_BASE: [f64; 3] = [30.0, 65.0, 30.0];
pub const FLAG_PICKUP_RADIUS: f64 = 2.0;
pub const FLAG_CAPTURE_RADIUS: f64 = 3.0;
/// How many ticks between score-bar broadcasts (10 = twice per second; fast enough to flash)
pub const SCORE_BROADCAST_TICKS: u32 = 10;
/// Seconds after dropping the flag before the dropper can pick it up again
pub const FLAG_DROP_PICKUP_COOLDOWN_SECS: u64 = 4;
