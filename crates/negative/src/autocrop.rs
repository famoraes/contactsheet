//! AutoCrop: find the image area of a scanned frame.
//!
//! Around the picture there is usually film base (unexposed rebate, sprocket
//! area), the film holder, or empty glass. Each of those is *uniform* along the
//! frame edge, while the picture is textured. So along each side we look at the
//! row/column density profile and pick the innermost strong step whose outer side
//! is uniform: that is the frame edge, with everything outside it (holder, base,
//! sprocket holes) cut away.
//!
//! A tilted frame is detected by measuring each edge in two halves; the image is
//! then straightened and measured again, so the result is a crop in the same
//! convention as [`Crop`] (rotation about the centre, then an upright rectangle).

use crate::{
    analysis::density,
    geometry::{Crop, Framed, Orientation},
    image::{downsample, ImageF32, RgbSource},
};

/// Long edge of the working copy.
const WORK_LONG: u32 = 800;
/// Edges are searched within this fraction of each side.
const SEARCH_ZONE: f32 = 0.25;
/// Extra inward margin past the detected edge, as a fraction of the dimension.
const MARGIN: f32 = 0.004;
/// Slivers: at most this fraction of the side, differing by at least this density.
const SLIVER_MAX: f32 = 0.012;
const SLIVER_STEP: f32 = 0.06;
/// Smallest believable frame, as a fraction of each dimension.
const MIN_FRAME: f32 = 0.4;
/// Tilts below this are noise; above the max the detection is not trusted.
const MIN_TILT_DEG: f32 = 0.15;
const MAX_TILT_DEG: f32 = 8.0;

/// Grey density map with very dense areas (opaque holder) clamped, so their
/// noise doesn't look like texture.
struct DensityMap {
    w: usize,
    h: usize,
    d: Vec<f32>,
}

impl DensityMap {
    fn new<S: RgbSource>(src: &S) -> Self {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let mut d: Vec<f32> = (0..w * h)
            .map(|i| {
                let p = src.rgb(i);
                (density(p[0]) + density(p[1]) + density(p[2])) / 3.0
            })
            .collect();
        let mut sorted = d.clone();
        let k = (sorted.len() / 200).min(sorted.len().saturating_sub(1));
        let floor = *sorted.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1;
        let cap = floor + 2.2;
        for v in &mut d {
            *v = v.clamp(floor - 0.5, cap);
        }
        DensityMap { w, h, d }
    }

    /// Mean and standard deviation of each column over rows `y0..y1`.
    fn columns(&self, y0: usize, y1: usize) -> (Vec<f32>, Vec<f32>) {
        let n = (y1 - y0).max(1) as f32;
        (0..self.w)
            .map(|x| {
                let (mut s, mut s2) = (0f32, 0f32);
                for y in y0..y1 {
                    let v = self.d[y * self.w + x];
                    s += v;
                    s2 += v * v;
                }
                let m = s / n;
                (m, (s2 / n - m * m).max(0.0).sqrt())
            })
            .unzip()
    }

    /// Mean and standard deviation of each row over columns `x0..x1`.
    fn rows(&self, x0: usize, x1: usize) -> (Vec<f32>, Vec<f32>) {
        let n = (x1 - x0).max(1) as f32;
        (0..self.h)
            .map(|y| {
                let row = &self.d[y * self.w + x0..y * self.w + x1];
                let m = row.iter().sum::<f32>() / n;
                let v = row.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / n;
                (m, v.sqrt())
            })
            .unzip()
    }
}

fn median(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    let k = s.len() / 2;
    *s.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1
}

/// A detected border: distance in samples from the side to the first picture
/// sample. `strong` edges (a real step with a uniform band outside) are precise
/// enough to measure tilt from; slivers are not.
#[derive(Clone, Copy, Debug)]
struct Edge {
    at: f32,
    strong: bool,
}

