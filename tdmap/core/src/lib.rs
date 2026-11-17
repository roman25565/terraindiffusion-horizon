//! tdmap rules with no game types: tested on their own by `core-tests/`.
//!
//! - `config`: `config/tdmap.toml`.
//! - `map`: the baked map (`map.toml`, `tiles/<tx>_<tz>.bin`) and its tile cache.
//! - `column`: which block stands at each height of a column (terrain, water, surface).
//! - `world`: the map and config the server loaded, shared by the chunk hooks and the
//!   structure queries, and each column's ground as both see it.
pub mod column;
pub mod config;
pub mod map;
pub mod world;
