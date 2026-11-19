use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::image::RgbSource;

/// Per-channel density black/white points.
///
/// `lo` is the thinnest density (film base / deepest shadow -> print black),
/// `hi` the densest (brightest highlight -> print white).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// Normalized rectangle (0..1 of width/height).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const FULL: Rect = Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };

    /// Default region before the user crops: skip borders, rebate and film holder.
    pub const DEFAULT_INSET: Rect = Rect { x: 0.08, y: 0.08, w: 0.84, h: 0.84 };

    fn to_pixels(self, width: u32, height: u32) -> (usize, usize, usize, usize) {
        let (wu, hu) = (width.max(1) as usize, height.max(1) as usize);
        let (w, h) = (wu as f32, hu as f32);
        let x0 = ((self.x.clamp(0.0, 1.0) * w).floor() as usize).min(wu - 1);
        let y0 = ((self.y.clamp(0.0, 1.0) * h).floor() as usize).min(hu - 1);
        let x1 = (((self.x + self.w).clamp(0.0, 1.0) * w).ceil() as usize).clamp(x0 + 1, wu);
        let y1 = (((self.y + self.h).clamp(0.0, 1.0) * h).ceil() as usize).clamp(y0 + 1, hu);
        (x0, y0, x1, y1)
    }
}

/// Default percentiles: clip almost nothing (SmartConvert also defaults to no clipping).
pub const LOW_PERCENTILE: f32 = 0.0005;
pub const HIGH_PERCENTILE: f32 = 0.9995;

const D_MIN: f32 = -1.0;
const D_MAX: f32 = 6.0;
const BINS: usize = 14_000;
const MIN_RANGE: f32 = 0.05;

#[inline]
pub fn density(x: f32) -> f32 {
    -x.max(1e-6).log10()
}

#[inline]
fn bin_of(d: f32) -> usize {
    let t = (d - D_MIN) / (D_MAX - D_MIN);
    ((t * BINS as f32) as isize).clamp(0, BINS as isize - 1) as usize
}

#[inline]
fn value_of(bin: usize) -> f32 {
    D_MIN + (bin as f32 + 0.5) / BINS as f32 * (D_MAX - D_MIN)
}

fn percentile(hist: &[u32], total: u64, p: f32) -> f32 {
    let target = ((total as f64) * p as f64).ceil().max(1.0) as u64;
    let mut acc = 0u64;
    for (i, &n) in hist.iter().enumerate() {
        acc += n as u64;
        if acc >= target {
            return value_of(i);
        }
    }
    value_of(hist.len() - 1)
}

/// Compute density black/white points per channel inside `region`.
pub fn analyze<S: RgbSource>(src: &S, region: Rect) -> Analysis {
    analyze_with(src, region, LOW_PERCENTILE, HIGH_PERCENTILE)
}

pub fn analyze_with<S: RgbSource>(src: &S, region: Rect, low: f32, high: f32) -> Analysis {
    let width = src.width() as usize;
    let (x0, y0, x1, y1) = region.to_pixels(src.width(), src.height());
    let hist = (y0..y1)
        .into_par_iter()
        .fold(
            || vec![0u32; 3 * BINS],
            |mut h, y| {
                for x in x0..x1 {
                    let p = src.rgb(y * width + x);
                    for c in 0..3 {
                        h[c * BINS + bin_of(density(p[c]))] += 1;
                    }
                }
                h
            },
        )
        .reduce(
            || vec![0u32; 3 * BINS],
            |mut a, b| {
                a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
                a
            },
        );
    let total = ((x1 - x0) * (y1 - y0)) as u64;
    let mut lo = [0f32; 3];
    let mut hi = [0f32; 3];
    for c in 0..3 {
        let ch = &hist[c * BINS..(c + 1) * BINS];
        lo[c] = percentile(ch, total, low);
        hi[c] = percentile(ch, total, high).max(lo[c] + MIN_RANGE);
    }
    Analysis { lo, hi }
}
