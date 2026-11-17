//! A map column turned into blocks, as Terrain Diffusion's own world preset builds it
//! (`terrain_diffusion.json` noise settings of the Fabric mod 2.2.0):
//!
//! - ground below the column's height, water from there up to the sea level (no aquifers);
//! - bedrock floor from y -60 (always) fading out by -55, deepslate from y 0 fading out by 8;
//! - the surface rule: the top block and the 4 under it by biome (sand in deserts, snow
//!   blocks on peaks, gravel under cold seas, grass above y 62, dirt elsewhere).
//!
//! Biomes are the mod's numbers; its three "sparse" biomes don't exist in Horizon (a data
//! pack's new biomes are left out), so they become their vanilla originals.

/// Terrain Diffusion's biome number -> the vanilla biome it stands for.
pub fn biome_name(id: u8) -> &'static str {
    match id {
        1 => "minecraft:plains",
        3 => "minecraft:snowy_plains",
        5 => "minecraft:desert",
        6 => "minecraft:swamp",
        8 | 108 => "minecraft:forest",
        15 | 115 => "minecraft:taiga",
        16 | 116 => "minecraft:snowy_taiga",
        17 => "minecraft:savanna",
        19 => "minecraft:windswept_hills",
        23 => "minecraft:jungle",
        26 => "minecraft:badlands",
        29 => "minecraft:meadow",
        31 => "minecraft:grove",
        32 => "minecraft:snowy_slopes",
        33 => "minecraft:frozen_peaks",
        35 => "minecraft:stony_peaks",
        41 => "minecraft:warm_ocean",
        44 => "minecraft:ocean",
        46 => "minecraft:cold_ocean",
        48 => "minecraft:frozen_ocean",
        _ => "minecraft:plains",
    }
}

/// Every biome a map can hold (index into [`biome_index`]).
pub const BIOMES: [&str; 20] = [
    "minecraft:plains",
    "minecraft:snowy_plains",
    "minecraft:desert",
    "minecraft:swamp",
    "minecraft:forest",
    "minecraft:taiga",
    "minecraft:snowy_taiga",
    "minecraft:savanna",
    "minecraft:windswept_hills",
    "minecraft:jungle",
    "minecraft:badlands",
    "minecraft:meadow",
    "minecraft:grove",
    "minecraft:snowy_slopes",
    "minecraft:frozen_peaks",
    "minecraft:stony_peaks",
    "minecraft:warm_ocean",
    "minecraft:ocean",
    "minecraft:cold_ocean",
    "minecraft:frozen_ocean",
];

/// The position of a map biome in [`BIOMES`].
pub fn biome_index(id: u8) -> usize {
    let name = biome_name(id);
    BIOMES.iter().position(|b| *b == name).unwrap_or(0)
}

/// Terrain Diffusion's number for vanilla's ocean (the area outside the map).
pub const OCEAN: u8 = 44;

/// A column `distance` blocks inside the map's edge, sunk towards `floor` over the last `blend`
/// blocks (smoothly: no change at `blend` and further in, the ocean floor at the edge). Land that
/// ends up under the sea becomes ocean. Returns the height and the biome.
pub fn blend_to_edge(height: i32, biome: u8, distance: i32, blend: i32, floor: i32, sea_level: i32) -> (i32, u8) {
    if blend <= 0 || distance >= blend {
        return (height, biome);
    }
    let t = (distance.max(0) as f32 + 0.5) / blend as f32;
    let t = t * t * (3.0 - 2.0 * t);
    let h = floor as f32 + (height - floor) as f32 * t;
    let h = h.round() as i32;
    let ocean = matches!(biome, 41 | 44 | 46 | 48);
    if h < sea_level - 1 && !ocean {
        (h, OCEAN)
    } else {
        (h, biome)
    }
}

/// The blocks a column is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mat {
    Stone,
    Deepslate,
    Bedrock,
    Water,
    GrassBlock,
    Dirt,
    Sand,
    RedSand,
    Sandstone,
    Terracotta,
    SnowBlock,
    Gravel,
}

