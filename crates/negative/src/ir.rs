//! Infrared dust and scratch removal.
//!
//! Film dyes are almost transparent to infrared, while dust, hair and scratches
//! block it. A scanner's IR channel is therefore a near-uniform field with the
//! defects dark in it. The steps here:
//!
//! 1. **Align** the IR to the RGB. Scanners often capture IR in a separate pass,
//!    so it can be offset and slightly stretched; an affine map is fitted from
//!    defect correlations measured on a grid of regions.
//! 2. **Normalize** by a robust local baseline, giving the fraction of light each
//!    pixel lets through (`attenuation`, 1 = clean). The baseline is coarse, so
//!    uneven IR illumination drops out; the faint image "ghost" and Newton's
//!    rings stay well under the detection threshold.
//! 3. **Repair** in the linear negative: partly transparent dust is divided out
//!    (it dims all wavelengths alike, so the detail under it survives); opaque
//!    defects are filled from their surroundings.

use serde::{Deserialize, Serialize};

use crate::image::{ImageF32, RgbSource};

/// The IR channel of a scan: linear transmittance (0..65535), same size as the RGB.
/// Kept as 16-bit to halve memory at full resolution.
#[derive(Clone)]
pub struct IrPlane {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u16>,
}

impl IrPlane {
    #[inline]
    fn at(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width as usize + x] as f32 * (1.0 / 65535.0)
    }

    /// Bilinear sample with edge clamping.
    #[inline]
    fn sample(&self, fx: f32, fy: f32) -> f32 {
        let (w, h) = (self.width as usize, self.height as usize);
        let fx = fx.clamp(0.0, (w - 1) as f32);
        let fy = fy.clamp(0.0, (h - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let top = self.at(x0, y0) * (1.0 - tx) + self.at(x1, y0) * tx;
        let bottom = self.at(x0, y1) * (1.0 - tx) + self.at(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
    }

    /// Box-filter downsample by an integer factor (matches [`crate::downsample`]).
    pub fn downsample(&self, factor: usize) -> IrPlane {
        let f = factor.max(1);
        let (w, h) = (self.width as usize, self.height as usize);
        let (ow, oh) = ((w / f).max(1), (h / f).max(1));
        let mut data = vec![0u16; ow * oh];
        for oy in 0..oh {
            for ox in 0..ow {
                let mut acc = 0f32;
                let mut n = 0u32;
                for y in oy * f..((oy + 1) * f).min(h) {
                    for x in ox * f..((ox + 1) * f).min(w) {
                        acc += self.at(x, y);
                        n += 1;
                    }
                }
                data[oy * ow + ox] = (acc / n.max(1) as f32 * 65535.0 + 0.5) as u16;
            }
        }
        IrPlane { width: ow as u32, height: oh as u32, data }
    }
}

/// Where each RGB pixel's IR sample is: `ir = (x, y) - offset(x, y)`, with the
/// offset an affine function of position (in RGB pixels).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct IrAlignment {
    /// dx = c[0] + c[1]·x + c[2]·y
    pub dx: [f32; 3],
    /// dy = c[0] + c[1]·x + c[2]·y
    pub dy: [f32; 3],
}

impl IrAlignment {
    #[inline]
    fn ir_coords(&self, x: f32, y: f32) -> (f32, f32) {
        let dx = self.dx[0] + self.dx[1] * x + self.dx[2] * y;
        let dy = self.dy[0] + self.dy[1] * x + self.dy[2] * y;
        (x - dx, y - dy)
    }

    /// The same alignment for an image downsampled by `factor`.
    pub fn scaled(&self, factor: usize) -> IrAlignment {
        let f = factor.max(1) as f32;
        IrAlignment {
            dx: [self.dx[0] / f, self.dx[1], self.dx[2]],
            dy: [self.dy[0] / f, self.dy[1], self.dy[2]],
        }
    }
}

/// High-pass of `ln(value)`: value minus a (2r+1)² box mean, over a square region.
fn highpass(get: &dyn Fn(usize, usize) -> f32, x0: usize, y0: usize, size: usize, r: usize) -> Vec<f32> {
    let n = size + 2 * r;
    let mut v = vec![0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            v[y * n + x] = get(x0 + x - r, y0 + y - r).max(1e-5).ln();
        }
    }
    // Integral image for the box mean.
    let mut ii = vec![0f64; (n + 1) * (n + 1)];
    for y in 0..n {
        let mut row = 0f64;
        for x in 0..n {
            row += v[y * n + x] as f64;
            ii[(y + 1) * (n + 1) + x + 1] = ii[y * (n + 1) + x + 1] + row;
        }
    }
    let k = (2 * r + 1) as f64;
    let mut out = vec![0f32; size * size];
    for y in 0..size {
        for x in 0..size {
            let (ya, yb, xa, xb) = (y, y + 2 * r + 1, x, x + 2 * r + 1);
            let s = ii[yb * (n + 1) + xb] - ii[ya * (n + 1) + xb] - ii[yb * (n + 1) + xa] + ii[ya * (n + 1) + xa];
            out[y * size + x] = v[(y + r) * n + x + r] - (s / (k * k)) as f32;
        }
    }
    out
}

