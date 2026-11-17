// RouterSamplers::get: the map's MapDensity in place of a registered final density (where
// structures stand, the spawn search, the far terrain's samples).
fn tdmap_get(field: RouterField, value: &Arc<dyn DensitySampler>) -> &Arc<dyn DensitySampler> {
    if field == RouterField::FinalDensity
        && let Some(wrapped) = tdmap_density::wrapped(value)
    {
        return wrapped;
    }
    value
}
