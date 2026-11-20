use rayon::prelude::*;

use crate::{
    image::RgbSource,
    pipeline::{srgb_encode, Prepared, ViewOptions, CLIP_HIGHLIGHT},
};

const CLIP_HIGHLIGHT_COLOR: [u8; 3] = [255, 40, 40];
const CLIP_SHADOW_COLOR: [u8; 3] = [40, 110, 255];

#[inline]
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline]
fn to_u16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
}

#[inline]
fn view_pixel(prep: &Prepared, view: ViewOptions, px: [f32; 3]) -> [u8; 3] {
    if view.show_negative {
        return px.map(|v| to_u8(srgb_encode(v.clamp(0.0, 1.0))));
    }
    let (rgb, clip) = prep.apply(px);
    if view.show_clipping && clip != 0 {
        return if clip & CLIP_HIGHLIGHT != 0 {
            CLIP_HIGHLIGHT_COLOR
        } else {
            CLIP_SHADOW_COLOR
        };
    }
    rgb.map(to_u8)
}

/// Preview buffer: 8-byte header (width, height as little-endian u32) + RGBA8 pixels.
pub fn render_preview<S: RgbSource>(src: &S, prep: &Prepared, view: ViewOptions) -> Vec<u8> {
    render_rgba(src, prep, view, &[src.width(), src.height()])
}

/// RGBA8 pixels after a header of little-endian u32 values.
pub fn render_rgba<S: RgbSource>(src: &S, prep: &Prepared, view: ViewOptions, header: &[u32]) -> Vec<u8> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let hl = header.len() * 4;
    let mut out = vec![0u8; hl + w * h * 4];
    let (head, pixels) = out.split_at_mut(hl);
    for (i, v) in header.iter().enumerate() {
        head[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    pixels.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let [r, g, b] = view_pixel(prep, view, src.rgb(y * w + x));
            row[x * 4..x * 4 + 4].copy_from_slice(&[r, g, b, 255]);
        }
    });
    out
}

/// Interleaved RGB8 positive (thumbnails, JPEG export).
pub fn render_rgb8<S: RgbSource>(src: &S, prep: &Prepared) -> Vec<u8> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut out = vec![0u8; w * h * 3];
    out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (rgb, _) = prep.apply(src.rgb(y * w + x));
            row[x * 3..x * 3 + 3].copy_from_slice(&rgb.map(to_u8));
        }
    });
    out
}

/// Interleaved RGB16 positive (TIFF export).
pub fn render_rgb16<S: RgbSource>(src: &S, prep: &Prepared) -> Vec<u16> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut out = vec![0u16; w * h * 3];
    out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (rgb, _) = prep.apply(src.rgb(y * w + x));
            row[x * 3..x * 3 + 3].copy_from_slice(&rgb.map(to_u16));
        }
    });
    out
}
