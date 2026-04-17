/// Builds the physical game world used by Valence as the physics engine.
///
/// The arena is a flat stone floor enclosed by walls.  Players walk on it;
/// Minecraft's built-in collision engine keeps them on the surface.
/// The web frontend provides the actual visual representation.

use crate::config::*;
use rand::Rng;
use valence::math::DVec3;
use valence::prelude::*;

// ─── Startup system ───────────────────────────────────────────────────────────

pub fn setup_arena(
    mut commands: Commands,
    server: Res<Server>,
    biomes: Res<BiomeRegistry>,
    dimensions: Res<DimensionTypeRegistry>,
) {
    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    // Pre-generate all chunks covering the arena
    for cz in -CHUNK_RADIUS..=CHUNK_RADIUS {
        for cx in -CHUNK_RADIUS..=CHUNK_RADIUS {
            layer.chunk.insert_chunk([cx, cz], UnloadedChunk::new());
        }
    }

    fill_floor(&mut layer);
    build_walls(&mut layer);
    build_obstacles(&mut layer);

    commands.spawn(layer);
    tracing::info!(
        radius = ARENA_RADIUS,
        floor_y = ARENA_FLOOR_Y,
        "Arena ready (physics layer)"
    );
}

// ─── Floor ───────────────────────────────────────────────────────────────────

fn fill_floor(layer: &mut LayerBundle) {
    let r = ARENA_RADIUS;
    for x in -r..=r {
        for z in -r..=r {
            layer.chunk.set_block([x, ARENA_FLOOR_Y - 1, z], BlockState::BEDROCK);
            layer.chunk.set_block([x, ARENA_FLOOR_Y, z], BlockState::WHITE_TERRACOTTA);
        }
    }
    // Visible centre marker (helps Minecraft clients orient themselves)
    for i in -2i32..=2 {
        layer.chunk.set_block([i, ARENA_FLOOR_Y, 0], BlockState::WHITE_CONCRETE);
        layer.chunk.set_block([0, ARENA_FLOOR_Y, i], BlockState::WHITE_CONCRETE);
    }
}

// ─── Boundary walls ──────────────────────────────────────────────────────────

fn build_walls(layer: &mut LayerBundle) {
    let r = ARENA_RADIUS;
    for i in -r..=r {
        for h in 1..=WALL_HEIGHT {
            let y = ARENA_FLOOR_Y + h;
            layer.chunk.set_block([i, y, -r], BlockState::YELLOW_CONCRETE);
            layer.chunk.set_block([i, y,  r], BlockState::YELLOW_CONCRETE);
            layer.chunk.set_block([-r, y, i], BlockState::YELLOW_CONCRETE);
            layer.chunk.set_block([ r, y, i], BlockState::YELLOW_CONCRETE);
        }
    }
}

// ─── Obstacles (cover blocks) ─────────────────────────────────────────────────

fn build_obstacles(layer: &mut LayerBundle) {
    let mut rng = rand::thread_rng();
    let max = ARENA_RADIUS - 6;

    for _ in 0..OBSTACLE_COUNT {
        let cx: i32 = rng.gen_range(-max..=max);
        let cz: i32 = rng.gen_range(-max..=max);

        // Leave the spawn cross clear
        if cx.abs() < 5 && cz.abs() < 5 {
            continue;
        }

        match rng.gen_range(0u8..4) {
            // L-shaped bunker
            0 => {
                for x in 0..5i32 {
                    for h in 1..=3i32 {
                        layer.chunk.set_block(
                            [cx + x, ARENA_FLOOR_Y + h, cz],
                            BlockState::LIGHT_BLUE_CONCRETE,
                        );
                    }
                }
                for z in 1..=3i32 {
                    for h in 1..=3i32 {
                        layer.chunk.set_block(
                            [cx, ARENA_FLOOR_Y + h, cz + z],
                            BlockState::LIGHT_BLUE_CONCRETE,
                        );
                    }
                }
            }
            // Iron pillar with overhang
            1 => {
                for h in 1..=5i32 {
                    layer.chunk.set_block([cx, ARENA_FLOOR_Y + h, cz], BlockState::YELLOW_TERRACOTTA);
                }
                for d in -1i32..=1 {
                    layer.chunk.set_block([cx + d, ARENA_FLOOR_Y + 5, cz], BlockState::YELLOW_TERRACOTTA);
                    layer.chunk.set_block([cx, ARENA_FLOOR_Y + 5, cz + d], BlockState::YELLOW_TERRACOTTA);
                }
            }
            // Horizontal wall segment
            2 => {
                let len: i32 = rng.gen_range(4..9);
                let vertical = rng.gen_bool(0.5);
                for i in 0..len {
                    for h in 1..=2i32 {
                        if vertical {
                            layer.chunk.set_block(
                                [cx, ARENA_FLOOR_Y + h, cz + i],
                                BlockState::YELLOW_CONCRETE,
                            );
                        } else {
                            layer.chunk.set_block(
                                [cx + i, ARENA_FLOOR_Y + h, cz],
                                BlockState::YELLOW_CONCRETE,
                            );
                        }
                    }
                }
            }
            // U-shaped fortification
            _ => {
                let w: i32 = rng.gen_range(3..6);
                let d: i32 = rng.gen_range(2..4);
                for x in 0..=w {
                    for h in 1..=3i32 {
                        layer.chunk.set_block(
                            [cx + x, ARENA_FLOOR_Y + h, cz],
                            BlockState::ORANGE_TERRACOTTA,
                        );
                        if x == 0 || x == w {
                            for z in 1..=d {
                                layer.chunk.set_block(
                                    [cx + x, ARENA_FLOOR_Y + h, cz + z],
                                    BlockState::ORANGE_TERRACOTTA,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Random spawn position inside the arena, one block above the floor.
pub fn random_spawn_pos() -> DVec3 {
    let mut rng = rand::thread_rng();
    let margin = ARENA_RADIUS - 5;
    DVec3::new(
        rng.gen_range(-margin..=margin) as f64 + 0.5,
        (ARENA_FLOOR_Y + 1) as f64,
        rng.gen_range(-margin..=margin) as f64 + 0.5,
    )
}
