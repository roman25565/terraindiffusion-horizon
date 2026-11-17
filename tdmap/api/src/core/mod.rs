//! tdmap rules with no game types: tested on their own by `core-tests/`.
//!
//! - `config`: `config/tdmap.toml`.
//! - `map`: the baked map (`map.toml`, `tiles/<tx>_<tz>.bin`) and its tile cache.
//! - `column`: which block stands at each height of a column (terrain, water, surface).
pub mod column;
pub mod config;
pub mod map;