/// Fit the IR-to-RGB alignment from defect correlations on a grid of regions.
/// Defects (dark in both IR and the negative's green) dominate the high-passed
/// signals, so their best overlap gives the local offset.
pub fn estimate_alignment<S: RgbSource>(rgb: &S, ir: &IrPlane) -> IrAlignment {
    const GRID: usize = 4;
    const SEARCH: i32 = 6;
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let size = (w.min(h) / 6).clamp(64, 512);
    let margin = size / 2 + SEARCH as usize + 8;
    if w < 2 * margin || h < 2 * margin {
        return IrAlignment::default();
    }
    let green = |x: usize, y: usize| rgb.rgb(y * w + x)[1];
    let infra = |x: usize, y: usize| ir.at(x, y);
    // (x, y, dx, dy, weight)
    let mut samples: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
    for gy in 0..GRID {
        for gx in 0..GRID {
            let cx = margin + (w - 2 * margin) * (2 * gx + 1) / (2 * GRID);
            let cy = margin + (h - 2 * margin) * (2 * gy + 1) / (2 * GRID);
            let (x0, y0) = (cx - size / 2, cy - size / 2);
            let a = highpass(&infra, x0, y0, size, 3);
            let b = highpass(&green, x0 - SEARCH as usize, y0 - SEARCH as usize, size + 2 * SEARCH as usize, 3);
            let bs = size + 2 * SEARCH as usize;
            let corr = |dx: i32, dy: i32| -> f64 {
                let mut s = 0f64;
                for y in 0..size {
                    let brow = (y as i32 + dy + SEARCH) as usize * bs;
                    for x in 0..size {
                        s += (a[y * size + x] * b[brow + (x as i32 + dx + SEARCH) as usize]) as f64;
                    }
                }
                s
            };
            let mut grid = vec![0f64; ((2 * SEARCH + 1) * (2 * SEARCH + 1)) as usize];
            let mut best = (0i32, 0i32, f64::MIN);
            for dy in -SEARCH..=SEARCH {
                for dx in -SEARCH..=SEARCH {
                    let c = corr(dx, dy);
                    grid[((dy + SEARCH) * (2 * SEARCH + 1) + dx + SEARCH) as usize] = c;
                    if c > best.2 {
                        best = (dx, dy, c);
                    }
                }
            }
            let (bx, by) = (best.0, best.1);
            if bx.abs() == SEARCH || by.abs() == SEARCH || best.2 <= 0.0 {
                continue; // peak on the search border: unreliable
            }
            let g = |dx: i32, dy: i32| grid[((dy + SEARCH) * (2 * SEARCH + 1) + dx + SEARCH) as usize];
            let sub = |m: f64, z: f64, p: f64| {
                let d = m - 2.0 * z + p;
                if d.abs() > 1e-12 { (0.5 * (m - p) / d).clamp(-0.5, 0.5) } else { 0.0 }
            };
            let sx = bx as f64 + sub(g(bx - 1, by), best.2, g(bx + 1, by));
            let sy = by as f64 + sub(g(bx, by - 1), best.2, g(bx, by + 1));
            // Weight by normalized correlation: regions with defects line up sharply.
            let na: f64 = a.iter().map(|v| (*v as f64).powi(2)).sum();
            let nb: f64 = (0..size)
                .flat_map(|y| (0..size).map(move |x| (y, x)))
                .map(|(y, x)| (b[(y as i32 + by + SEARCH) as usize * bs + (x as i32 + bx + SEARCH) as usize] as f64).powi(2))
                .sum();
            let ncc = best.2 / (na * nb).sqrt().max(1e-12);
            if ncc > 0.05 {
                samples.push((cx as f32, cy as f32, sx as f32, sy as f32, ncc as f32));
            }
        }
    }
    fit_affine(&samples)
}

