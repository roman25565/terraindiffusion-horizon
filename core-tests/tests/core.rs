use std::fs;
use std::path::PathBuf;
use tdmap_core::column::{self, Mat, Plan};
use tdmap_core::config::{self, Config, Outside};
use tdmap_core::map::{Map, MapInfo, Tile, TILE, TILE_BYTES};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tdmap-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

const META: &str = "format = 1\nseed = -5408366058459925370\nscale = 1\nmeters_per_block = 30.0\ntile_size = 256\nsea_level = 63\ntile_x_min = -1\ntile_z_min = 0\ntile_x_max = 1\ntile_z_max = 1\n";

/// A tile whose height is 40 + x/4 (an ocean to the west rising to hills) and biome by column.
fn sample_tile() -> Vec<u8> {
    let mut heights = Vec::new();
    let mut biomes = Vec::new();
    for z in 0..TILE {
        for x in 0..TILE {
            heights.push((40 + x / 4) as i16);
            biomes.push(if z < 128 { 5 } else { 115 });
        }
    }
    Tile::encode(&heights, &biomes)
}

#[test]
fn default_config_is_the_shipped_file() {
    let d = temp("cfg");
    let (c, problems) = config::load(&d);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(c, Config::default());
    assert!(d.join("tdmap.toml").exists(), "the defaults are written");
    fs::write(d.join("tdmap.toml"), "map = 'F:\\\\maps\\\\td'\noutside = \"vanilla\"\ncache_tiles = 2\n").unwrap();
    let (c, problems) = config::load(&d);
    assert_eq!(c.outside, Outside::Vanilla);
    assert_eq!(c.cache_tiles, 16, "too small a cache is raised");
    assert_eq!(problems.len(), 1);
    fs::write(d.join("tdmap.toml"), "outside = 5\n").unwrap();
    let (c, problems) = config::load(&d);
    assert_eq!(c, Config::default());
    assert_eq!(problems.len(), 1, "a broken file falls back to the defaults and says so");
}

#[test]
fn map_info_matches_what_tdbake_writes() {
    let i = MapInfo::parse(META).unwrap();
    assert_eq!(i.seed, -5408366058459925370);
    assert_eq!(i.blocks(), (-256, 0, 256, 256));
    assert!(i.has_tile(-1, 0) && i.has_tile(0, 0) && !i.has_tile(1, 0) && !i.has_tile(0, -1));
    assert!(MapInfo::parse(&META.replace("format = 1", "format = 2")).is_err());
    assert!(MapInfo::parse(&META.replace("tile_size = 256", "tile_size = 128")).is_err());
}

#[test]
fn tiles_read_columns_and_missing_tiles_are_outside() {
    let d = temp("map");
    fs::create_dir_all(d.join("tiles")).unwrap();
    fs::write(d.join("map.toml"), META).unwrap();
    let bytes = sample_tile();
    assert_eq!(bytes.len(), TILE_BYTES);
    fs::write(d.join("tiles").join("0_0.bin"), &bytes).unwrap();
    fs::write(d.join("tiles").join("-1_0.bin"), &bytes[..100]).unwrap();
    let map = Map::open(&d, 16).unwrap();
    assert_eq!(map.count_baked(), 1);
    let c = map.column(8, 3).unwrap();
    assert_eq!((c.height, c.biome), (42, 5));
    assert_eq!(map.column(255, 200).unwrap().height, 40 + 255 / 4);
    assert!(map.column(-5, 3).is_none(), "a broken tile reads as outside");
    assert_eq!(map.take_problems().len(), 1, "and is reported");
    assert!(map.column(300, 3).is_none(), "outside the baked area");
    assert!(map.column(3, -1).is_none());
}

#[test]
fn cache_stays_bounded() {
    let d = temp("cache");
    fs::create_dir_all(d.join("tiles")).unwrap();
    let meta = META.replace("tile_x_max = 1", "tile_x_max = 40");
    fs::write(d.join("map.toml"), meta).unwrap();
    for tx in 0..40 {
        fs::write(d.join("tiles").join(format!("{tx}_0.bin")), sample_tile()).unwrap();
    }
    let map = Map::open(&d, 16).unwrap();
    for round in 0..3 {
        for tx in 0..40 {
            assert_eq!(map.column(tx * TILE + 4, 0).unwrap().height, 41, "round {round}");
        }
    }
}

