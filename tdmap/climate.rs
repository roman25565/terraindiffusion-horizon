// ClimateSampler::sample (one quart): in the map's dimension, the climate is moved to where
// the biome source answers the map's biome of the column, so structures (villages, temples,
// the data packs' ones) choose their place by the map's biomes. Deep cave biomes stay.
fn tdmap_climate(
    climate: &ClimateSampler,
    context: &mut SamplerContext,
    quart: [i32; 3],
) -> std::ops::ControlFlow<Result<TargetPoint, SampleError>> {
    let id = Arc::as_ptr(&climate.samplers[0]) as *const () as usize;
    if !tdmap_core::world::is_map_climate(id) {
        return std::ops::ControlFlow::Continue(());
    }
    let position = [
        quart[0].wrapping_shl(2),
        quart[1].wrapping_shl(2),
        quart[2].wrapping_shl(2),
    ];
    let mut values = [0f32; 6];
    for i in 0..6 {
        match climate.samplers[i].sample_value(context, position) {
            Ok(value) => values[i] = value,
            Err(error) => return std::ops::ControlFlow::Break(Err(error)),
        }
    }
    let vanilla = TargetPoint::from_values(values);
    let p = vanilla.parameters();
    let target = match tdmap_core::world::map_climate(quart, [p[0], p[1], p[2], p[3], p[4], p[5]]) {
        Some([temperature, humidity, continentalness, erosion, depth, weirdness]) => TargetPoint {
            temperature,
            humidity,
            continentalness,
            erosion,
            depth,
            weirdness,
        },
        None => vanilla,
    };
    std::ops::ControlFlow::Break(Ok(target))
}
