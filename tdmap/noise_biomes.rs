// The biomes a structure checks (Structure.isValidBiome at its start, the biome checks of its
// pieces): in the map's dimension, the map's biome of each quart's column instead of the
// biome source's, so a desert pyramid stands in the map's desert.
fn tdmap_noise_biomes(
    dimension: &str,
    flat: bool,
    data: &GeneratorData,
    quarts: &[[i32; 3]],
    value: Vec<BiomeId>,
) -> Vec<BiomeId> {
    quarts
        .iter()
        .zip(value)
        .map(|(q, vanilla)| {
            tdmap_core::world::structure_biome(dimension, flat, (q[0] << 2) + 2, (q[2] << 2) + 2)
                .and_then(|name| data.biome_id(name))
                .unwrap_or(vanilla)
        })
        .collect()
}
