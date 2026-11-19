use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;

/// User adjustments. All sliders are in -1..=1 with 0 = automatic result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Adjustments {
    /// Print density: + = darker (more paper exposure), - = lighter.
    pub density: f32,
    /// Contrast: maps to a slope of 2^contrast (0.5x..2x).
    pub contrast: f32,
    /// Saturation: -1 = monochrome, +1 = double.
    pub saturation: f32,
    /// + = more cyan (less red).
    pub cyan: f32,
    /// + = more magenta (less green).
    pub magenta: f32,
    /// + = more yellow (less blue).
    pub yellow: f32,
    /// True monochrome output.
    pub grayscale: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewOptions {
    /// Show the untouched negative (before/after).
    pub show_negative: bool,
    /// Highlight clipped pixels (red = highlights, blue = shadows).
    pub show_clipping: bool,
}

/// The "paper": how normalized exposure becomes print tones. Presets pick one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    /// log10 scene range the normalized density range is mapped onto.
    /// Larger = deeper blacks and more contrast in the shadows.
    pub print_range: f32,
    /// Display-space S-curve strength (0 = none, < 1 keeps it monotonic).
    pub paper_s: f32,
}

impl Look {
    /// Punchy but with open shadows (tuned on Epson V800 / SilverFast colour scans).
    pub const NORMAL: Look = Look { print_range: 2.0, paper_s: 0.15 };
    /// Lower contrast, more shadow detail; a good base for further editing.
    pub const FLAT: Look = Look { print_range: 1.7, paper_s: 0.05 };
}

impl Default for Look {
    fn default() -> Self {
        Look::NORMAL
    }
}

/// Contrast pivot in normalized exposure; ~ where 18% grey lands.
const PIVOT: f32 = 0.7;
/// Normalized exposure shift per unit of the density slider (~1.8 stops at the extremes).
const DENSITY_STEP: f32 = 0.25;
/// Normalized exposure shift per unit of a C/M/Y slider.
const COLOR_STEP: f32 = 0.08;
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

pub const CLIP_SHADOW: u8 = 1;
pub const CLIP_HIGHLIGHT: u8 = 2;

/// Analysis + adjustments folded into a per-channel power law.
///
/// With `D = -log10(x)` and `E'' = a*D + b`, scene light is
/// `10^((E'' - 1) * R) = x^(-a*R) * 10^((b - 1) * R)`, so each channel costs one `powf`.
#[derive(Clone, Copy, Debug)]
pub struct Prepared {
    exponent: [f32; 3],
    scale: [f32; 3],
    black: f32,
    inv_range: f32,
    saturation: f32,
    grayscale: bool,
    paper_s: f32,
}

impl Prepared {
    pub fn new(an: &Analysis, adj: &Adjustments) -> Self {
        Self::with_look(an, adj, &Look::default())
    }

    pub fn with_look(an: &Analysis, adj: &Adjustments, look: &Look) -> Self {
        let range = look.print_range.max(0.5);
        let k = 2f32.powf(adj.contrast.clamp(-1.0, 1.0));
        let dens = adj.density * DENSITY_STEP;
        let offsets = [
            -dens - adj.cyan * COLOR_STEP,
            -dens - adj.magenta * COLOR_STEP,
            -dens - adj.yellow * COLOR_STEP,
        ];
        let mut exponent = [0f32; 3];
        let mut scale = [0f32; 3];
        for c in 0..3 {
            let span = (an.hi[c] - an.lo[c]).max(1e-3);
            let a = k / span;
            let b = (-an.lo[c] / span + offsets[c] - PIVOT) * k + PIVOT;
            exponent[c] = -a * range;
            scale[c] = 10f32.powf((b - 1.0) * range);
        }
        let black = 10f32.powf(-range);
        Prepared {
            exponent,
            scale,
            black,
            inv_range: 1.0 / (1.0 - black),
            saturation: (1.0 + adj.saturation).max(0.0),
            grayscale: adj.grayscale,
            paper_s: look.paper_s.clamp(0.0, 0.95),
        }
    }

    /// Negative linear RGB -> unclamped scene-linear positive RGB.
    #[inline]
    pub fn linear(&self, px: [f32; 3]) -> [f32; 3] {
        let mut l = [0f32; 3];
        for c in 0..3 {
            let x = px[c].max(1e-6);
            l[c] = (self.scale[c] * x.powf(self.exponent[c]) - self.black) * self.inv_range;
        }
        let y = l[0] * LUMA[0] + l[1] * LUMA[1] + l[2] * LUMA[2];
        if self.grayscale {
            return [y, y, y];
        }
        if self.saturation != 1.0 {
            for v in &mut l {
                *v = y + self.saturation * (*v - y);
            }
        }
        l
    }

    /// Negative linear RGB -> display-encoded positive in [0, 1] plus clip flags.
    #[inline]
    pub fn apply(&self, px: [f32; 3]) -> ([f32; 3], u8) {
        let l = self.linear(px);
        let mut clip = 0u8;
        let mut out = [0f32; 3];
        for c in 0..3 {
            if l[c] <= 0.0 {
                clip |= CLIP_SHADOW;
            } else if l[c] >= 1.0 {
                clip |= CLIP_HIGHLIGHT;
            }
            out[c] = paper(srgb_encode(l[c].clamp(0.0, 1.0)), self.paper_s);
        }
        (out, clip)
    }
}

#[inline]
pub fn srgb_encode(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Cubic S-curve that keeps 0, 0.5 and 1 fixed.
#[inline]
fn paper(y: f32, s: f32) -> f32 {
    (y + s * y * (1.0 - y) * (2.0 * y - 1.0)).clamp(0.0, 1.0)
}
