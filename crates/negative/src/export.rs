use std::{
    borrow::Cow,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, ExtendedColorType, ImageBuffer, ImageEncoder, Rgb,
};
use tiff::{
    encoder::{colortype, TiffEncoder, TiffValue},
    tags::{Tag, Type},
};

use crate::{
    analysis::Analysis,
    decode::decode_image,
    geometry::{Crop, Framed, Orientation},
    image::{ImageF32, RgbSource},
    metadata::{jpeg_with_xmp, Embed, SOFTWARE},
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

/// Tag 700: XMP packet (TIFF/XMP spec, type BYTE).
const TAG_XMP: Tag = Tag::Unknown(700);
/// Tag 34675: embedded ICC profile (type UNDEFINED).
const TAG_ICC: Tag = Tag::Unknown(34675);

/// Exports are sRGB; this profile says so.
fn srgb_icc() -> &'static [u8] {
    static ICC: OnceLock<Vec<u8>> = OnceLock::new();
    ICC.get_or_init(crate::icc::srgb_profile)
}

/// Opaque bytes written with TIFF type UNDEFINED.
struct Undefined<'a>(&'a [u8]);

impl TiffValue for Undefined<'_> {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::UNDEFINED;
    fn count(&self) -> usize {
        self.0.len()
    }
    fn data(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.0)
    }
}

pub fn write_tiff16(path: &Path, width: u32, height: u32, rgb: &[u16], embed: Option<&Embed>) -> Result<()> {
    let mut enc = TiffEncoder::new(BufWriter::new(File::create(path)?))?;
    let mut img = enc.new_image::<colortype::RGB16>(width, height)?;
    let dir = img.encoder();
    dir.write_tag(Tag::Software, SOFTWARE)?;
    dir.write_tag(TAG_ICC, Undefined(srgb_icc()))?;
    if let Some(e) = embed {
        for (tag, value) in [
            (Tag::ImageDescription, &e.image_description),
            (Tag::Model, &e.model),
            (Tag::Artist, &e.artist),
            (Tag::Copyright, &e.copyright),
        ] {
            if let Some(v) = value {
                dir.write_tag(tag, v.as_str())?;
            }
        }
        dir.write_tag(TAG_XMP, e.xmp.as_bytes())?;
    }
    img.write_data(rgb)?;
    Ok(())
}

pub fn write_jpeg(path: &Path, width: u32, height: u32, rgb: &[u8], quality: u8, embed: Option<&Embed>) -> Result<()> {
    let mut bytes = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut bytes, quality.clamp(1, 100));
    enc.set_icc_profile(srgb_icc().to_vec()).expect("JPEG encoder supports ICC profiles");
    enc.write_image(rgb, width, height, ExtendedColorType::Rgb8)?;
    if let Some(with) = embed.and_then(|e| jpeg_with_xmp(&bytes, &e.xmp)) {
        bytes = with;
    }
    std::fs::write(path, bytes)?;
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
    embed: Option<&Embed>,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    if settings.tiff16 {
        let path = settings.out_dir.join(format!("{stem}.tif"));
        write_tiff16(&path, src.width(), src.height(), &render_rgb16(src, prep), embed)?;
        written.push(path);
    }
    if let Some(q) = settings.jpeg_quality {
        let path = settings.out_dir.join(format!("{stem}.jpg"));
        write_jpeg(&path, src.width(), src.height(), &render_rgb8(src, prep), q, embed)?;
        written.push(path);
    }
    Ok(written)
}

/// Decode the full-resolution scan again and write the positive(s), embedding
/// `embed` metadata when given.
pub fn export_file(
    input: &Path,
    analysis: &Analysis,
    adjustments: &Adjustments,
    orientation: Orientation,
    crop: Option<Crop>,
    settings: &ExportSettings,
    embed: Option<&Embed>,
) -> Result<Vec<PathBuf>> {
    let raw = decode_image(input)?;
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
        Some(small) => write_outputs(&small, &prep, &stem, settings, embed),
        None => write_outputs(&framed, &prep, &stem, settings, embed),
    }
}
