//! The map tdbake writes: `map.toml` and `tiles/<tx>_<tz>.bin`, each tile 256 x 256 blocks:
//! the magic `TDMAP1\0\0`, then the surface height of each column (Minecraft Y of the first
//! air block over the ground, i16 little endian, row by row: z then x), then its biome
//! (Terrain Diffusion's biome number, one byte each).
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const TILE: i32 = 256;
pub const MAGIC: &[u8; 8] = b"TDMAP1\0\0";
const CELLS: usize = (TILE * TILE) as usize;
pub const TILE_BYTES: usize = MAGIC.len() + CELLS * 2 + CELLS;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MapInfo {
    pub format: u32,
    pub seed: i64,
    pub scale: u32,
    pub tile_size: i32,
    pub sea_level: i32,
    pub tile_x_min: i32,
    pub tile_z_min: i32,
    pub tile_x_max: i32,
    pub tile_z_max: i32,
}

impl MapInfo {
    pub fn parse(text: &str) -> Result<MapInfo, String> {
        let info: MapInfo = toml::from_str(text).map_err(|e| e.to_string())?;
        if info.format != 1 {
            return Err(format!("map format {} (this mod reads 1)", info.format));
        }
        if info.tile_size != TILE {
            return Err(format!("tile_size {} (this mod reads {TILE})", info.tile_size));
        }
        if info.tile_x_max <= info.tile_x_min || info.tile_z_max <= info.tile_z_min {
            return Err("empty map".into());
        }
        Ok(info)
    }
    /// Whether tile `[tx, tz]` lies in the baked area.
    pub fn has_tile(&self, tx: i32, tz: i32) -> bool {
        (self.tile_x_min..self.tile_x_max).contains(&tx) && (self.tile_z_min..self.tile_z_max).contains(&tz)
    }
    /// How many blocks block `[x, z]` lies inside the baked area: 0 on its outermost
    /// columns, negative outside.
    pub fn edge_distance(&self, x: i32, z: i32) -> i32 {
        let (x1, z1, x2, z2) = self.blocks();
        (x - x1).min(x2 - 1 - x).min(z - z1).min(z2 - 1 - z)
    }
    /// The baked area in blocks: `(x_min, z_min, x_max, z_max)`, max exclusive.
    pub fn blocks(&self) -> (i32, i32, i32, i32) {
        (self.tile_x_min * TILE, self.tile_z_min * TILE, self.tile_x_max * TILE, self.tile_z_max * TILE)
    }
}

/// One tile's columns.
pub struct Tile {
    heights: Box<[i16]>,
    biomes: Box<[u8]>,
}

impl Tile {
    pub fn parse(bytes: &[u8]) -> Result<Tile, String> {
        if bytes.len() != TILE_BYTES {
            return Err(format!("{} bytes, expected {TILE_BYTES}", bytes.len()));
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err("not a tdmap tile".into());
        }
        let h = &bytes[MAGIC.len()..MAGIC.len() + CELLS * 2];
        let heights = h.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let biomes = bytes[MAGIC.len() + CELLS * 2..].to_vec().into_boxed_slice();
        Ok(Tile { heights, biomes })
    }
    /// Encodes a tile (tests, tools): `heights` and `biomes` row by row, z then x.
    #[allow(dead_code)]
    pub fn encode(heights: &[i16], biomes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(TILE_BYTES);
        out.extend_from_slice(MAGIC);
        for h in heights {
            out.extend_from_slice(&h.to_le_bytes());
        }
        out.extend_from_slice(biomes);
        out
    }
    /// Column at `[lx, lz]` inside the tile (0..256).
    pub fn column(&self, lx: i32, lz: i32) -> Column {
        let i = (lz * TILE + lx) as usize;
        Column { height: i32::from(self.heights[i]), biome: self.biomes[i] }
    }
}

/// A map column: blocks below `height` are ground, `biome` is Terrain Diffusion's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub height: i32,
    pub biome: u8,
}

