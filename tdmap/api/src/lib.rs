//! tdmap for Horizon: the Overworld's terrain from a Terrain Diffusion map baked by `tdbake`.
//!
//! The map (`map.toml` and `tiles/`) holds each column's surface height and biome. Through
//! the catalog hook `chunk_generate` the mod builds every new chunk of the map's area from it
//! (ground, water up to the sea, the surface rule of Terrain Diffusion's own preset) and lets
//! vanilla add the carvers, structures and the biomes' features; `set_biomes` gives the chunk
//! the map's biomes (decoration, mobs, the saved chunk and the players see them). Through
//! `lod_column` the far terrain the server sends (`horizon-lod-distance`) shows the same
//! heights. Outside the map: ocean, or vanilla (config).
//!
//! The rules and the map state live in the bundle's crate `tdmap-core` (`../core`, no game
//! types, tested by `core-tests/`), which horizon-world's structure queries read too (the
//! injections in `horizon-mixin.toml`); this file is the glue.
//! Both hooks run on the generation workers, several chunks at once and never on the server
//! thread, so everything here is read-only after the first call (`OnceLock`) or locked.

use horizon_client::mod_api::*;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use tdmap_core::column::{self, BIOMES, Mat, Plan};
use tdmap_core::config::Outside;
use tdmap_core::map::TILE;
use tdmap_core::world::{self, LOG, log_problems, shore, state};

/// The level's build range as the last generated chunk had it, for the far terrain (which
/// is not told it): vanilla's until a chunk generates.
static LEVEL_MIN_Y: AtomicI32 = AtomicI32::new(-64);
static LEVEL_HEIGHT: AtomicI32 = AtomicI32::new(384);
static WARNED_HEIGHT: AtomicBool = AtomicBool::new(false);

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(TdMap)
}

struct TdMap;

impl Hooks for TdMap {
    fn chunk_generate(&self, event: &mut ChunkGenerateEvent) {
        let st = state();
        if event.flat || event.dimension != st.cfg.dimension {
            return;
        }
        let Some(map) = &st.map else { return };
        let [cx, cz] = event.chunk;
        let (x0, z0) = (cx * 16, cz * 16);
        // A chunk lies in one tile (256 is a multiple of 16).
        let tile = map.tile_at(x0, z0);
        log_problems(map);
        if tile.is_none() && st.cfg.outside == Outside::Vanilla {
            return;
        }
        let out = &mut *event.chunk_out;
        let (min_y, height) = out.level();
        LEVEL_MIN_Y.store(min_y, Ordering::Relaxed);
        LEVEL_HEIGHT.store(height, Ordering::Relaxed);
        let registry = out.registry();
        let states: Vec<_> = Mat::ALL.iter().map(|m| registry.default_state(m.name())).collect();
        let biome_ids: Vec<_> = BIOMES.iter().map(|b| out.biome_id(b)).collect();
        let sea = map.info.sea_level;
        let mut biomes = [0usize; 256];
        let mut cut = false;
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (x0 + lx, z0 + lz);
                let (h, b) = match &tile {
                    Some(t) => {
                        let c = t.column(x.rem_euclid(TILE), z.rem_euclid(TILE));
                        shore(st, map, x, z, c.height, c.biome)
                    }
                    None => (st.cfg.ocean_floor, column::OCEAN),
                };
                cut |= h >= min_y + height;
                biomes[(lz * 16 + lx) as usize] = column::biome_index(b);
                let plan = Plan::new(x, z, h, b, min_y, height, sea);
                for (lo, hi, m) in plan.runs() {
                    if let Some(s) = states[m.index()] {
                        for y in lo..hi {
                            out.set_block(x, y, z, s);
                        }
                    }
                }
            }
        }
        if cut && !WARNED_HEIGHT.swap(true, Ordering::Relaxed) {
            log::warn!(
                target: LOG,
                "the map is taller than the level (top y {}): peaks are cut flat; add the data pack tdmap-height \
                 from the map's datapack/ folder to a NEW world",
                min_y + height - 1
            );
        }
        out.set_biomes(|q| {
            let lx = ((q[0] << 2) + 2 - x0).clamp(0, 15);
            let lz = ((q[2] << 2) + 2 - z0).clamp(0, 15);
            biome_ids[biomes[(lz * 16 + lx) as usize]]
        });
        *event.generate = NativeGenerate::Replaced {
            surface: false,
            carvers: st.cfg.carvers,
            structures: st.cfg.structures,
            decorations: st.cfg.decorations,
        };
    }

    fn lod_column(&self, event: &mut LodColumnEvent) {
        let st = state();
        if !st.cfg.far_terrain || event.flat || event.dimension != st.cfg.dimension {
            return;
        }
        let Some(map) = &st.map else { return };
        let [x, z] = event.column;
        let Some((h, b)) = world::ground(event.dimension, event.flat, x, z) else { return };
        let out = &mut *event.column_out;
        let plan = Plan::new(
            x,
            z,
            h,
            b,
            LEVEL_MIN_Y.load(Ordering::Relaxed),
            LEVEL_HEIGHT.load(Ordering::Relaxed),
            map.info.sea_level,
        );
        let registry = out.registry();
        let spans: Vec<_> = plan
            .far_runs()
            .into_iter()
            .filter_map(|(lo, hi, m)| registry.default_state(m.name()).map(|s| (lo, hi, s)))
            .collect();
        let biome = out.biome_id(column::biome_name(b));
        out.replace(spans, biome);
    }
}
