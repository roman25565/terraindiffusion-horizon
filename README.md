# Terrain Diffusion for Horizon (the cheap way)

Terrain and biomes from the [Terrain Diffusion](https://github.com/xandergos/terrain-diffusion-mc) neural network
in the overworld of a Horizon server, **without a GPU on the server**.

How it works:

1. **`tdbake.jar`** (Java, on your own PC with a GPU) computes a map once: the surface height and biome
   of every column. It is the same code and the same models as in the Fabric mod 2.2.0, just
   without Minecraft. The result is a folder with `map.toml` and `tiles/*.bin` files (256×256-block squares).
2. **The `tdmap` mod** (Rust, Horizon) reads that folder and builds every new chunk from it: land, water up to
   sea level, a surface as in Terrain Diffusion (sand in deserts, snow on peaks, gravel under
   cold seas, grass, bedrock, deepslate). Vanilla adds caves, ores, trees,
   grass, structures and mobs on top. Chunk biomes come from the map.

The Nether and the End are unchanged. Players install nothing.

## What's in the repository

| Path | What it is |
|---|---|
| `baker/` | source of `tdbake.jar` (Gradle): the Terrain Diffusion pipeline code (MIT) + `tdbake/Bake.java` (commands), `tdbake/Gui.java` (window) |
| `tdmap/` | the Horizon mod |
| `tdmap/core/` | rules without game types: the map, tile cache, column layers, config; shared by chunks and structures |
| `tdmap/*.rs` | mixins: the overworld generator takes terrain and biomes from the map (for structures, spawn) |
| `core-tests/` | `cargo test`: 13 core tests without the game |
| `test-server/install.ps1` | the author's install script (puts the mod in the mods folder and rebuilds the engine; edit the paths at the top) |

## 1. The tdbake window

Needs Java 21 (`java -version`). Double-click `tdbake.jar` (or `javaw -jar tdbake.jar`):
a window opens. It remembers everything you enter (`tdbake-gui.properties` next to the jar).
The window's labels are in Ukrainian; the English names below are translations.

Maps are saved to `maps\<map name>` next to the jar.

| Field | What it is |
|---|---|
| Map name | the folder name in `maps\` |
| Seed, Scale | the Terrain Diffusion world. Scale 1 = 30 m per block, mountains up to ~330 blocks above the sea (recommended); 2 = 15 m per block, mountains up to ~670 |
| Map center, Size | the middle of the map in blocks and the side of the square; on the right you see how many tiles, how much time and disk space it takes |
| Models | the Fabric mod's models folder, `%APPDATA%\.minecraft\terrain-diffusion-models` |
| Horizon server | the test server folder, for the connect button |

## 2. Choose a seed and a place

The **Explorer** button opens in the browser the same explorer as `/td-explore` in the mod. The
new-seed button lets you browse worlds; clicking the map shows coordinates. Copy the seed and center into the window.

## 3. Compute the map

The **Compute map** button. The bar shows tiles and the time left (on an RTX 3060 ~0.5 s per tile,
10240×10240 = 1600 tiles ≈ 13 min). You can make a 2048 trial map first.

- The map is computed from the center outwards, so even an unfinished one is usable around the center.
- **Stop** or closing the window doesn't damage the map: the same name, seed and scale continue from where
  they stopped. A bigger size with the same seed and scale extends the map.
- **View map** draws `preview.png` (1 pixel = 8 blocks), **Open map folder**
  opens it in Explorer.

The command line still works: `java -jar tdbake.jar explore|bake|preview ...`
(`java -jar tdbake.jar` with no arguments in a console without graphics prints help).

## 4. Mod and map on the server

The mod: copy the `tdmap` folder next to your other Horizon mods and rebuild the engine with them. It is made for Horizon
`fdc87ae` (nightly-2026-11-08).

The map: stop the server and press **Connect map to server** in the window. It writes
`map = '...'` into the server's `config\tdmap.toml` and puts the `tdmap-height` data pack (raises the top of the world for
the highest mountains, for scale 1: from y -64 to 415) into `<world>\datapacks\`. If the world already exists, the window
warns you: the map and height only apply to a new world.

## 5. A new world

The map only applies to **new** chunks: already generated ones don't change. So the world must be new:

1. Stop the server, rename the old world folder, then press **Connect map to server**
   (without the data pack peaks above y 319 are cut flat; the mod logs this).
2. Data packs: [`CustomWorldgen-26.3-vanilla-terrain`](https://github.com/roman25565/customworldgen-26.3) **is compatible** (ores, veins, carvers, more frequent
   witch huts and pyramids work; its cave zones don't, because they are in `final_density` and the terrain now comes from
   the map). Packs that change the terrain shape make no sense with a map. Structure packs work, but see the limitations.
3. Start the server. The log should contain `[horizon::tdmap] map ...\maps\<name>: seed ..., N of M tiles baked`.
4. Spawn point: vanilla picks it by its own biomes, so it may end up in the map's sea.
   Stand on land and run `/setworldspawn`.
5. A world border along the map edge, e.g. for 10240 centered at 0 0: `/worldborder center 0 0` and
   `/worldborder set 10240`.
6. Pregeneration as usual: `/pregen start ...`. It is faster than vanilla because the height doesn't need
   to be computed, only read.

## Settings in `config/tdmap.toml`

| Key | What it does |
|---|---|
| `map` | the map folder (relative to the server folder or a full path) |
| `outside` | `"ocean"` (open ocean beyond the map, floor at `ocean_floor`) or `"vanilla"` (vanilla terrain, there will be a seam) |
| `edge_blend` | how many blocks before the map edge land smoothly sinks into the ocean (256; 0 = a wall at the edge) |
| `carvers`, `decorations` | vanilla caves/canyons, ores/trees/grass on top of the map |
| `structures` | `false` removes all structures from the map (see limitations) |
| `far_terrain` | distant terrain (`horizon-lod-distance`) shows the map's heights |
| `cache_tiles` | how many tiles to keep in memory (256 ≈ 50 MB) |

## Limitations

- **Structures** (vanilla and from data packs) stand on the map's ground: the overworld generator reads
  the terrain from the map (`tdmap/density.rs`), so the map provides the height for structures, spawn search and distant
  terrain. Structure placement also follows **the map's biomes** (`tdmap/climate.rs`: the biome search gets the climate of the map's biome):
  a village only appears where the map has plains or savanna, not over water. Cave biomes deeper than
  12 blocks under the map's ground (deep dark, lush caves) stay vanilla, so ancient
  cities appear too. Vanilla slightly flattens the terrain under villages, this mod doesn't,
  so on a steep slope a house may hang a little. With the tdmap-height data pack the overworld noise becomes as
  tall as the world (`tdmap/noise_height.rs`), so structures on peaks above 320 stand on them too. Check: the server log at startup has
  `minecraft:overworld: the terrain queries read the map`.
- Terrain Diffusion's **"sparse" biomes** (`forest_sparse`, `taiga_sparse`, `snowy_taiga_sparse`)
  become ordinary forest, taiga and snowy taiga: Horizon doesn't add new biomes from data packs.
  So forests are denser than in the Fabric mod.
- Snow on high mountains in warm biomes falls by vanilla rules (the Fabric mod turned that off).
- The mod is verified by core tests and building; it still needs an in-game check on a test server.

## License and credits

The `tdmap` mod and `baker/src/main/java/tdbake/`: MIT, see [LICENSE](LICENSE).

The pipeline code in `baker/` comes from terrain-diffusion-mc (MIT, Alexander Goslin,
`baker/LICENSE-terrain-diffusion-mc.txt`); the models are downloaded from the author's HuggingFace.

Thanks to Alexander Goslin ([xandergos](https://github.com/xandergos)) for [Terrain Diffusion](https://github.com/xandergos/terrain-diffusion-mc) and the models. This project is unofficial and not supported by him.

`baker/libs/onnxruntime-dml.jar` (ONNX Runtime with DirectML, © Microsoft) is not in the repository: to build, take it from
the `libs/` folder of the terrain-diffusion-mc repository and put it in `baker/libs/`. Then `gradle jar` in `baker/` builds `tdbake.jar`.

A mod for [Horizon](https://github.com/horizon-rust/horizon) (MIT, Ludovic Marie and the Horizon contributors).

Unofficial project, not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB.
