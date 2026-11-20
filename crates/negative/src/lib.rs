//! Film negative inversion core.
//!
//! Pipeline overview (all per channel, on linear transmittance):
//!
//! 1. decode TIFF -> linear samples in [0, 1]
//! 2. density `D = -log10(T)`
//! 3. analysis finds per-channel density black/white points (`lo`/`hi`) inside
//!    the analysis region; normalizing `E = (D - lo) / (hi - lo)` removes the
//!    orange mask and the per-layer gamma differences in one step
//! 4. user adjustments act on `E` the way an enlarger acts on log exposure
//!    (density = global offset, C/M/Y = per-channel offsets, contrast = slope)
//! 5. `E` is mapped to scene-linear light, then sRGB-encoded with a gentle
//!    paper S-curve.
//!
//! Because steps 2-3 are log-domain and normalized per channel, any constant
//! scale (film base, light source brightness, 12- vs 16-bit range) and any pure
//! power-law input gamma cancel out. That makes the pipeline robust to scans
//! from cameras, scanners and converters alike.

pub mod analysis;
pub mod autocrop;
pub mod decode;
pub mod error;
pub mod export;
pub mod geometry;
pub mod image;
pub mod metadata;
pub mod neutral;
pub mod pipeline;
pub mod render;
pub mod synth;

pub use analysis::{analyze, Analysis, Rect};
pub use decode::{decode_tiff, RawImage};
pub use error::{Error, Result};
pub use geometry::{Crop, Framed, Orientation, Oriented};
pub use image::{downsample, ImageF32, RgbSource};
pub use pipeline::{Adjustments, Look, LookPreset, Prepared, ViewOptions};
