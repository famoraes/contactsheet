use std::{fs::File, io::BufReader, path::Path};

use tiff::{
    decoder::{Decoder, DecodingResult, Limits},
    tags::Tag,
    ColorType,
};

use crate::{geometry::Orientation, image::RgbSource, ir::IrPlane, Error, Result};

/// Raw decoded samples, kept in their native type to avoid a full-size f32 copy.
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

/// A decoded scan. Samples are interleaved; `channels` is 1 (gray), 2 (gray+alpha),
/// 3 (RGB) or 4 (RGBA). Alpha is ignored.
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub bit_depth: u8,
    pub samples: Samples,
    /// From the TIFF Orientation tag: how the stored pixels should be displayed.
    pub orientation: Orientation,
    /// Infrared channel, when the scanner saved one (for dust removal).
    pub ir: Option<IrPlane>,
}

/// Decode a scan: TIFF, or camera RAW / DNG by file extension.
pub fn decode_image(path: &Path) -> Result<RawImage> {
    if crate::raw::is_raw(path) {
        crate::raw::decode_raw(path)
    } else {
        decode_tiff(path)
    }
}

pub fn decode_tiff(path: &Path) -> Result<RawImage> {
    let file = File::open(path)?;
    let mut dec = Decoder::new(BufReader::new(file))?.with_limits(Limits::unlimited());
    let (width, height) = dec.dimensions()?;
    let orientation = dec
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .ok()
        .flatten()
        .map(Orientation::from_tiff_tag)
        .unwrap_or_default();
    let (channels, bit_depth) = match dec.colortype()? {
        ColorType::Gray(b) => (1, b),
        ColorType::GrayA(b) => (2, b),
        ColorType::RGB(b) => (3, b),
        ColorType::RGBA(b) => (4, b),
        other => {
            return Err(Error::msg(format!(
                "unsupported TIFF color type {other:?} (expected RGB or grayscale)"
            )))
        }
    };
    let samples = match dec.read_image()? {
        DecodingResult::U8(v) => Samples::U8(v),
        DecodingResult::U16(v) => Samples::U16(v),
        DecodingResult::F32(v) => Samples::F32(v),
        DecodingResult::F64(v) => Samples::F32(v.into_iter().map(|x| x as f32).collect()),
        DecodingResult::U32(v) => Samples::F32(
            v.into_iter()
                .map(|x| (x as f64 / u32::MAX as f64) as f32)
                .collect(),
        ),
        _ => return Err(Error::msg("unsupported TIFF sample format")),
    };
    let expected = width as usize * height as usize * channels;
    let got = match &samples {
        Samples::U8(v) => v.len(),
        Samples::U16(v) => v.len(),
        Samples::F32(v) => v.len(),
    };
    if got < expected {
        return Err(Error::msg(format!(
            "TIFF has {got} samples, expected {expected} (planar or tiled layouts may be unsupported)"
        )));
    }
    // Infrared: either a 4th channel that isn't a real (constant) alpha, or a
    // later full-size grayscale page (how some scanner software stores it).
    let mut ir = if channels == 4 { fourth_channel_ir(&samples, width, height) } else { None };
    while ir.is_none() && dec.more_images() {
        if dec.next_image().is_err() {
            break;
        }
        let gray = matches!(dec.colortype(), Ok(ColorType::Gray(8 | 16)));
        if !gray || dec.dimensions().ok() != Some((width, height)) {
            continue;
        }
        ir = match dec.read_image() {
            Ok(DecodingResult::U16(v)) => Some(IrPlane { width, height, data: v }),
            Ok(DecodingResult::U8(v)) => Some(IrPlane { width, height, data: v.into_iter().map(|x| x as u16 * 257).collect() }),
            _ => None,
        };
    }
    Ok(RawImage {
        width,
        height,
        channels,
        bit_depth,
        samples,
        orientation,
        ir,
    })
}

/// The 4th channel as IR, unless it is a constant (genuine, unused) alpha channel.
fn fourth_channel_ir(samples: &Samples, width: u32, height: u32) -> Option<IrPlane> {
    let n = width as usize * height as usize;
    let data: Vec<u16> = match samples {
        Samples::U16(v) => (0..n).map(|i| v[i * 4 + 3]).collect(),
        Samples::U8(v) => (0..n).map(|i| v[i * 4 + 3] as u16 * 257).collect(),
        Samples::F32(v) => (0..n).map(|i| (v[i * 4 + 3].clamp(0.0, 1.0) * 65535.0 + 0.5) as u16).collect(),
    };
    let (lo, hi) = data.iter().step_by(97).fold((u16::MAX, 0), |(a, b), &x| (a.min(x), b.max(x)));
    (hi > lo).then_some(IrPlane { width, height, data })
}

#[inline(always)]
fn fetch<T: Copy + Into<f32>>(v: &[T], base: usize, channels: usize, scale: f32) -> [f32; 3] {
    if channels >= 3 {
        [
            v[base].into() * scale,
            v[base + 1].into() * scale,
            v[base + 2].into() * scale,
        ]
    } else {
        let g = v[base].into() * scale;
        [g, g, g]
    }
}

impl RgbSource for RawImage {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    #[inline]
    fn rgb(&self, i: usize) -> [f32; 3] {
        let base = i * self.channels;
        match &self.samples {
            Samples::U8(v) => fetch(v, base, self.channels, 1.0 / 255.0),
            Samples::U16(v) => fetch(v, base, self.channels, 1.0 / 65535.0),
            Samples::F32(v) => fetch(v, base, self.channels, 1.0),
        }
    }
}
