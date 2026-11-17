// ClimateSampler::sample_volume (a block of quarts at once: a chunk's biomes, a column's for a
// jigsaw structure): as climate.rs, for every quart of the volume.
fn tdmap_climate_volume(
    climate: &ClimateSampler,
    context: &mut SamplerContext,
    quart_minimum: [i32; 3],
    extents: [i32; 3],
) -> std::ops::ControlFlow<Result<ClimateVolume, SampleError>> {
    let id = Arc::as_ptr(&climate.samplers[0]) as *const () as usize;
    if !tdmap_core::world::is_map_climate(id) {
        return std::ops::ControlFlow::Continue(());
    }
    let minimum = [
        quart_minimum[0].wrapping_shl(2),
        quart_minimum[1].wrapping_shl(2),
        quart_minimum[2].wrapping_shl(2),
    ];
    let volume = match DensityVolume::new(extents, minimum, [4; 3]) {
        Ok(volume) => volume,
        Err(error) => return std::ops::ControlFlow::Break(Err(error.into())),
    };
    let mut fields: [Vec<f32>; 6] = Default::default();
    for i in 0..6 {
        fields[i].resize(volume.size(), 0.);
        if let Err(error) = climate.samplers[i].sample_volume(context, &mut fields[i], volume) {
            return std::ops::ControlFlow::Break(Err(error));
        }
    }
    let [sx, sy, sz] = volume.sizes();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let Some(index) = volume.index([x, y, z]) else {
                    continue;
                };
                let quart = [
                    quart_minimum[0].wrapping_add(x as i32),
                    quart_minimum[1].wrapping_add(y as i32),
                    quart_minimum[2].wrapping_add(z as i32),
                ];
                let mut values = [0f32; 6];
                for i in 0..6 {
                    values[i] = fields[i][index];
                }
                tdmap_core::world::map_climate_values(quart, &mut values);
                for i in 0..6 {
                    fields[i][index] = values[i];
                }
            }
        }
    }
    std::ops::ControlFlow::Break(Ok(ClimateVolume {
        quart_minimum,
        volume,
        fields,
    }))
}
