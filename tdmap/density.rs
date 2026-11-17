// A module of horizon-world (`crate::worldgen::router::tdmap_density`): the map's ground as a
// density function. When the Overworld's generator is built its `final_density` is registered
// here, and the router hands out the MapDensity in its place (RouterSamplers::get), so everything that reads the terrain from the noise (where structures stand and
// whether they fit, the spawn search, the far terrain's samples) reads the map: positive (solid)
// below the column's ground, negative above. Where the mod leaves the level to vanilla (outside
// the map with `outside = "vanilla"`) the wrapped function answers.
use horizon_core::density::{DensitySampler, SampleError, SamplerContext};
use horizon_core::noise::DensityVolume;
use std::sync::Arc;

#[derive(Debug)]
pub struct MapDensity {
    inner: Arc<dyn DensitySampler>,
    dimension: String,
}

impl MapDensity {
    pub fn new(inner: Arc<dyn DensitySampler>, dimension: &str) -> MapDensity {
        MapDensity { inner, dimension: dimension.to_owned() }
    }
    /// The first Y above the ground of column `[x, z]`, when the map has it.
    fn top(&self, x: i32, z: i32) -> Option<i32> {
        tdmap_core::world::structure_ground(&self.dimension, false, x, z).map(|(top, _)| top)
    }
}

/// Solid below `top`, air (or the aquifer's water) from `top` up; a slope of 1/8 per block so
/// the chunk fill's interpolation between cell corners keeps the surface where it is.
fn density(top: i32, y: i32) -> f32 {
    ((top as f32 - 0.5 - y as f32) * 0.125).clamp(-1.0, 1.0)
}

impl DensitySampler for MapDensity {
    fn sample_value(&self, context: &mut SamplerContext, position: [i32; 3]) -> Result<f32, SampleError> {
        match self.top(position[0], position[2]) {
            Some(top) => Ok(density(top, position[1])),
            None => self.inner.sample_value(context, position),
        }
    }

    fn sample_volume(
        &self,
        context: &mut SamplerContext,
        buffer: &mut [f32],
        volume: DensityVolume,
    ) -> Result<(), SampleError> {
        let [sx, sy, sz] = volume.sizes();
        let [mx, my, mz] = volume.minimum();
        let [stx, sty, stz] = volume.steps();
        // Columns in buffer order (Z outer, X inner; Y innermost in the buffer).
        let mut tops = Vec::with_capacity(sx * sz);
        for iz in 0..sz {
            for ix in 0..sx {
                let x = mx.wrapping_add((ix as i32).wrapping_mul(stx));
                let z = mz.wrapping_add((iz as i32).wrapping_mul(stz));
                tops.push(self.top(x, z));
            }
        }
        if tops.iter().all(Option::is_none) || buffer.len() < volume.size() {
            return self.inner.sample_volume(context, buffer, volume);
        }
        if tops.iter().any(Option::is_none) {
            self.inner.sample_volume(context, buffer, volume)?;
        }
        for (column, top) in tops.iter().enumerate() {
            let Some(top) = *top else { continue };
            for iy in 0..sy {
                let y = my.wrapping_add((iy as i32).wrapping_mul(sty));
                buffer[column * sy + iy] = density(top, y);
            }
        }
        Ok(())
    }

    fn sample_needed(
        &self,
        context: &mut SamplerContext,
        buffer: &mut [f32],
        volume: DensityVolume,
        needed: &[u64],
    ) -> Result<(), SampleError> {
        let [sx, _, sz] = volume.sizes();
        let [mx, _, mz] = volume.minimum();
        let [stx, _, stz] = volume.steps();
        let ours = (0..sz).any(|iz| {
            (0..sx).any(|ix| {
                self.top(
                    mx.wrapping_add((ix as i32).wrapping_mul(stx)),
                    mz.wrapping_add((iz as i32).wrapping_mul(stz)),
                )
                .is_some()
            })
        });
        if ours {
            self.sample_volume(context, buffer, volume)
        } else {
            self.inner.sample_needed(context, buffer, volume, needed)
        }
    }
}

// The final densities wrapped so far: (the wrapped function's address, its MapDensity). One
// per generator of the map's dimension, built once and kept for the server's life.
static WRAPPED: std::sync::RwLock<Vec<(usize, &'static Arc<dyn DensitySampler>)>> =
    std::sync::RwLock::new(Vec::new());
static ANY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn key(sampler: &Arc<dyn DensitySampler>) -> usize {
    Arc::as_ptr(sampler) as *const () as usize
}

/// `final_density` belongs to a generator of the map's `dimension`: from now on the router
/// answers [`MapDensity`] for it.
pub fn register(final_density: &Arc<dyn DensitySampler>, dimension: &str) {
    let k = key(final_density);
    let mut list = WRAPPED.write().unwrap_or_else(|e| e.into_inner());
    if list.iter().any(|(w, _)| *w == k) {
        return;
    }
    let wrapped: Arc<dyn DensitySampler> = Arc::new(MapDensity::new(final_density.clone(), dimension));
    list.push((k, Box::leak(Box::new(wrapped))));
    ANY.store(true, std::sync::atomic::Ordering::Release);
}

/// The [`MapDensity`] standing for `sampler`, when it was registered.
pub fn wrapped(sampler: &Arc<dyn DensitySampler>) -> Option<&'static Arc<dyn DensitySampler>> {
    if !ANY.load(std::sync::atomic::Ordering::Acquire) {
        return None;
    }
    let k = key(sampler);
    let list = WRAPPED.read().unwrap_or_else(|e| e.into_inner());
    list.iter().find(|(w, _)| *w == k).map(|(_, s)| *s)
}