/// The border at the start of `mean`, or `None` if this side has none.
fn edge_from_start(mean: &[f32], stdev: &[f32]) -> Option<Edge> {
    let n = mean.len();
    if n < 32 {
        return None;
    }
    // Light smoothing, then a wide central difference.
    let m: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
            (mean[a] + mean[i] + mean[b]) / 3.0
        })
        .collect();
    let grad: Vec<f32> = (0..n)
        .map(|i| if i < 2 || i + 2 >= n { 0.0 } else { (m[i + 2] - m[i - 2]).abs() })
        .collect();
    let (q1, q3) = (n / 4, 3 * n / 4);
    let texture = median(&stdev[q1..q3]);
    let threshold = (4.0 * median(&grad[q1..q3])).max(0.04);
    let zone = ((n as f32 * SEARCH_ZONE) as usize).min(n - 3);
    let window = ((n as f32 * 0.015) as usize).max(3);

    // Innermost local maximum above threshold whose outer side is uniform.
    for i in (4..zone).rev() {
        let g = grad[i];
        if g < threshold || g < grad[i - 1] || g < grad[i + 1] {
            continue;
        }
        let (o0, o1) = (i.saturating_sub(window + 2), i - 2);
        if o1 <= o0 {
            continue;
        }
        let outer = stdev[o0..o1].iter().sum::<f32>() / (o1 - o0) as f32;
        if outer < (0.6 * texture).max(0.02) {
            // Skip the rest of the transition (edge blur), then a small margin.
            let mut j = i;
            while j + 1 < zone + window && j < i + 8 && grad[j + 1] > threshold * 0.5 {
                j += 1;
            }
            return Some(Edge { at: j as f32 + 1.0 + MARGIN * n as f32, strong: true });
        }
    }
    sliver(&m).map(|at| Edge { at, strong: false })
}

/// Thin dark/light slivers at the very edge (a scan area set just inside the
/// frame catches a few pixels of rebate): edge samples that clearly differ from
/// the picture just inside them.
fn sliver(m: &[f32]) -> Option<f32> {
    let n = m.len();
    let limit = ((n as f32 * SLIVER_MAX) as usize).max(2);
    let inside = median(&m[limit + 2..(limit + 12).min(n)]);
    let count = (0..limit).take_while(|&k| (m[k] - inside).abs() > SLIVER_STEP).count();
    (count > 0).then(|| count as f32 + MARGIN * n as f32)
}

/// Edge positions measured from each side, in pixels: (left, right, top, bottom).
#[derive(Clone, Copy, Debug, Default)]
struct Edges {
    left: Option<Edge>,
    right: Option<Edge>,
    top: Option<Edge>,
    bottom: Option<Edge>,
}

fn both_ends(mean: &[f32], stdev: &[f32]) -> (Option<Edge>, Option<Edge>) {
    let start = edge_from_start(mean, stdev);
    let rm: Vec<f32> = mean.iter().rev().copied().collect();
    let rs: Vec<f32> = stdev.iter().rev().copied().collect();
    (start, edge_from_start(&rm, &rs))
}

fn frac(v: usize, f: f32) -> usize {
    (v as f32 * f) as usize
}

impl DensityMap {
    /// Vertical edges measured over rows `y0..y1`, horizontal over columns `x0..x1`.
    fn edges(&self, (y0, y1): (usize, usize), (x0, x1): (usize, usize)) -> Edges {
        let (cm, cs) = self.columns(y0, y1);
        let (rm, rs) = self.rows(x0, x1);
        let (left, right) = both_ends(&cm, &cs);
        let (top, bottom) = both_ends(&rm, &rs);
        Edges { left, right, top, bottom }
    }

    fn central(&self) -> Edges {
        self.edges((frac(self.h, 0.2), frac(self.h, 0.8)), (frac(self.w, 0.2), frac(self.w, 0.8)))
    }