#[test]
fn column_layers_follow_terrain_diffusion_preset() {
    // Desert column, ground up to y 80 (top block y 79).
    let p = Plan::new(10, 20, 80, 5, -64, 384, 63);
    assert_eq!(p.block(80), None, "air above");
    assert_eq!(p.block(79), Some(Mat::Sand));
    assert_eq!(p.block(75), Some(Mat::Sandstone));
    assert_eq!(p.block(74), Some(Mat::Stone));
    assert_eq!(p.block(8), Some(Mat::Stone));
    assert_eq!(p.block(0), Some(Mat::Deepslate));
    assert_eq!(p.block(-60), Some(Mat::Bedrock));
    assert_eq!(p.block(-64), Some(Mat::Bedrock));
    assert_eq!(p.block(-65), None, "below the level");
    // Plains under the sea: dirt, water above up to y 62.
    let p = Plan::new(0, 0, 50, 1, -64, 384, 63);
    assert_eq!(p.block(49), Some(Mat::Dirt));
    assert_eq!(p.block(50), Some(Mat::Water));
    assert_eq!(p.block(62), Some(Mat::Water));
    assert_eq!(p.block(63), None);
    // Plains above y 62: grass.
    assert_eq!(Plan::new(0, 0, 70, 1, -64, 384, 63).block(69), Some(Mat::GrassBlock));
    // Peaks: snow block over stone.
    let p = Plan::new(0, 0, 300, 33, -64, 384, 63);
    assert_eq!((p.block(299), p.block(298)), (Some(Mat::SnowBlock), Some(Mat::Stone)));
    // Cold ocean floor: gravel.
    assert_eq!(Plan::new(0, 0, 30, 46, -64, 384, 63).block(29), Some(Mat::Gravel));
}

#[test]
fn gradients_mix_in_between_and_repeat() {
    let mut deepslate = 0;
    let mut bedrock = 0;
    for x in 0..64 {
        for z in 0..64 {
            let p = Plan::new(x, z, 100, 1, -64, 384, 63);
            deepslate += (p.block(4) == Some(Mat::Deepslate)) as i32;
            bedrock += (p.block(-58) == Some(Mat::Bedrock)) as i32;
            assert_eq!(p.block(4), Plan::new(x, z, 100, 1, -64, 384, 63).block(4), "same answer each time");
        }
    }
    // y 4 of 0..8: chance 1/2; y -58 of -60..-55: chance 3/5.
    assert!((1700..2400).contains(&deepslate), "{deepslate}");
    assert!((2100..2800).contains(&bedrock), "{bedrock}");
}

#[test]
fn ground_is_cut_to_the_level() {
    let p = Plan::new(0, 0, 900, 35, -64, 384, 63);
    assert_eq!(p.top, 319);
    assert_eq!(p.block(318), Some(Mat::Stone));
    assert_eq!(p.block(319), None);
    let p = Plan::new(0, 0, -200, 44, -64, 384, 63);
    assert_eq!(p.top, -63, "one block of floor at least");
}

#[test]
fn runs_cover_every_block_once() {
    for (h, b) in [(80, 5), (50, 46), (200, 32), (63, 1), (-40, 44)] {
        let p = Plan::new(7, 9, h, b, -64, 384, 63);
        let runs = p.runs();
        let mut y = -64;
        for (lo, hi, m) in &runs {
            assert!(lo < hi);
            assert!(*lo >= y);
            for yy in *lo..*hi {
                assert_eq!(p.block(yy), Some(*m), "h {h} y {yy}");
            }
            y = *hi;
        }
        assert_eq!(y, p.top.max(63));
        assert_eq!(p.block(y), None);
    }
}

