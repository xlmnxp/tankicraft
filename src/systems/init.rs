use crate::{
    arena::random_spawn_pos,
    components::Tank,
    systems::visuals::spawn_tank_visuals,
};
use valence::entity::entity::Flags as EntityFlags;
use valence::prelude::*;

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
        // Spectator mode: hides the vanilla inventory/hotbar UI and keeps the
        // player ghost invisible.  HandSwingEvent still fires on left-click.
        *mode = GameMode::Adventure;

        // Hide the Minecraft player model – the tank Display Entities are the visuals.
        flags.set_invisible(true);

        // Spawn the 3-D tank (five BlockDisplay + two TextDisplay entities)
        let visuals = spawn_tank_visuals(&mut commands, layer, pos.0, &username.0);
        commands.entity(entity).insert((Tank::default(), visuals));

        client.send_chat_message("§6§l== Tanki Online ==");
        client.send_chat_message("§7WASD to move  ·  §fLeft-click §7to fire");
    }
}
