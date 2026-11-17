use tdmap_core::world;

#[test]
fn without_a_registered_source_the_climate_stays_vanilla() {
    let mut values = [0.25f32, -0.5, 0.1, 0.0, 0.0, -0.7];
    let before = values;
    world::map_climate_values([10, 0, 10], &mut values);
    assert_eq!(values, before);
    assert!(!world::is_map_climate(1234));
}
