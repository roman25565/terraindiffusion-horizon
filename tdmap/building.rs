// A generator is being built for `dimension` on this thread: the router compiled next is that
// dimension's (`NoiseRouter::compile`, below, wraps the map's dimension's final density).
fn tdmap_building(dimension: &str) {
    tdmap_core::world::set_building(dimension);
}