struct Slot {
    tile: Option<Arc<Tile>>,
    used: AtomicU64,
}

/// The map on disk, read tile by tile on demand and kept in an LRU cache. Shared by the
/// generation workers (several threads at once).
pub struct Map {
    pub info: MapInfo,
    dir: PathBuf,
    cap: usize,
    clock: AtomicU64,
    tiles: Mutex<HashMap<(i32, i32), Slot>>,
    problems: Mutex<Vec<String>>,
}

impl Map {
    /// Opens the map in `dir` (its `map.toml`); tiles are read when first needed.
    pub fn open(dir: &Path, cap: usize) -> Result<Map, String> {
        let meta = dir.join("map.toml");
        let text = fs::read_to_string(&meta).map_err(|e| format!("{}: {e}", meta.display()))?;
        let info = MapInfo::parse(&text).map_err(|e| format!("{}: {e}", meta.display()))?;
        Ok(Map {
            info,
            dir: dir.to_owned(),
            cap: cap.max(16),
            clock: AtomicU64::new(0),
            tiles: Mutex::new(HashMap::new()),
            problems: Mutex::new(Vec::new()),
        })
    }

    /// The tile holding block `[x, z]`, None outside the map or when the tile is not baked.
    pub fn tile_at(&self, x: i32, z: i32) -> Option<Arc<Tile>> {
        let key = (x.div_euclid(TILE), z.div_euclid(TILE));
        if !self.info.has_tile(key.0, key.1) {
            return None;
        }
        let now = self.clock.fetch_add(1, Ordering::Relaxed);
        {
            let tiles = self.tiles.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(slot) = tiles.get(&key) {
                slot.used.store(now, Ordering::Relaxed);
                return slot.tile.clone();
            }
        }
        let tile = self.read(key).map(Arc::new);
        let mut tiles = self.tiles.lock().unwrap_or_else(|p| p.into_inner());
        if tiles.len() >= self.cap {
            let mut ages: Vec<((i32, i32), u64)> =
                tiles.iter().map(|(k, s)| (*k, s.used.load(Ordering::Relaxed))).collect();
            ages.sort_by_key(|a| a.1);
            for (k, _) in ages.into_iter().take(self.cap / 8 + 1) {
                tiles.remove(&k);
            }
        }
        let slot = tiles.entry(key).or_insert(Slot { tile, used: AtomicU64::new(now) });
        slot.tile.clone()
    }

    /// The column at block `[x, z]`, None outside the baked map.
    pub fn column(&self, x: i32, z: i32) -> Option<Column> {
        self.tile_at(x, z).map(|t| t.column(x.rem_euclid(TILE), z.rem_euclid(TILE)))
    }

    fn read(&self, (tx, tz): (i32, i32)) -> Option<Tile> {
        let path = self.dir.join("tiles").join(format!("{tx}_{tz}.bin"));
        let bytes = fs::read(&path).ok()?;
        match Tile::parse(&bytes) {
            Ok(t) => Some(t),
            Err(e) => {
                self.problems.lock().unwrap_or_else(|p| p.into_inner()).push(format!("{}: {e}", path.display()));
                None
            }
        }
    }

    /// Problems met reading tiles since the last call (for the log).
    pub fn take_problems(&self) -> Vec<String> {
        std::mem::take(&mut *self.problems.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// How many tiles of the baked area exist on disk.
    pub fn count_baked(&self) -> usize {
        let i = &self.info;
        let mut n = 0;
        for tz in i.tile_z_min..i.tile_z_max {
            for tx in i.tile_x_min..i.tile_x_max {
                if fs::metadata(self.dir.join("tiles").join(format!("{tx}_{tz}.bin")))
                    .is_ok_and(|m| m.len() == TILE_BYTES as u64)
                {
                    n += 1;
                }
            }
        }
        n
    }
}
