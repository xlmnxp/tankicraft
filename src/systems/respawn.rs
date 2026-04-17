use crate::arena::random_spawn_pos;
use crate::components::{Dead, KillEvent, Tank};
use crate::config::TANK_MAX_HEALTH;
use std::time::Instant;
use valence::prelude::*;

pub fn grant_kill_credits(
    mut events: EventReader<KillEvent>,
    mut tanks: Query<(&mut Tank, &mut Client), Without<Dead>>,
) {
    for ev in events.iter() {
        if let Ok((mut tank, mut client)) = tanks.get_mut(ev.killer) {
            tank.kills += 1;
            client.send_chat_message(format!(
                "§6§l✔ KILL! §r§6Total: {}",
                tank.kills
            ));
        }
    }
}

pub fn handle_respawn(
    mut commands: Commands,
    mut dead: Query<(Entity, &Dead, &mut Tank, &mut Position, &mut Client)>,
) {
    let now = Instant::now();
    for (entity, dead_state, mut tank, mut pos, mut client) in &mut dead {
        let remaining = dead_state.respawn_at.saturating_duration_since(now);
        if remaining.is_zero() {
            tank.health = TANK_MAX_HEALTH;
            tank.hull_yaw = 0.0;
            pos.0 = random_spawn_pos();
            commands.entity(entity).remove::<Dead>();
            commands.entity(entity).remove::<crate::components::FlagCarrier>();
            client.send_chat_message("§a§l↑ Respawned!");
        } else if remaining.subsec_millis() > 950 {
            client.send_chat_message(format!("§7Respawning in §f{}§7…", remaining.as_secs() + 1));
        }
    }
}
