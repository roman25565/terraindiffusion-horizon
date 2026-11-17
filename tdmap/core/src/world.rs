//! The map and config this server loaded, once for the whole game: the chunk hooks of the
//! `[api]` package and the structure queries injected into horizon-world read the same state,
//! so a structure stands on the ground the chunk is built with.
//!
//! Everything runs on the generation workers, several at once: read-only after the first call
//! (`OnceLock`), the tile cache is locked inside [`Map`].

use crate::column::{self, OCEAN};
use crate::config::{self, Config, Outside};
use crate::map::Map;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Horizon's logger keeps Info/Warn only for targets starting with `horizon`.
pub const LOG: &str = "horizon::tdmap";

pub struct State {
    pub cfg: Config,
    pub map: Option<Map>,
}

static STATE: OnceLock<State> = OnceLock::new();
static STRUCTURES_SEEN: AtomicBool = AtomicBool::new(false);

/// The state, loaded from `<server>/config/tdmap.toml` (the server's working folder) on first use.
pub fn state() -> &'static State {
    STATE.get_or_init(|| {
        let server = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (cfg, problems) = config::load(&server.join("config"));
        for p in problems {
            log::warn!(target: LOG, "{p}");
        }
        let dir = cfg.map_dir(&server);
        let map = match Map::open(&dir, cfg.cache_tiles) {
            Ok(m) => {
                let (x1, z1, x2, z2) = m.info.blocks();
                let tiles = ((m.info.tile_x_max - m.info.tile_x_min) * (m.info.tile_z_max - m.info.tile_z_min)) as usize;
                log::info!(
                    target: LOG,
                    "map {}: seed {}, scale {} ({} m/block), x {x1}..{} z {z1}..{}, {} of {tiles} tiles baked; \
                     outside: {:?}; carvers {}, structures {}, decorations {}",
                    dir.display(),
                    m.info.seed,
                    m.info.scale,
                    30.0 / m.info.scale as f32,
                    x2 - 1,
                    z2 - 1,
                    m.count_baked(),
                    cfg.outside,
                    cfg.carvers,
                    cfg.structures,
                    cfg.decorations,
                );
                Some(m)
            }
            Err(e) => {
                log::warn!(target: LOG, "no map ({e}): {} generates as vanilla", cfg.dimension);
                None
            }
        };
        State { cfg, map }
    })
}

/// Logs the map's problems met so far (a tile that could not be read...).
pub fn log_problems(map: &Map) {
    for p in map.take_problems() {
        log::warn!(target: LOG, "{p}");
    }
}

/// A map column near the edge sinks into the ocean (with `outside = "ocean"`).
pub fn shore(st: &State, map: &Map, x: i32, z: i32, height: i32, biome: u8) -> (i32, u8) {
    if st.cfg.outside != Outside::Ocean {
        return (height, biome);
    }
    column::blend_to_edge(height, biome, map.info.edge_distance(x, z), st.cfg.edge_blend, st.cfg.ocean_floor, map.info.sea_level)
}

/// The ground of column `[x, z]` as the mod builds it: the first Y above the ground (before the
/// level's limits cut it) and Terrain Diffusion's biome number. None where the mod leaves the
/// level to vanilla: another dimension, a superflat level, no map, or outside the map with
/// `outside = "vanilla"`.
pub fn ground(dimension: &str, flat: bool, x: i32, z: i32) -> Option<(i32, u8)> {
    let st = state();
    if flat || dimension != st.cfg.dimension {
        return None;
    }
    let map = st.map.as_ref()?;
    match map.column(x, z) {
        Some(c) => Some(shore(st, map, x, z, c.height, c.biome)),
        None if st.cfg.outside == Outside::Ocean => Some((st.cfg.ocean_floor, OCEAN)),
        None => None,
    }
}

/// The sea level of the map (where its water stands), when there is a map.
pub fn sea_level() -> Option<i32> {
    state().map.as_ref().map(|m| m.info.sea_level)
}

/// [`ground`] for the generator's own queries (the terrain under structures, the spawn
/// search): also says once in the log that they read the map.
pub fn structure_ground(dimension: &str, flat: bool, x: i32, z: i32) -> Option<(i32, u8)> {
    let g = ground(dimension, flat, x, z)?;
    if !STRUCTURES_SEEN.swap(true, Ordering::Relaxed) {
        log::info!(target: LOG, "the generator reads the map's heights and biomes");
    }
    Some(g)
}

std::thread_local! {
    /// The dimension whose generator this thread is building.
    static BUILDING: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// A generator for `dimension` is being built on this thread.
pub fn set_building(dimension: &str) {
    BUILDING.with(|b| *b.borrow_mut() = Some(dimension.to_owned()));
}

/// The dimension being built on this thread, when it is the map's and there is a map.
pub fn building_ours() -> Option<String> {
    let dimension = BUILDING.with(|b| b.borrow().clone())?;
    let st = state();
    (dimension == st.cfg.dimension && st.map.is_some()).then_some(dimension)
}

/// Biomes no map column stands for: the Nether's and the End's (left as they are).
fn other_dimension(biome: &str) -> bool {
    matches!(
        biome,
        "minecraft:nether_wastes"
            | "minecraft:soul_sand_valley"
            | "minecraft:crimson_forest"
            | "minecraft:warped_forest"
            | "minecraft:basalt_deltas"
            | "minecraft:the_end"
            | "minecraft:end_highlands"
            | "minecraft:end_midlands"
            | "minecraft:small_end_islands"
            | "minecraft:end_barrens"
    )
}

/// Vanilla's cave biomes, kept deep under the map's ground.
fn cave(biome: &str) -> bool {
    matches!(biome, "minecraft:lush_caves" | "minecraft:dripstone_caves" | "minecraft:deep_dark")
}

/// How far under the map's ground a cave biome of vanilla's is kept.
pub const CAVE_DEPTH: i32 = 12;

/// The biome a multi-noise biome source gives at block `[x, y, z]` instead of `vanilla`: the
/// map's biome of the column, or None to keep vanilla's (another dimension's biome, a cave
/// biome deep enough under the ground, no map there).
pub fn biome_at(vanilla: &str, x: i32, y: i32, z: i32) -> Option<&'static str> {
    if other_dimension(vanilla) {
        return None;
    }
    let st = state();
    let (top, biome) = ground(&st.cfg.dimension, false, x, z)?;
    if cave(vanilla) && y < top - CAVE_DEPTH {
        return None;
    }
    Some(column::biome_name(biome))
}

