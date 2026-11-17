// The router of a generator being built (its climate is taken from it once, while it is
// built): in the map's dimension, its final density is registered to read the map, and its
// climate fields are noted, so the biome source answers the map's biomes (climate.rs).
fn tdmap_register(router: &RouterSamplers) {
    if let Some(dimension) = tdmap_core::world::building_ours() {
        let final_density = &router.samplers[RouterField::FinalDensity as usize];
        tdmap_density::register(final_density, &dimension);
        let temperature = &router.samplers[0];
        tdmap_core::world::register_climate(Arc::as_ptr(temperature) as *const () as usize);
        log::info!(target: tdmap_core::world::LOG, "{dimension}: the terrain queries read the map");
    }
}
