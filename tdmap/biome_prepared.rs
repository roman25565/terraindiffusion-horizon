// BiomeSourceResolver::sample_prepared with a climate volume (a chunk's biomes): as
// biome.rs. Without a volume the original calls sample, which biome.rs handles.
fn tdmap_biome_prepared(
    resolver: &mut BiomeSourceResolver<'_>,
    volume: Option<&ClimateVolume>,
    quart: [i32; 3],
) -> std::ops::ControlFlow<Result<&'static str, BiomeSourceError>> {
    if let BiomeSourceResolver::MultiNoise(source) = resolver
        && let Some(volume) = volume
        && tdmap_core::world::biomes_active()
    {
        return std::ops::ControlFlow::Break(match source.sample_prepared(volume, quart) {
            Ok(vanilla) => Ok(tdmap_core::world::noise_biome(vanilla, quart)),
            Err(error) => Err(error.into()),
        });
    }
    std::ops::ControlFlow::Continue(())
}
