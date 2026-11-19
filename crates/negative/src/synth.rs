//! Synthetic colour negatives for tests and demo files.
//!
//! Model: `T_c = light * mask_c * (L_c / L_MIN)^(-gamma_c)`, so each dye layer has
//! its own base density (orange mask) and its own gamma, as with real C-41 film.

use crate::image::ImageF32;

pub const L_MIN: f32 = 0.004;
pub const MASK: [f32; 3] = [0.80, 0.50, 0.28];
pub const GAMMA: [f32; 3] = [0.60, 0.65, 0.72];
pub const LIGHT: f32 = 0.92;

/// Transmittance of the negative for a scene-linear RGB value.
pub fn negative_of(scene: [f32; 3]) -> [f32; 3] {
    let mut t = [0f32; 3];
    for c in 0..3 {
        let l = scene[c].clamp(L_MIN, 1.0);
        t[c] = LIGHT * MASK[c] * (l / L_MIN).powf(-GAMMA[c]);
    }
    t
}

/// Scene luminance for position `t` in 0..1 on a log ramp from `L_MIN` to 1.
pub fn log_ramp(t: f32) -> f32 {
    L_MIN * (1.0 / L_MIN).powf(t.clamp(0.0, 1.0))
}

/// A demo frame: unexposed film border, a neutral log ramp, colour patches and
/// a soft gradient, all rendered as an orange-masked negative.
pub fn demo_negative(width: u32, height: u32) -> ImageF32 {
    let (w, h) = (width as usize, height as usize);
    let border = (w.min(h) as f32 * 0.05) as usize;
    let patches: [[f32; 3]; 6] = [
        [0.40, 0.08, 0.06], // red
        [0.08, 0.30, 0.07], // green
        [0.05, 0.09, 0.35], // blue
        [0.45, 0.35, 0.06], // yellow
        [0.06, 0.25, 0.32], // cyan
        [0.42, 0.24, 0.17], // skin
    ];
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let scene = if x < border || y < border || x >= w - border || y >= h - border {
                [0.0; 3] // unexposed rebate -> film base
            } else {
                let u = (x - border) as f32 / (w - 2 * border) as f32;
                let v = (y - border) as f32 / (h - 2 * border) as f32;
                if v < 0.4 {
                    [log_ramp(u); 3]
                } else if v < 0.7 {
                    let i = ((u * 6.0) as usize).min(5);
                    patches[i]
                } else {
                    let l = log_ramp(0.3 + 0.6 * u);
                    [l * (1.0 - 0.5 * v), l * 0.8, l * (0.4 + 0.6 * v)]
                }
            };
            data.extend_from_slice(&negative_of(scene));
        }
    }
    ImageF32 { width, height, data }
}
