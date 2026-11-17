// ChunkGenerator.iterateNoiseColumn, which getBaseHeight and getBaseColumn read for the
// structures (where a village, a pyramid, a portal stands, whether it fits): in the map's
// dimension, the map's column (ground, water up to the map's sea level, air above) instead of
// vanilla's noise, so a structure stands on the ground the chunk is built with.
fn tdmap_noise_column(
    dimension: &str,
    flat: bool,
    data: &GeneratorData,
    min: i32,
    height: i32,
    x: i32,
    z: i32,
    visit: &mut dyn FnMut(i32, BlockStateId) -> bool,
) -> std::ops::ControlFlow<()> {
    use std::ops::ControlFlow;
    use tdmap_core::world::{self, Fill};
    let Some((top, _)) = world::structure_ground(dimension, flat, x, z) else {
        return ControlFlow::Continue(());
    };
    let registry = data.registry;
    let (Some(stone), Some(water), Some(air)) = (
        registry.default_state("minecraft:stone"),
        registry.default_state("minecraft:water"),
        registry.default_state("minecraft:air"),
    ) else {
        return ControlFlow::Continue(());
    };
    if height <= 1 {
        return ControlFlow::Continue(());
    }
    let sea = world::sea_level().unwrap_or(63);
    let top = top.clamp(min + 1, min + height - 1);
    for y in (min..min + height).rev() {
        let state = match world::fill(top, sea, y) {
            Fill::Ground => stone,
            Fill::Water => water,
            Fill::Air => air,
        };
        if visit(y, state) {
            break;
        }
    }
    ControlFlow::Break(())
}
