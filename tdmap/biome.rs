// BiomeSourceResolver::sample (a multi-noise source, at a quart): the map's biome of the
// column instead of vanilla's, so structures (villages, temples, the data packs' ones) choose
// their place by the map's biomes. Nether and End biomes and deep cave biomes stay vanilla's.
fn tdmap_biome(
    resolver: &mut BiomeSourceResolver<'_>,
    context: &mut SamplerContext,
    quart: [i32; 3],
) -> std::ops::ControlFlow<Result<&'static str, BiomeSourceError>> {
    let BiomeSourceResolver::MultiNoise(source) = resolver else {
        return std::ops::ControlFlow::Continue(());
    };
    if !tdmap_core::world::biomes_active() {
        return std::ops::ControlFlow::Continue(());
    }
    std::ops::ControlFlow::Break(match source.sample(context, quart) {
        Ok(vanilla) => Ok(tdmap_core::world::noise_biome(vanilla, quart)),
        Err(error) => Err(error.into()),
    })
}
