use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use ::image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, ExtendedColorType, ImageBuffer, Rgb,
};
use tiff::encoder::{colortype, TiffEncoder};

use crate::{
    analysis::Analysis,
    decode::decode_tiff,
    geometry::{Crop, Framed, Orientation},
    image::{ImageF32, RgbSource},
    pipeline::{Adjustments, Prepared},
    render::{render_rgb16, render_rgb8},
    Result,
};

#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub out_dir: PathBuf,
    pub tiff16: bool,
    /// JPEG quality 1..=100, `None` to skip JPEG.
    pub jpeg_quality: Option<u8>,
    /// Downscale so the long edge is at most this many pixels.
    pub max_size: Option<u32>,
}

pub fn write_tiff16(path: &Path, width: u32, height: u32, rgb: &[u16]) -> Result<()> {
    let mut enc = TiffEncoder::new(BufWriter::new(File::create(path)?))?;
    enc.write_image::<colortype::RGB16>(width, height, rgb)?;
    Ok(())
}

pub fn write_jpeg(path: &Path, width: u32, height: u32, rgb: &[u8], quality: u8) -> Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    JpegEncoder::new_with_quality(&mut w, quality.clamp(1, 100)).encode(
        rgb,
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

pub fn encode_jpeg(width: u32, height: u32, rgb: &[u8], quality: u8) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100)).encode(
        rgb,
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

/// Lanczos resize of linear negative data so the long edge is `max_long`.
fn resize_to<S: RgbSource>(src: &S, max_long: u32) -> Option<ImageF32> {
    let (w, h) = (src.width(), src.height());
    let long = w.max(h);
    if max_long == 0 || long <= max_long {
        return None;
    }
    let s = max_long as f64 / long as f64;
    let nw = ((w as f64 * s).round() as u32).max(1);
    let nh = ((h as f64 * s).round() as u32).max(1);
    let n = w as usize * h as usize;
    let mut data = Vec::with_capacity(n * 3);
    for i in 0..n {
        data.extend_from_slice(&src.rgb(i));
    }
    let buf: ImageBuffer<Rgb<f32>, Vec<f32>> = ImageBuffer::from_raw(w, h, data)?;
    let resized = ::image::imageops::resize(&buf, nw, nh, FilterType::Lanczos3);
    Some(ImageF32 {
        width: nw,
        height: nh,
        data: resized.into_raw(),
    })
}

fn write_outputs<S: RgbSource>(
    src: &S,
    prep: &Prepared,
    stem: &str,
    settings: &ExportSettings,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    if settings.tiff16 {
        let path = settings.out_dir.join(format!("{stem}.tif"));
        write_tiff16(&path, src.width(), src.height(), &render_rgb16(src, prep))?;
        written.push(path);
    }
    if let Some(q) = settings.jpeg_quality {
        let path = settings.out_dir.join(format!("{stem}.jpg"));
        write_jpeg(&path, src.width(), src.height(), &render_rgb8(src, prep), q)?;
        written.push(path);
    }
    Ok(written)
}

/// Decode the full-resolution scan again and write the positive(s).
pub fn export_file(
    input: &Path,
    analysis: &Analysis,
    adjustments: &Adjustments,
    orientation: Orientation,
    crop: Option<Crop>,
    settings: &ExportSettings,
) -> Result<Vec<PathBuf>> {
    let raw = decode_tiff(input)?;
    let prep = Prepared::new(analysis, adjustments);
    let stem = format!(
        "{}_positive",
        input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into())
    );
    std::fs::create_dir_all(&settings.out_dir)?;
    // Geometry first, then resize, so the size limit applies to the final frame.
    let framed = Framed::new(&raw, orientation, crop);
    match settings.max_size.and_then(|m| resize_to(&framed, m)) {
        Some(small) => write_outputs(&small, &prep, &stem, settings),
        None => write_outputs(&framed, &prep, &stem, settings),
    }
}