/// Weighted least squares for d = c0 + c1·x + c2·y (each of dx, dy); falls back to
/// a plain offset with too few samples, and limits the stretch to what scanners do.
fn fit_affine(samples: &[(f32, f32, f32, f32, f32)]) -> IrAlignment {
    if samples.is_empty() {
        return IrAlignment::default();
    }
    let wsum: f64 = samples.iter().map(|s| s.4 as f64).sum();
    let mean = |f: &dyn Fn(&(f32, f32, f32, f32, f32)) -> f32| samples.iter().map(|s| f(s) as f64 * s.4 as f64).sum::<f64>() / wsum;
    let (mx, my) = (mean(&|s| s.0), mean(&|s| s.1));
    let (mdx, mdy) = (mean(&|s| s.2), mean(&|s| s.3));
    let offset_only = IrAlignment { dx: [mdx as f32, 0.0, 0.0], dy: [mdy as f32, 0.0, 0.0] };
    if samples.len() < 4 {
        return offset_only;
    }
    // Solve the 2x2 normal equations around the weighted means.
    let (mut sxx, mut sxy, mut syy, mut sxd, mut syd, mut sxe, mut sye) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
    for s in samples {
        let (x, y, w) = (s.0 as f64 - mx, s.1 as f64 - my, s.4 as f64);
        let (d, e) = (s.2 as f64 - mdx, s.3 as f64 - mdy);
        sxx += w * x * x;
        sxy += w * x * y;
        syy += w * y * y;
        sxd += w * x * d;
        syd += w * y * d;
        sxe += w * x * e;
        sye += w * y * e;
    }
    let det = sxx * syy - sxy * sxy;
    if det.abs() < 1e-9 {
        return offset_only;
    }
    let solve = |bx: f64, by: f64| ((syy * bx - sxy * by) / det, (sxx * by - sxy * bx) / det);
    let (dxx, dxy) = solve(sxd, syd);
    let (dyx, dyy) = solve(sxe, sye);
    const MAX_STRETCH: f64 = 0.003; // 0.3%: far beyond what a scanner pass varies
    if [dxx, dxy, dyx, dyy].iter().any(|v| v.abs() > MAX_STRETCH) {
        return offset_only;
    }
    IrAlignment {
        dx: [(mdx - dxx * mx - dxy * my) as f32, dxx as f32, dxy as f32],
        dy: [(mdy - dyx * mx - dyy * my) as f32, dyx as f32, dyy as f32],
    }
}