    /// Clockwise rotation (degrees) that would square up the frame, if it is tilted.
    fn tilt(&self) -> Option<f32> {
        let (w, h) = (self.w, self.h);
        let upper = self.edges((frac(h, 0.1), frac(h, 0.45)), (frac(w, 0.1), frac(w, 0.45)));
        let lower = self.edges((frac(h, 0.55), frac(h, 0.9)), (frac(w, 0.55), frac(w, 0.9)));
        let (dy, dx) = (h as f32 * 0.45, w as f32 * 0.45);
        let mut est = Vec::new();
        let pair = |a: Option<Edge>, b: Option<Edge>| match (a, b) {
            (Some(a), Some(b)) if a.strong && b.strong => Some((a.at, b.at)),
            _ => None,
        };
        // A frame turned anticlockwise has its left edge further right at the bottom.
        if let Some((t, b)) = pair(upper.left, lower.left) {
            est.push((b - t).atan2(dy));
        }
        if let Some((t, b)) = pair(upper.right, lower.right) {
            est.push((t - b).atan2(dy)); // measured from the right side
        }
        // ...and its top edge lower on the left than on the right.
        if let Some((l, r)) = pair(upper.top, lower.top) {
            est.push((l - r).atan2(dx)); // `upper`/`lower` here are the left/right column bands
        }
        if let Some((l, r)) = pair(upper.bottom, lower.bottom) {
            est.push((r - l).atan2(dx));
        }
        if est.is_empty() {
            return None;
        }
        let deg: Vec<f32> = est.iter().map(|a| a.to_degrees()).collect();
        let mid = median(&deg);
        let agree: Vec<f32> = deg.iter().copied().filter(|d| (d - mid).abs() < 0.5).collect();
        // A single estimate is only trusted if nothing contradicts it.
        if agree.len() * 2 < deg.len() + 1 && deg.len() > 1 {
            return None;
        }
        let a = agree.iter().sum::<f32>() / agree.len() as f32;
        (a.abs() >= MIN_TILT_DEG && a.abs() <= MAX_TILT_DEG).then_some(a)
    }
}

fn materialize<S: RgbSource>(src: &S) -> ImageF32 {
    let n = (src.width() * src.height()) as usize;
    let mut data = Vec::with_capacity(n * 3);
    for i in 0..n {
        data.extend_from_slice(&src.rgb(i));
    }
    ImageF32 { width: src.width(), height: src.height(), data }
}

/// Find the picture area of `view` (a display-oriented frame). Returns `None` when
/// no border is found, i.e. the frame is already cropped.
pub fn auto_crop<S: RgbSource>(view: &S) -> Option<Crop> {
    let small = downsample(view, WORK_LONG);
    let angle = DensityMap::new(&small).tilt().unwrap_or(0.0);
    let map = if angle != 0.0 {
        let straight = Framed::new(&small, Orientation::default(), Some(Crop { angle, ..Crop::default() }));
        DensityMap::new(&materialize(&straight))
    } else {
        DensityMap::new(&small)
    };
    let e = map.central();
    if e.left.is_none() && e.right.is_none() && e.top.is_none() && e.bottom.is_none() && angle == 0.0 {
        return None;
    }
    let (w, h) = (map.w as f32, map.h as f32);
    let at = |e: Option<Edge>| e.map_or(0.0, |e| e.at);
    let x0 = at(e.left) / w;
    let x1 = 1.0 - at(e.right) / w;
    let y0 = at(e.top) / h;
    let y1 = 1.0 - at(e.bottom) / h;
    if x1 - x0 < MIN_FRAME || y1 - y0 < MIN_FRAME {
        return None;
    }
    let crop = Crop { x: x0, y: y0, w: x1 - x0, h: y1 - y0, angle };
    Some(fit_inside(crop, map.w as f32, map.h as f32))
}

/// True if every corner of `c` lands inside the image once it is rotated by `c.angle`.
fn fits(c: &Crop, w: f32, h: f32) -> bool {
    let (sin, cos) = c.angle.to_radians().sin_cos();
    [(c.x, c.y), (c.x + c.w, c.y), (c.x, c.y + c.h), (c.x + c.w, c.y + c.h)].iter().all(|&(u, v)| {
        let (dx, dy) = ((u - 0.5) * w, (v - 0.5) * h);
        let sx = w / 2.0 + dx * cos + dy * sin;
        let sy = h / 2.0 - dx * sin + dy * cos;
        (-0.01..=w + 0.01).contains(&sx) && (-0.01..=h + 0.01).contains(&sy)
    })
}

