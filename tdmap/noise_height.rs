// WorldgenOverrides::apply for the noise settings: the Overworld's noise (where the generator
// and its structure queries look for the ground) reaches only y 320 in vanilla. With the
// map's data pack tdmap-height the level is taller, so the noise is made as tall as the
// level: structures on peaks above 320 stand on them instead of inside them.
fn tdmap_noise_height(
    overrides: &WorldgenOverrides,
    bundle: &str,
    value: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut value = value;
    let st = tdmap_core::world::state();
    if bundle != "settings" || st.map.is_none() || st.cfg.dimension != "minecraft:overworld" {
        return value;
    }
    let Some(level) = overrides.level_height("minecraft:overworld") else {
        return value;
    };
    let Some(text) = value.get_mut("minecraft:overworld") else {
        return value;
    };
    let Ok(mut json) = serde_json::from_str::<Value>(text) else {
        return value;
    };
    let (Some(min_y), Some(height)) = (json["noise"]["min_y"].as_i64(), json["noise"]["height"].as_i64()) else {
        return value;
    };
    let (level_min, level_top) = (level.min_y as i64, (level.min_y + level.height) as i64);
    if min_y < level_min || min_y + height > level_top || (min_y == level_min && min_y + height == level_top) {
        return value;
    }
    json["noise"]["min_y"] = level_min.into();
    json["noise"]["height"] = (level_top - level_min).into();
    *text = json.to_string();
    log::info!(
        target: tdmap_core::world::LOG,
        "minecraft:overworld: the noise reaches y {level_min}..{level_top} (the level's height), so structures see the map's peaks"
    );
    value
}
