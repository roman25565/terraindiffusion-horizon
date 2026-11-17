// The router just compiled: in the map's dimension, its final density reads the map
// (`tdmap_density::MapDensity`), so structures and the spawn search see the map's ground.
fn tdmap_router(value: Result<RouterSamplers, CompileError>) -> Result<RouterSamplers, CompileError> {
    value.map(|mut router| {
        if let Some(dimension) = tdmap_core::world::building_ours() {
            let i = RouterField::FinalDensity as usize;
            let inner = router.samplers[i].clone();
            router.samplers[i] = Arc::new(tdmap_density::MapDensity::new(inner, &dimension));
            log::info!(target: tdmap_core::world::LOG, "{dimension}: the terrain queries read the map");
        }
        router
    })
}
