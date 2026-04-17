use crate::{
    arena::random_spawn_pos,
    components::{Tank, Team, TeamCounter},
    systems::visuals::spawn_tank_visuals,
};
use valence::entity::entity::Flags as EntityFlags;
use valence::prelude::*;
use valence::text::Color;

pub fn init_client(
    mut commands: Commands,
    mut new_clients: Query<
        (
            Entity,
            &mut Client,
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut GameMode,
            &mut Inventory,
            &Username,
            &mut EntityFlags,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
    mut counter: ResMut<TeamCounter>,
) {
    let Ok(layer) = layers.get_single() else {
        return;
    };

    for (
        entity,
        mut client,
        mut layer_id,
        mut vis_chunk,
        mut vis_entities,
        mut pos,
        mut mode,
        _inv,
        username,
        mut flags,
    ) in &mut new_clients
    {
        layer_id.0 = layer;
        vis_chunk.0 = layer;
        vis_entities.0.insert(layer);

        pos.0 = random_spawn_pos();

        // Adventure mode: physics + collision, HandSwingEvent fires on left-click
        *mode = GameMode::Adventure;

        // Player model is invisible; the tank Display Entities are the visuals
        flags.set_invisible(true);

        // Assign alternating team, colour the name tag accordingly
        let team = if counter.count % 2 == 0 {
            Team::Red
        } else {
            Team::Blue
        };
        counter.count += 1;
        let team_color = match team {
            Team::Red => Color::RED,
            Team::Blue => Color::BLUE,
        };

        let visuals = spawn_tank_visuals(&mut commands, layer, pos.0, &username.0, team_color);
        commands
            .entity(entity)
            .insert((Tank::default(), visuals, team));

        let team_str = match team {
            Team::Red => "§c[Red Team]",
            Team::Blue => "§9[Blue Team]",
        };
        client.send_chat_message("§6§l== TankiCraft ==");
        client.send_chat_message(format!(
            "§7You joined {team_str}§7."
        ));
        client.send_chat_message("§7WASD to move  ·  §fLeft-click §7to fire");
        client.send_chat_message("§7Drive over the enemy flag to capture it!");
    }
}