/// Fraction of light passed at each RGB pixel (1 = clean, lower = defect), from the
/// aligned IR divided by a robust local baseline.
pub fn attenuation(ir: &IrPlane, align: &IrAlignment, width: u32, height: u32) -> Vec<f32> {
    let (w, h) = (width as usize, height as usize);
    // Robust baseline: a high percentile per large block ignores dust inside it.
    // Blocks must be much larger than any defect (out-of-focus dust can be ~1% of
    // the frame) or the baseline would follow the defect; IR illumination itself
    // only varies slowly.
    let block = (w.max(h) / 16).max(8);
    let (bw, bh) = (w.div_ceil(block), h.div_ceil(block));
    let (iw, ih) = (ir.width as usize, ir.height as usize);
    let mut base = vec![0f32; bw * bh];
    let mut buf = Vec::with_capacity(block * block);
    for by in 0..bh {
        for bx in 0..bw {
            buf.clear();
            for y in (by * block..((by + 1) * block).min(ih)).step_by(2) {
                for x in (bx * block..((bx + 1) * block).min(iw)).step_by(2) {
                    buf.push(ir.at(x, y));
                }
            }
            if buf.is_empty() {
                continue;
            }
            let k = buf.len() * 9 / 10;
            base[by * bw + bx] = *buf.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1;
        }
    }
    // Smooth the block grid (3x3 mean) so block edges never show.
    let smooth: Vec<f32> = (0..bh)
        .flat_map(|y| (0..bw).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (mut s, mut n) = (0f32, 0f32);
            for yy in y.saturating_sub(1)..(y + 2).min(bh) {
                for xx in x.saturating_sub(1)..(x + 2).min(bw) {
                    s += base[yy * bw + xx];
                    n += 1.0;
                }
            }
            s / n
        })
        .collect();
    let base_at = |fx: f32, fy: f32| {
        let gx = (fx / block as f32 - 0.5).clamp(0.0, (bw - 1) as f32);
        let gy = (fy / block as f32 - 0.5).clamp(0.0, (bh - 1) as f32);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
        let (tx, ty) = (gx - x0 as f32, gy - y0 as f32);
        let top = smooth[y0 * bw + x0] * (1.0 - tx) + smooth[y0 * bw + x1] * tx;
        let bottom = smooth[y1 * bw + x0] * (1.0 - tx) + smooth[y1 * bw + x1] * tx;
        top * (1.0 - ty) + bottom * ty
    };
    let mut out = vec![1f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (ix, iy) = align.ir_coords(x as f32, y as f32);
            let b = base_at(ix, iy).max(1e-4);
            out[y * w + x] = (ir.sample(ix, iy) / b).min(1.0);
        }
    }
    out
}

/// Dust removal settings. `strength` 0..1 sets how faint a mark still counts as a
/// defect (higher = catches fainter marks).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DustParams {
    pub strength: f32,
}

impl Default for DustParams {
    fn default() -> Self {
        DustParams { strength: 0.5 }
    }
}

impl DustParams {
    /// Darkening (1 - attenuation) below which nothing is touched.
    pub fn threshold(&self) -> f32 {
        0.12 - 0.08 * self.strength.clamp(0.0, 1.0) // 12% .. 4%
    }
}

/// Attenuation below which a pixel is always filled (too little light came through
/// for the colour underneath to be trusted).
const FILL_BELOW: f32 = 0.5;

/// Remove dust and scratches from `rgb` (linear negative) given its attenuation map.
/// `scale` is the image's size relative to the full scan (1 = full resolution), so
/// sizes stay the same physical size at any resolution.
///
/// The IR is only used to find defects; their shape in the image differs (the IR
/// is blurrier, and dust dims visible light more than IR), so dividing by it
/// leaves rims. Instead:
/// - narrow defects (specks, hair, scratches) are filled from their surroundings,
///   with the mask grown to cover the IR blur and grain copied in from beside them;
/// - broad soft marks (out-of-focus dust on the glass) keep their detail and grain:
///   only the image's smooth component is corrected, by the ratio between what the
///   surroundings predict and what is there, so the correction has the mark's
///   true shape.
pub fn clean<S: RgbSource>(rgb: &S, atten: &[f32], params: DustParams, scale: f32) -> ImageF32 {
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let t = params.threshold();
    // How much darker a pixel is than its neighbourhood, relative to its own
    // darkness: ~0 for broad soft marks, towards 1 for narrow defects.
    let radius = ((8.0 * scale).round() as usize).max(2);
    let blurred = box_blur(atten, w, h, radius);
    let narrow_defect: Vec<bool> = (0..w * h)
        .map(|i| {
            let d = 1.0 - atten[i];
            let narrowness = (blurred[i] - atten[i]) / d.max(1e-3);
            (d > t && narrowness > NARROW) || atten[i] < FILL_BELOW
        })
        .collect();
    let grow = ((2.5 * scale).round() as usize).max(1);
    let fill = dilate(&narrow_defect, w, h, grow);
    let mut data = Vec::with_capacity(w * h * 3);
    for i in 0..w * h {
        data.extend_from_slice(&rgb.rgb(i));
    }
    let broad: Vec<bool> = (0..w * h).map(|i| 1.0 - atten[i] > t && !fill[i]).collect();
    correct_broad(&mut data, &broad, &fill, w, h, grow, ((6.0 * scale).round() as usize).max(1));
    fill_masked(&mut data, &fill, w, h, ((12.0 * scale).round() as usize).max(3));
    ImageF32 { width: w as u32, height: h as u32, data }
}