/// The six climate parameters of the multi-noise biome search (temperature, humidity,
/// continentalness, erosion, depth, weirdness), quantized as vanilla does (x 10000).
pub type Climate = [i64; 6];
/// A biome's climate box: `[min, max]` per parameter.
pub type ClimateBox = [[i64; 2]; 6];

/// What the map's dimension's multi-noise biome source can answer: for each biome, a climate
/// at the middle of one of its surface boxes (the search answers exactly that biome there),
/// and the boxes of the cave biomes (under the surface only).
struct BiomeTable {
    surface: std::collections::HashMap<String, Climate>,
    caves: Vec<ClimateBox>,
}

static TABLE: std::sync::RwLock<Option<BiomeTable>> = std::sync::RwLock::new(None);
/// The climate samplers of the map's dimension (an address of their first field each).
static CLIMATES: std::sync::RwLock<Vec<usize>> = std::sync::RwLock::new(Vec::new());
static CLIMATE_SEEN: AtomicBool = AtomicBool::new(false);

/// The biome source of a generator being built on this thread: its parameter list, when it
/// is the map's dimension's.
pub fn register_biomes(entries: &[(ClimateBox, &str)]) {
    if building_ours().is_none() {
        return;
    }
    let mut surface = std::collections::HashMap::new();
    let mut caves = Vec::new();
    for (bounds, name) in entries {
        let [lo, hi] = bounds[4];
        if lo <= 0 && 0 <= hi {
            surface
                .entry((*name).to_owned())
                .or_insert_with(|| bounds.map(|[lo, hi]| lo / 2 + hi / 2));
        } else if lo > 0 {
            caves.push(*bounds);
        }
    }
    *TABLE.write().unwrap_or_else(|e| e.into_inner()) = Some(BiomeTable { surface, caves });
}

/// The climate sampler of a generator being built on this thread (`id`: the address of its
/// first field), when it is the map's dimension's.
pub fn register_climate(id: usize) {
    if building_ours().is_some() {
        let mut climates = CLIMATES.write().unwrap_or_else(|e| e.into_inner());
        if !climates.contains(&id) {
            climates.push(id);
        }
    }
}

/// Whether the climate sampler `id` is the map's dimension's.
pub fn is_map_climate(id: usize) -> bool {
    CLIMATES.read().unwrap_or_else(|e| e.into_inner()).contains(&id)
}

/// The climate the biome search gets at quart `[x, y, z]` (4 blocks a step) instead of
/// `vanilla`, so it answers the map's biome of the column; None keeps vanilla's (no map
/// there, a cave biome deeper than [`CAVE_DEPTH`] under the map's ground, a biome the
/// source does not have).
pub fn map_climate(quart: [i32; 3], vanilla: Climate) -> Option<Climate> {
    let [qx, qy, qz] = quart;
    let (x, y, z) = ((qx << 2) + 2, qy << 2, (qz << 2) + 2);
    let st = state();
    let (top, biome) = ground(&st.cfg.dimension, false, x, z)?;
    let table = TABLE.read().unwrap_or_else(|e| e.into_inner());
    let table = table.as_ref()?;
    if y < top - CAVE_DEPTH && table.caves.iter().any(|b| inside(b, vanilla)) {
        return None;
    }
    let climate = table.surface.get(column::biome_name(biome)).copied();
    if climate.is_some() && !CLIMATE_SEEN.swap(true, Ordering::Relaxed) {
        log::info!(target: LOG, "the biome source answers the map's biomes");
    }
    climate
}

fn inside(bounds: &ClimateBox, climate: Climate) -> bool {
    bounds.iter().zip(climate).all(|([lo, hi], v)| *lo <= v && v <= *hi)
}

/// [`map_climate`] for a climate as the bulk sampler keeps it (unquantized floats).
pub fn map_climate_values(quart: [i32; 3], values: &mut [f32; 6]) {
    let vanilla = values.map(|v| (v * 10_000.0) as i64);
    if let Some(climate) = map_climate(quart, vanilla) {
        // Half a step away from zero, so quantizing (which truncates) gives `v` back.
        *values = climate.map(|v| ((v as f64 + 0.5 * v.signum() as f64) / 10_000.0) as f32);
    }
}

/// What a structure query reads at height `y` of a column whose ground ends below `top`
/// (with the map's sea level): solid ground, water, or air.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Ground,
    Water,
    Air,
}

pub fn fill(top: i32, sea_level: i32, y: i32) -> Fill {
    if y < top {
        Fill::Ground
    } else if y < sea_level {
        Fill::Water
    } else {
        Fill::Air
    }
}
