//! Neutral picking: make user-chosen grey points render grey.
//!
//! One point fixes a uniform cast (a per-channel offset in normalized exposure).
//! Two points in different tones also fix crossover casts, where shadows and
//! highlights lean different ways (a per-channel slope plus offset).
//!
//! The correction is folded back into the per-channel black/white points, so
//! rendering and export need no extra state.

use crate::{
    analysis::{density, Analysis},
    geometry::{Orientation, Oriented},
    image::RgbSource,
};

/// Below this tonal separation two points are averaged into one.
const MIN_TONAL_SEPARATION: f32 = 0.1;
/// Limit on how much a channel's contrast may be rescaled relative to the others.
const SLOPE_LIMIT: (f32, f32) = (0.5, 2.0);

/// Mean density of a square patch centred at normalized display coordinates
/// (`u`, `v` in 0..1, after `orientation`). Averages transmittance first, which
/// behaves like a larger measuring aperture and suppresses grain.
pub fn sample_density<S: RgbSource>(
    src: &S,
    orientation: Orientation,
    u: f32,
    v: f32,
    radius: usize,
) -> [f32; 3] {
    let view = Oriented::new(src, orientation);
    let (w, h) = (view.width() as usize, view.height() as usize);
    let cx = ((u.clamp(0.0, 1.0) * w as f32) as usize).min(w - 1);
    let cy = ((v.clamp(0.0, 1.0) * h as f32) as usize).min(h - 1);
    let (x0, x1) = (cx.saturating_sub(radius), (cx + radius + 1).min(w));
    let (y0, y1) = (cy.saturating_sub(radius), (cy + radius + 1).min(h));
    let mut acc = [0f64; 3];
    let mut n = 0f64;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = view.rgb(y * w + x);
            for c in 0..3 {
                acc[c] += p[c] as f64;
            }
            n += 1.0;
        }
    }
    [0, 1, 2].map(|c| density((acc[c] / n) as f32))
}

fn exposure(an: &Analysis, d: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|c| (d[c] - an.lo[c]) / (an.hi[c] - an.lo[c]).max(1e-3))
}

fn mean3(v: [f32; 3]) -> f32 {
    (v[0] + v[1] + v[2]) / 3.0
}

/// Return a new analysis in which every sample (given as densities) is neutral.
/// Uses the first two samples; the target for each is its mean normalized exposure,
/// so overall brightness stays put.
pub fn neutralize(an: &Analysis, samples: &[[f32; 3]]) -> Analysis {
    let mut pts: Vec<[f32; 3]> = samples.iter().take(2).map(|&d| exposure(an, d)).collect();
    if pts.is_empty() {
        return *an;
    }
    if pts.len() == 2 && (mean3(pts[1]) - mean3(pts[0])).abs() < MIN_TONAL_SEPARATION {
        let avg = [0, 1, 2].map(|c| (pts[0][c] + pts[1][c]) / 2.0);
        pts = vec![avg];
    }
    let targets: Vec<f32> = pts.iter().map(|&e| mean3(e)).collect();

    let mut out = *an;
    for c in 0..3 {
        // Corrected exposure: E' = a * E + b.
        let (a, b) = if pts.len() == 2 {
            let de = pts[1][c] - pts[0][c];
            let a = if de.abs() > 1e-4 {
                ((targets[1] - targets[0]) / de).clamp(SLOPE_LIMIT.0, SLOPE_LIMIT.1)
            } else {
                1.0
            };
            // Least-squares offset for the (possibly clamped) slope.
            let b = ((targets[0] - a * pts[0][c]) + (targets[1] - a * pts[1][c])) / 2.0;
            (a, b)
        } else {
            (1.0, targets[0] - pts[0][c])
        };
        // (D - lo') / span' == a * (D - lo) / span + b
        let span = (an.hi[c] - an.lo[c]).max(1e-3) / a;
        let lo = an.lo[c] - b * span;
        out.lo[c] = lo;
        out.hi[c] = lo + span;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pipeline::Prepared, Adjustments};

    fn analysis() -> Analysis {
        Analysis { lo: [0.5, 0.9, 1.2], hi: [1.1, 1.65, 2.1] }
    }

    /// Densities whose normalized exposures are `e`.
    fn dens(an: &Analysis, e: [f32; 3]) -> [f32; 3] {
        [0, 1, 2].map(|c| an.lo[c] + e[c] * (an.hi[c] - an.lo[c]))
    }

    fn spread(v: [f32; 3]) -> f32 {
        v[0].max(v[1]).max(v[2]) - v[0].min(v[1]).min(v[2])
    }

    #[test]
    fn one_point_removes_a_uniform_cast() {
        let an = analysis();
        let grey = dens(&an, [0.45, 0.40, 0.52]);
        let fixed = neutralize(&an, &[grey]);
        assert!(spread(exposure(&fixed, grey)) < 1e-4);
        assert!((mean3(exposure(&fixed, grey)) - 0.45667).abs() < 1e-3, "brightness kept");
    }

    #[test]
    fn two_points_remove_a_crossover_cast() {
        let an = analysis();
        // Shadows lean red, highlights lean blue: a slope mismatch.
        let shadow = dens(&an, [0.36, 0.32, 0.33]);
        let highlight = dens(&an, [0.88, 0.92, 0.95]);
        let fixed = neutralize(&an, &[shadow, highlight]);
        assert!(spread(exposure(&fixed, shadow)) < 1e-4);
        assert!(spread(exposure(&fixed, highlight)) < 1e-4);

        // And the rendered pixels come out grey.
        let prep = Prepared::new(&fixed, &Adjustments::default());
        for d in [shadow, highlight] {
            let x = d.map(|v| 10f32.powf(-v));
            assert!(spread(prep.apply(x).0) < 1e-3);
        }
    }

    #[test]
    fn close_tones_fall_back_to_one_point() {
        let an = analysis();
        let a = dens(&an, [0.50, 0.45, 0.55]);
        let b = dens(&an, [0.52, 0.47, 0.57]);
        let fixed = neutralize(&an, &[a, b]);
        for c in 0..3 {
            let span_ratio = (fixed.hi[c] - fixed.lo[c]) / (an.hi[c] - an.lo[c]);
            assert!((span_ratio - 1.0).abs() < 1e-5, "slope must be untouched");
        }
    }

    #[test]
    fn sampling_follows_orientation() {
        use crate::image::ImageF32;
        // 4x2 image: left half transmittance 0.1, right half 0.5.
        let data = (0..8)
            .flat_map(|i| if i % 4 < 2 { [0.1f32; 3] } else { [0.5f32; 3] })
            .collect();
        let img = ImageF32 { width: 4, height: 2, data };
        let left = sample_density(&img, Orientation::default(), 0.1, 0.5, 0);
        assert!((left[0] - 1.0).abs() < 1e-5);
        // Mirrored, the left of the screen shows the stored right half.
        let mirrored = sample_density(&img, Orientation { rotation: 0, mirror: true }, 0.1, 0.5, 0);
        assert!((mirrored[0] - density(0.5)).abs() < 1e-5);
    }
}