/// Clean an image (full scan or a box-downsampled copy, `factor` times smaller) using
/// the scan's full-resolution IR and its alignment.
pub fn clean_image<S: RgbSource>(src: &S, ir: &IrPlane, align: &IrAlignment, params: DustParams, factor: usize) -> ImageF32 {
    let f = factor.max(1);
    let (plane, al) = if f > 1 { (ir.downsample(f), align.scaled(f)) } else { (ir.clone(), *align) };
    let att = attenuation(&plane, &al, src.width(), src.height());
    clean(src, &att, params, 1.0 / f as f32)
}

/// Correct broad soft marks: per mark, in a region grown to cover its tails, the
/// image's smooth component (box blur of radius `smooth`) is replaced by what the
/// surroundings predict (push-pull from outside the region), multiplicatively, so
/// grain and detail inside the mark are kept.
fn correct_broad(data: &mut [f32], broad: &[bool], fill: &[bool], w: usize, h: usize, grow: usize, smooth: usize) {
    let mut seen = vec![false; w * h];
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !broad[start] || seen[start] {
            continue;
        }
        let mut pixels = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            pixels.push(i);
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < w && ny < h && broad[ny * w + nx] && !seen[ny * w + nx] {
                    seen[ny * w + nx] = true;
                    stack.push(ny * w + nx);
                }
            }
        }
        // The mark's tails in the image reach beyond what the IR shows.
        let radius = (x1 - x0).max(y1 - y0) / 2;
        let reach = (radius * 3 / 5 + grow).min(200);
        let margin = reach + 2 * smooth + 4;
        let (wx0, wy0) = (x0.saturating_sub(margin), y0.saturating_sub(margin));
        let (wx1, wy1) = ((x1 + margin + 1).min(w), (y1 + margin + 1).min(h));
        let (ww, wh) = (wx1 - wx0, wy1 - wy0);
        let mut core = vec![false; ww * wh];
        for &i in &pixels {
            core[(i / w - wy0) * ww + (i % w - wx0)] = true;
        }
        let region = dilate(&core, ww, wh, reach);
        // Smooth component of the window, per channel.
        let lows: Vec<Vec<f32>> = (0..3)
            .map(|c| {
                let ch: Vec<f32> = (0..ww * wh).map(|j| data[((wy0 + j / ww) * w + wx0 + j % ww) * 3 + c]).collect();
                box_blur(&ch, ww, wh, smooth)
            })
            .collect();
        // What the surroundings predict inside the region.
        let mut predicted: Vec<f32> = (0..ww * wh).flat_map(|j| [lows[0][j], lows[1][j], lows[2][j]]).collect();
        push_pull(&mut predicted, &region, ww, 0, 0, ww, wh);
        for j in 0..ww * wh {
            let i = (wy0 + j / ww) * w + wx0 + j % ww;
            if !region[j] || fill[i] {
                continue;
            }
            for c in 0..3 {
                let gain = (predicted[j * 3 + c] / lows[c][j].max(1e-5)).clamp(0.5, 3.0);
                data[i * 3 + c] *= gain;
            }
        }
    }
}

/// Narrowness above which a defect is filled rather than divided.
const NARROW: f32 = 0.3;

