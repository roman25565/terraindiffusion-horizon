//! `config/tdmap.toml`: written from the defaults in `data/tdmap.toml` when missing.
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_TEXT: &str = include_str!("../data/tdmap.toml");
pub const FILE: &str = "tdmap.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outside {
    Ocean,
    Vanilla,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub map: String,
    pub dimension: String,
    pub outside: Outside,
    pub ocean_floor: i32,
    pub edge_blend: i32,
    pub carvers: bool,
    pub decorations: bool,
    pub structures: bool,
    pub far_terrain: bool,
    pub cache_tiles: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            map: "tdmap".into(),
            dimension: "minecraft:overworld".into(),
            outside: Outside::Ocean,
            ocean_floor: 40,
            edge_blend: 256,
            carvers: true,
            decorations: true,
            structures: true,
            far_terrain: true,
            cache_tiles: 256,
        }
    }
}

impl Config {
    /// The map folder: `map` as written when absolute, else under `server_dir`.
    pub fn map_dir(&self, server_dir: &Path) -> PathBuf {
        let p = PathBuf::from(&self.map);
        if p.is_absolute() { p } else { server_dir.join(p) }
    }
}

/// Reads `dir/tdmap.toml` (writing the defaults there first when missing). Returns the config
/// and the problems met, one line each, for the log.
pub fn load(dir: &Path) -> (Config, Vec<String>) {
    let path = dir.join(FILE);
    let mut problems = Vec::new();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            if let Err(e) = fs::create_dir_all(dir).and_then(|()| fs::write(&path, DEFAULT_TEXT)) {
                problems.push(format!("could not write {}: {e}", path.display()));
            }
            DEFAULT_TEXT.to_owned()
        }
    };
    match toml::from_str::<Config>(&text) {
        Ok(mut c) => {
            if c.cache_tiles < 16 {
                problems.push(format!("cache_tiles = {} is too small, using 16", c.cache_tiles));
                c.cache_tiles = 16;
            }
            (c, problems)
        }
        Err(e) => {
            problems.push(format!("{}: {e}; using the defaults", path.display()));
            (Config::default(), problems)
        }
    }
}
