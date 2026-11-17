// JigsawPlacement.couldStructureExistInColumn (villages, outposts, trail ruins, ancient
// cities, the data packs' jigsaw structures): in the map's dimension, whether the map's biome
// of the column is one the structure may start in.
fn tdmap_valid_biome(
    dimension: &str,
    flat: bool,
    data: &GeneratorData,
    valid: &BTreeSet<BiomeId>,
    x: i32,
    z: i32,
    value: bool,
) -> bool {
    match tdmap_core::world::structure_biome(dimension, flat, x, z) {
        Some(name) => data.biome_id(name).is_some_and(|id| valid.contains(&id)),
        None => value,
    }
}