/// Mean over a (2r+1)² box, with edge clamping.
fn box_blur(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let (n, m) = if horizontal { (w, h) } else { (h, w) };
        let mut out = vec![0f32; w * h];
        let idx = |line: usize, k: usize| if horizontal { line * w + k } else { k * w + line };
        for line in 0..m {
            let mut acc = 0f32;
            let at = |k: isize| src[idx(line, k.clamp(0, n as isize - 1) as usize)];
            for k in -(r as isize)..=(r as isize) {
                acc += at(k);
            }
            for k in 0..n {
                out[idx(line, k)] = acc / (2 * r + 1) as f32;
                acc += at(k as isize + r as isize + 1) - at(k as isize - r as isize);
            }
        }
        out
    };
    pass(&pass(v, true), false)
}

fn dilate(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    // Separable square dilation.
    let mut tmp = vec![false; w * h];
    for y in 0..h {
        let row = &m[y * w..(y + 1) * w];
        let mut last: Option<usize> = None;
        for x in 0..w {
            if row[x] {
                last = Some(x);
            }
            if last.is_some_and(|l| x - l <= r) {
                tmp[y * w + x] = true;
            }
        }
        last = None;
        for x in (0..w).rev() {
            if row[x] {
                last = Some(x);
            }
            if last.is_some_and(|l| l - x <= r) {
                tmp[y * w + x] = true;
            }
        }
    }
    let mut out = vec![false; w * h];
    for x in 0..w {
        let mut last: Option<usize> = None;
        for y in 0..h {
            if tmp[y * w + x] {
                last = Some(y);
            }
            if last.is_some_and(|l| y - l <= r) {
                out[y * w + x] = true;
            }
        }
        last = None;
        for y in (0..h).rev() {
            if tmp[y * w + x] {
                last = Some(y);
            }
            if last.is_some_and(|l| l - y <= r) {
                out[y * w + x] = true;
            }
        }
    }
    out
}

/// Fill masked pixels from their surroundings, one connected defect at a time, with
/// push-pull interpolation in a window around it.
fn fill_masked(data: &mut [f32], mask: &[bool], w: usize, h: usize, grain_offset: usize) {
    let mut seen = vec![false; w * h];
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || seen[start] {
            continue;
        }
        // Flood the component to get its bounding box.
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < w && ny < h && mask[ny * w + nx] && !seen[ny * w + nx] {
                    seen[ny * w + nx] = true;
                    stack.push(ny * w + nx);
                }
            }
        }
        let pad = ((x1 - x0).max(y1 - y0) / 2 + 4).min(64);
        let (wx0, wy0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
        let (wx1, wy1) = ((x1 + pad + 1).min(w), (y1 + pad + 1).min(h));
        push_pull(data, mask, w, wx0, wy0, wx1 - wx0, wy1 - wy0);
        add_grain(data, mask, w, h, (x0, y0, x1, y1), grain_offset);
    }
}

/// Give a filled defect the film grain of a nearby clean area: the fine detail
/// (pixel minus its 3x3 mean) of a donor spot beside the defect is added on top
/// of the smooth fill. The donor is offset across the defect's thin direction.
fn add_grain(data: &mut [f32], mask: &[bool], w: usize, h: usize, (x0, y0, x1, y1): (usize, usize, usize, usize), offset: usize) {
    let horizontal = (x1 - x0) >= (y1 - y0); // long side horizontal -> shift vertically
    let candidates: [(isize, isize); 4] = if horizontal {
        [(0, -(offset as isize)), (0, offset as isize), (-(offset as isize), 0), (offset as isize, 0)]
    } else {
        [(-(offset as isize), 0), (offset as isize, 0), (0, -(offset as isize)), (0, offset as isize)]
    };
    let detail = |x: usize, y: usize, c: usize, data: &[f32]| -> Option<f32> {
        if x == 0 || y == 0 || x + 1 >= w || y + 1 >= h {
            return None;
        }
        let mut sum = 0f32;
        for yy in y - 1..=y + 1 {
            for xx in x - 1..=x + 1 {
                if mask[yy * w + xx] {
                    return None;
                }
                sum += data[(yy * w + xx) * 3 + c];
            }
        }
        Some(data[(y * w + x) * 3 + c] - sum / 9.0)
    };
    for (ox, oy) in candidates {
        // Use the first direction whose donor area is clean.
        let mut ok = 0usize;
        let mut total = 0usize;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !mask[y * w + x] {
                    continue;
                }
                total += 1;
                let (sx, sy) = (x as isize + ox, y as isize + oy);
                if sx >= 1 && sy >= 1 && (sx as usize) + 1 < w && (sy as usize) + 1 < h && !mask[sy as usize * w + sx as usize] {
                    ok += 1;
                }
            }
        }
        if total == 0 || ok * 10 < total * 9 {
            continue;
        }
        let mut grain = Vec::with_capacity(total);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !mask[y * w + x] {
                    continue;
                }
                let (sx, sy) = ((x as isize + ox) as usize, (y as isize + oy) as usize);
                let g: [f32; 3] = [0, 1, 2].map(|c| detail(sx, sy, c, data).unwrap_or(0.0));
                grain.push((y * w + x, g));
            }
        }
        for (i, g) in grain {
            for c in 0..3 {
                data[i * 3 + c] = (data[i * 3 + c] + g[c]).max(0.0);
            }
        }
        return;
    }
}

