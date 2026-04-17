use crate::components::{Bullet, Dead, KillEvent, Tank};
use crate::config::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};
use valence::hand_swing::HandSwingEvent;
use valence::math::DVec3;
use valence::prelude::*;
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::GameMessageS2c;
use valence::text::IntoText;

pub fn tick_shoot_cooldowns(mut tanks: Query<&mut Tank, Without<Dead>>) {
    for mut tank in &mut tanks {
        if tank.shoot_cooldown > 0 {
            tank.shoot_cooldown -= 1;
        }
    }
}

/// Left-click (arm swing) fires the cannon.
/// Works in both Adventure and Spectator game modes since HandSwingC2s is sent
/// by the client regardless of game mode when left-clicking.
pub fn handle_shooting(
    mut commands: Commands,
    mut events: EventReader<HandSwingEvent>,
    mut tanks: Query<(&mut Tank, &Position, &Look, &mut Client), Without<Dead>>,
) {
    for event in events.iter() {
        let Ok((mut tank, pos, look, mut client)) = tanks.get_mut(event.client) else {
            continue;
        };
        if tank.shoot_cooldown > 0 {
            continue;
        }
        tank.shoot_cooldown = SHOOT_COOLDOWN_TICKS;

        // Direction from Minecraft look angles
        // yaw 0=South(+Z), 90=West(-X); pitch -90=up, 90=down
        let yaw = look.yaw.to_radians() as f64;
        let pitch = look.pitch.to_radians() as f64;
        let dir = DVec3::new(
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        );

        let origin = pos.0 + DVec3::new(0.0, 1.62, 0.0) + dir * 1.2;

        commands.spawn((
            Bullet {
                shooter: event.client,
                velocity: dir * BULLET_SPEED,
                damage: BULLET_DAMAGE,
                lifetime: BULLET_MAX_TICKS,
            },
            Position(origin),
        ));

        // Action bar feedback so the shooter knows the shot fired
        client.write_packet(&GameMessageS2c {
            chat: Cow::Owned("§c⚡ FIRE!".into_text()),
            overlay: true,
        });
    }
}

pub fn update_bullets(
    mut commands: Commands,
    mut bullets: Query<(Entity, &mut Position, &mut Bullet)>,
    mut tanks: Query<(Entity, &mut Tank, &Position, &mut Client), (Without<Bullet>, Without<Dead>)>,
    mut kill_events: EventWriter<KillEvent>,
) {
    for (bullet_entity, mut bullet_pos, mut bullet) in &mut bullets {
        bullet_pos.0 += bullet.velocity;

        if bullet.lifetime == 0 {
            commands.entity(bullet_entity).despawn();
            continue;
        }
        bullet.lifetime -= 1;

        // Out-of-arena despawn
        if bullet_pos.0.x.abs() > ARENA_RADIUS as f64
            || bullet_pos.0.z.abs() > ARENA_RADIUS as f64
        {
            commands.entity(bullet_entity).despawn();
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
                client.send_chat_message(format!(
                    "§c§lHIT! §r§cHP: {:.0}/{:.0}",
                    tank.health, tank.max_health
                ));
                commands.entity(bullet_entity).despawn();

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