#[test]
fn far_runs_show_the_top_and_the_water() {
    let p = Plan::new(7, 9, 50, 46, -64, 384, 63);
    let r = p.far_runs();
    assert_eq!(r[0], (50, 63, Mat::Water));
    assert_eq!(r[1].2, Mat::Gravel);
    assert_eq!(r[1].1, 50);
    assert_eq!(r.last().unwrap().0, -64);
    let p = Plan::new(7, 9, 120, 1, -64, 384, 63);
    let r = p.far_runs();
    assert_eq!(r[0], (119, 120, Mat::GrassBlock));
    assert_eq!(r[1], (115, 119, Mat::Dirt));
    assert_eq!(r[2], (-64, 115, Mat::Stone));
}

#[test]
fn every_biome_number_is_a_vanilla_biome() {
    for id in [1u8, 3, 5, 6, 8, 15, 16, 17, 19, 23, 26, 29, 31, 32, 33, 35, 41, 44, 46, 48, 108, 115, 116] {
        let name = column::biome_name(id);
        assert_eq!(column::BIOMES[column::biome_index(id)], name);
    }
    assert_eq!(column::biome_name(115), "minecraft:taiga");
    assert_eq!(column::biome_name(200), "minecraft:plains");
}

/// `map.toml` exactly as tdbake's `MapMeta.write` produced it (scale 3, 40 x 40 tiles).
#[test]
fn reads_the_bakers_own_map_toml() {
    let text = r#"# Terrain Diffusion map baked by tdbake for the Horizon tdmap mod. Do not edit.
format = 1
seed = -5408366058459925370
scale = 3
meters_per_block = 10.0
tile_size = 256
sea_level = 63
# Tiles tile_x_min..tile_x_max-1, tile_z_min..tile_z_max-1: blocks x -5120..5119, z -5120..5119
tile_x_min = -20
tile_z_min = -20
tile_x_max = 20
tile_z_max = 20
"#;
    let i = MapInfo::parse(text).unwrap();
    assert_eq!((i.scale, i.tile_x_min, i.tile_z_max), (3, -20, 20));
    assert_eq!(i.blocks(), (-5120, -5120, 5120, 5120));
}

#[test]
fn the_map_ends_in_a_coast_not_a_wall() {
    let i = MapInfo::parse(META).unwrap();
    assert_eq!(i.edge_distance(-256, 100), 0);
    assert_eq!(i.edge_distance(255, 100), 0);
    assert_eq!(i.edge_distance(0, 100), 100);
    assert_eq!(i.edge_distance(300, 100), -45);
    // Deep inside: unchanged.
    assert_eq!(column::blend_to_edge(180, 15, 256, 256, 40, 63), (180, 15));
    assert_eq!(column::blend_to_edge(180, 15, 1000, 256, 40, 63), (180, 15));
    // At the edge: the ocean floor, as an ocean.
    assert_eq!(column::blend_to_edge(180, 15, 0, 256, 40, 63), (40, column::OCEAN));
    // Sinking all the way: no step between neighbours bigger than a few blocks.
    let mut last = column::blend_to_edge(180, 15, 0, 256, 40, 63).0;
    for d in 1..300 {
        let (h, _) = column::blend_to_edge(180, 15, d, 256, 40, 63);
        assert!(h >= last && h - last <= 2, "d {d}: {last} -> {h}");
        last = h;
    }
    // Land still above the sea keeps its biome; 0 turns the blend off.
    assert_eq!(column::blend_to_edge(180, 15, 200, 256, 40, 63).1, 15);
    assert_eq!(column::blend_to_edge(180, 15, 0, 0, 40, 63), (180, 15));
}

#[test]
fn structure_fill_is_ground_then_water_then_air() {
    use tdmap_core::world::{Fill, fill};
    // Land at 80 above the sea at 63.
    assert_eq!(fill(80, 63, 79), Fill::Ground);
    assert_eq!(fill(80, 63, 80), Fill::Air);
    // Sea floor at 40: water up to 62.
    assert_eq!(fill(40, 63, 39), Fill::Ground);
    assert_eq!(fill(40, 63, 40), Fill::Water);
    assert_eq!(fill(40, 63, 62), Fill::Water);
    assert_eq!(fill(40, 63, 63), Fill::Air);
}