/// Push-pull fill inside a window: average known pixels down a pyramid, then
/// interpolate back up, writing only masked pixels.
fn push_pull(data: &mut [f32], mask: &[bool], stride: usize, x0: usize, y0: usize, ww: usize, wh: usize) {
    // Level 0: premultiplied colour and weight.
    let mut levels: Vec<(usize, usize, Vec<[f32; 4]>)> = Vec::new();
    let mut cur: Vec<[f32; 4]> = (0..ww * wh)
        .map(|i| {
            let (x, y) = (x0 + i % ww, y0 + i / ww);
            let j = y * stride + x;
            if mask[j] {
                [0.0; 4]
            } else {
                [data[j * 3], data[j * 3 + 1], data[j * 3 + 2], 1.0]
            }
        })
        .collect();
    let (mut cw, mut ch) = (ww, wh);
    levels.push((cw, ch, cur.clone()));
    while cw > 1 || ch > 1 {
        let (nw, nh) = (cw.div_ceil(2), ch.div_ceil(2));
        let mut next = vec![[0f32; 4]; nw * nh];
        for y in 0..ch {
            for x in 0..cw {
                let v = cur[y * cw + x];
                let n = &mut next[(y / 2) * nw + x / 2];
                for c in 0..4 {
                    n[c] += v[c];
                }
            }
        }
        // Normalize so weights saturate at 1 (a fully known cell counts as known).
        for n in &mut next {
            if n[3] > 1.0 {
                let k = 1.0 / n[3];
                for c in 0..4 {
                    n[c] *= k;
                }
            }
        }
        levels.push((nw, nh, next.clone()));
        cur = next;
        cw = nw;
        ch = nh;
    }
    // Pull: fill missing weight at each level from the coarser one (bilinear).
    for l in (0..levels.len() - 1).rev() {
        let (cw, ch, ref coarse) = levels[l + 1].clone();
        let (fw, fh, fine) = &mut levels[l];
        for y in 0..*fh {
            for x in 0..*fw {
                let gx = ((x as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (cw - 1) as f32);
                let gy = ((y as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (ch - 1) as f32);
                let (ax, ay) = (gx.floor() as usize, gy.floor() as usize);
                let (bx, by) = ((ax + 1).min(cw - 1), (ay + 1).min(ch - 1));
                let (tx, ty) = (gx - ax as f32, gy - ay as f32);
                let mut up = [0f32; 4];
                for c in 0..4 {
                    let top = coarse[ay * cw + ax][c] * (1.0 - tx) + coarse[ay * cw + bx][c] * tx;
                    let bot = coarse[by * cw + ax][c] * (1.0 - tx) + coarse[by * cw + bx][c] * tx;
                    up[c] = top * (1.0 - ty) + bot * ty;
                }
                let f = &mut fine[y * *fw + x];
                let missing = (1.0 - f[3]).max(0.0);
                if up[3] > 1e-6 && missing > 0.0 {
                    let k = missing / up[3];
                    for c in 0..4 {
                        f[c] += up[c] * k;
                    }
                }
            }
        }
    }
    let (_, _, ref top) = levels[0];
    for y in 0..wh {
        for x in 0..ww {
            let j = (y0 + y) * stride + x0 + x;
            let v = top[y * ww + x];
            if mask[j] && v[3] > 1e-6 {
                for c in 0..3 {
                    data[j * 3 + c] = v[c] / v[3];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grainy flat negative with dust; IR is clean except for the same dust.
    fn scene(w: usize, h: usize, shift: (f32, f32)) -> (ImageF32, IrPlane, Vec<(usize, usize, usize)>) {
        let dust: Vec<(usize, usize, usize)> = (0..40).map(|i| ((i * 97 + 31) % (w - 40) + 20, (i * 61 + 17) % (h - 40) + 20, 2 + i % 4)).collect();
        let blocked = |x: f32, y: f32| dust.iter().any(|&(dx, dy, r)| (x - dx as f32).powi(2) + (y - dy as f32).powi(2) <= (r * r) as f32);
        let mut rgb = Vec::with_capacity(w * h * 3);
        let mut ir = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let grain = 1.0 + 0.03 * (((x * 7919 + y * 104729) % 97) as f32 / 97.0 - 0.5);
                let k = if blocked(x as f32, y as f32) { 0.05 } else { 1.0 };
                rgb.extend_from_slice(&[0.4 * grain * k, 0.2 * grain * k, 0.1 * grain * k]);
                // IR sees the dust displaced by `shift`.
                let ki = if blocked(x as f32 + shift.0, y as f32 + shift.1) { 0.05 } else { 1.0 };
                ir.push((0.75 * ki * 65535.0) as u16);
            }
        }
        (ImageF32 { width: w as u32, height: h as u32, data: rgb }, IrPlane { width: w as u32, height: h as u32, data: ir }, dust)
    }

    #[test]
    fn finds_ir_offset() {
        let (rgb, ir, _) = scene(900, 800, (2.0, -1.0));
        let al = estimate_alignment(&rgb, &ir);
        let (ix, iy) = al.ir_coords(450.0, 400.0);
        // RGB pixel p sees its dust in IR at p - shift.
        assert!((450.0 - ix - 2.0).abs() < 0.3 && (400.0 - iy + 1.0).abs() < 0.3, "{al:?}");
    }

    #[test]
    fn removes_dust_and_leaves_clean_pixels() {
        let (rgb, ir, dust) = scene(600, 500, (0.0, 0.0));
        let att = attenuation(&ir, &IrAlignment::default(), 600, 500);
        let out = clean(&rgb, &att, DustParams::default(), 1.0);
        for &(dx, dy, _) in &dust {
            let p = out.rgb(dy * 600 + dx);
            assert!((p[0] - 0.4).abs() < 0.03, "dust at {dx},{dy} left {p:?}");
        }
        // A clean pixel is untouched.
        let i = 3 * 600 + 300;
        assert_eq!(out.rgb(i), rgb.rgb(i));
    }

    #[test]
    fn broad_soft_mark_is_divided_out() {
        // Out-of-focus dust: a wide, soft 30% dip, the same in RGB and IR.
        let (w, h) = (640usize, 640usize);
        let dip = |i: usize| {
            let (x, y) = ((i % w) as f32 - 320.0, (i / w) as f32 - 320.0);
            1.0 - 0.3 * (-(x * x + y * y) / (2.0 * 5.0 * 5.0)).exp()
        };
        let rgb = ImageF32 { width: w as u32, height: h as u32, data: (0..w * h).flat_map(|i| [0.4 * dip(i), 0.2 * dip(i), 0.1 * dip(i)]).collect() };
        let ir = IrPlane { width: w as u32, height: h as u32, data: (0..w * h).map(|i| (0.75 * dip(i) * 65535.0) as u16).collect() };
        let att = attenuation(&ir, &IrAlignment::default(), w as u32, h as u32);
        let out = clean(&rgb, &att, DustParams::default(), 1.0);
        let p = out.rgb(320 * w + 320);
        assert!((p[0] - 0.4).abs() < 0.02 && (p[2] - 0.1).abs() < 0.005, "centre {p:?}");
    }
}
