// MultiNoiseBiomeSource::new: the parameter list of the biome source being built; in the map's
// dimension it is kept, to know where in the climate each biome is (climate.rs).
fn tdmap_biomes(entries: &Vec<(ParameterPoint, String)>) {
    let mut boxes = Vec::with_capacity(entries.len());
    for (point, name) in entries {
        let mut bounds = [[0i64; 2]; 6];
        for i in 0..6 {
            bounds[i] = [point.coordinates[i].min, point.coordinates[i].max];
        }
        boxes.push((bounds, name.as_str()));
    }
    tdmap_core::world::register_biomes(&boxes);
}