/// Shrink a straightened crop about its centre until no blank corner shows.
fn fit_inside(c: Crop, w: f32, h: f32) -> Crop {
    let scaled = |k: f32| {
        let (cx, cy) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
        Crop { x: cx - c.w * k / 2.0, y: cy - c.h * k / 2.0, w: c.w * k, h: c.h * k, angle: c.angle }
    };
    if fits(&c, w, h) {
        return c;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        if fits(&scaled(mid), w, h) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    scaled(lo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::negative_of;

    /// Textured scene (smooth shapes plus grain-like noise), as a negative.
    fn picture(w: u32, h: u32) -> ImageF32 {
        let mut data = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32, y as f32);
                let noise = ((fx * 12.9898 + fy * 78.233).sin() * 43758.547).fract().abs();
                let shape = 0.5 + 0.5 * (fx / 23.0).sin() * (fy / 17.0).cos();
                let l = 0.03 + 0.55 * shape + 0.3 * noise;
                data.extend_from_slice(&negative_of([l, l * 0.9, l * 0.8]));
            }
        }
        ImageF32 { width: w, height: h, data }
    }

    /// A frame of picture inside a film-base rebate inside an opaque holder.
    /// Returns the image and the true picture rect (normalized).
    fn framed_scan(w: u32, h: u32) -> (ImageF32, Crop) {
        let pic = picture(w, h);
        let base = negative_of([0.0; 3]);
        let (hx, hy) = (w as f32 * 0.04, h as f32 * 0.03); // holder
        let (bx, by) = (w as f32 * 0.07, h as f32 * 0.09); // holder + rebate
        let mut data = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let holder = fx < hx || fx > w as f32 - hx || fy < hy || fy > h as f32 - hy;
                let rebate = fx < bx || fx > w as f32 - bx || fy < by || fy > h as f32 - by;
                let px = if holder {
                    [0.0008, 0.0006, 0.0005]
                } else if rebate {
                    base
                } else {
                    pic.rgb((y * w + x) as usize)
                };
                data.extend_from_slice(&px);
            }
        }
        let truth = Crop { x: 0.07, y: 0.09, w: 0.86, h: 0.82, angle: 0.0 };
        (ImageF32 { width: w, height: h, data }, truth)
    }

    fn close(c: &Crop, t: &Crop, tol: f32) -> bool {
        // Must not include border (crop inside truth) and not lose much picture.
        let inside = c.x >= t.x - 0.002 && c.y >= t.y - 0.002 && c.x + c.w <= t.x + t.w + 0.002 && c.y + c.h <= t.y + t.h + 0.002;
        inside && (c.w - t.w).abs() < tol && (c.h - t.h).abs() < tol
    }

    #[test]
    fn finds_frame_inside_rebate_and_holder() {
        let (img, truth) = framed_scan(900, 600);
        let c = auto_crop(&img).expect("frame found");
        assert!(close(&c, &truth, 0.03), "{c:?} vs {truth:?}");
        assert_eq!(c.angle, 0.0);
    }

    #[test]
    fn already_cropped_scan_gives_none() {
        assert!(auto_crop(&picture(900, 600)).is_none());
    }

    #[test]
    fn straightens_a_tilted_frame() {
        let (img, _) = framed_scan(900, 600);
        // Tilt the whole scan 2° anticlockwise; AutoCrop should rotate it back clockwise.
        let tilted = materialize(&Framed::new(&img, Orientation::default(), Some(Crop { angle: -2.0, ..Crop::default() })));
        let c = auto_crop(&tilted).expect("frame found");
        assert!((c.angle - 2.0).abs() < 0.3, "angle {}", c.angle);
        assert!(c.w > 0.7 && c.h > 0.6, "{c:?}");
        assert!(fits(&c, 900.0, 600.0), "no blank corners: {c:?}");
    }
}