impl Mat {
    pub const ALL: [Mat; 12] = [
        Mat::Stone,
        Mat::Deepslate,
        Mat::Bedrock,
        Mat::Water,
        Mat::GrassBlock,
        Mat::Dirt,
        Mat::Sand,
        Mat::RedSand,
        Mat::Sandstone,
        Mat::Terracotta,
        Mat::SnowBlock,
        Mat::Gravel,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Mat::Stone => "minecraft:stone",
            Mat::Deepslate => "minecraft:deepslate",
            Mat::Bedrock => "minecraft:bedrock",
            Mat::Water => "minecraft:water",
            Mat::GrassBlock => "minecraft:grass_block",
            Mat::Dirt => "minecraft:dirt",
            Mat::Sand => "minecraft:sand",
            Mat::RedSand => "minecraft:red_sand",
            Mat::Sandstone => "minecraft:sandstone",
            Mat::Terracotta => "minecraft:terracotta",
            Mat::SnowBlock => "minecraft:snow_block",
            Mat::Gravel => "minecraft:gravel",
        }
    }
    pub fn index(self) -> usize {
        Mat::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// The surface rule's top block (depth 0) and the blocks under it (depth 1..=4).
fn surface(biome: &str, y: i32, depth: i32) -> Option<Mat> {
    if depth == 0 {
        Some(match biome {
            "minecraft:desert" => Mat::Sand,
            "minecraft:badlands" => Mat::RedSand,
            "minecraft:snowy_slopes" | "minecraft:frozen_peaks" => Mat::SnowBlock,
            "minecraft:stony_peaks" | "minecraft:windswept_hills" => Mat::Stone,
            "minecraft:warm_ocean" | "minecraft:ocean" => Mat::Sand,
            "minecraft:cold_ocean" | "minecraft:frozen_ocean" => Mat::Gravel,
            _ if y >= 62 => Mat::GrassBlock,
            _ => Mat::Dirt,
        })
    } else if depth <= 4 {
        Some(match biome {
            "minecraft:desert" => Mat::Sandstone,
            "minecraft:badlands" => Mat::Terracotta,
            "minecraft:snowy_slopes"
            | "minecraft:frozen_peaks"
            | "minecraft:stony_peaks"
            | "minecraft:windswept_hills" => Mat::Stone,
            "minecraft:warm_ocean" => Mat::Sand,
            "minecraft:ocean" | "minecraft:cold_ocean" | "minecraft:frozen_ocean" => Mat::Gravel,
            _ => Mat::Dirt,
        })
    } else {
        None
    }
}

/// A random number in 0..1 fixed by the position (the gradients' noise; the same each time a
/// chunk is asked again).
fn unit(x: i32, y: i32, z: i32, salt: u64) -> f32 {
    let mut h = (x as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u32 as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ salt;
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Vanilla's `vertical_gradient`: always at and below `true_at`, never at and above
/// `false_at`, in between with a chance falling linearly.
fn gradient(x: i32, y: i32, z: i32, true_at: i32, false_at: i32, salt: u64) -> bool {
    if y <= true_at {
        return true;
    }
    if y >= false_at {
        return false;
    }
    let chance = (false_at - y) as f32 / (false_at - true_at) as f32;
    unit(x, y, z, salt) < chance
}

const BEDROCK_SALT: u64 = 0xB3D0_C4F1_0000_0001;
const DEEPSLATE_SALT: u64 = 0xD33F_5A7E_0000_0002;

/// One column ready to build: ground below `top` (exclusive), water from `top` up to the sea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub x: i32,
    pub z: i32,
    /// First Y above the ground, already inside the level.
    pub top: i32,
    pub biome: &'static str,
    pub min_y: i32,
    pub sea_level: i32,
}

impl Plan {
    /// The column at `[x, z]` with ground up to `height`, in a level spanning `min_y` and
    /// `height_of_level` blocks: the ground is cut to the level (one block of floor at least,
    /// the top block below the build limit).
    pub fn new(x: i32, z: i32, height: i32, biome_id: u8, min_y: i32, level_height: i32, sea_level: i32) -> Plan {
        let max = min_y + level_height - 1;
        Plan { x, z, top: height.clamp(min_y + 1, max), biome: biome_name(biome_id), min_y, sea_level }
    }

    /// The block at `y` (None: air).
    pub fn block(&self, y: i32) -> Option<Mat> {
        if y < self.min_y {
            return None;
        }
        if y >= self.top {
            return (y < self.sea_level).then_some(Mat::Water);
        }
        if gradient(self.x, y, self.z, -60, -55, BEDROCK_SALT) {
            return Some(Mat::Bedrock);
        }
        if gradient(self.x, y, self.z, 0, 8, DEEPSLATE_SALT) {
            return Some(Mat::Deepslate);
        }
        let depth = self.top - 1 - y;
        Some(surface(self.biome, y, depth).unwrap_or(Mat::Stone))
    }

    /// The column as runs `(lowest y, highest y + 1, block)`, bottom up: what the far terrain
    /// draws and what a chunk gets.
    pub fn runs(&self) -> Vec<(i32, i32, Mat)> {
        let mut runs: Vec<(i32, i32, Mat)> = Vec::new();
        let end = self.top.max(self.sea_level);
        for y in self.min_y..end {
            let Some(m) = self.block(y) else { continue };
            match runs.last_mut() {
                Some(last) if last.1 == y && last.2 == m => last.1 = y + 1,
                _ => runs.push((y, y + 1, m)),
            }
        }
        runs
    }
    /// A cheap outline of [`Plan::runs`] for the far terrain, top down: the surface blocks
    /// and the water exactly, the rock under them as one run (the far terrain draws only
    /// what can be seen from above and from the side of a cliff).
    pub fn far_runs(&self) -> Vec<(i32, i32, Mat)> {
        let mut runs = Vec::with_capacity(4);
        if self.top < self.sea_level {
            runs.push((self.top, self.sea_level, Mat::Water));
        }
        let mut y = self.top - 1;
        while y >= self.min_y && y >= self.top - 5 {
            if let Some(m) = self.block(y) {
                match runs.last_mut() {
                    Some(last) if last.0 == y + 1 && last.2 == m => last.0 = y,
                    _ => runs.push((y, y + 1, m)),
                }
            }
            y -= 1;
        }
        if y >= self.min_y {
            runs.push((self.min_y, y + 1, Mat::Stone));
        }
        runs
    }
}
