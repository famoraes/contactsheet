use rayon::prelude::*;

/// Anything that can hand out linear RGB pixels by linear index (`y * width + x`).
pub trait RgbSource: Sync {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn rgb(&self, i: usize) -> [f32; 3];
}

/// Interleaved linear RGB f32 image (used for preview proxies and thumbnails).
#[derive(Clone)]
pub struct ImageF32 {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

impl RgbSource for ImageF32 {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    #[inline]
    fn rgb(&self, i: usize) -> [f32; 3] {
        let b = i * 3;
        [self.data[b], self.data[b + 1], self.data[b + 2]]
    }
}

/// Box-filter downsample by an integer factor so the long edge is <= `max_long`.
/// Averaging happens on linear transmittance, which is physically correct.
pub fn downsample<S: RgbSource>(src: &S, max_long: u32) -> ImageF32 {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let max_long = max_long.max(1) as usize;
    let f = w.max(h).div_ceil(max_long).max(1);
    let ow = (w / f).max(1);
    let oh = (h / f).max(1);
    let mut data = vec![0f32; ow * oh * 3];
    data.par_chunks_mut(ow * 3).enumerate().for_each(|(oy, row)| {
        let y0 = oy * f;
        let y1 = ((oy + 1) * f).min(h);
        for ox in 0..ow {
            let x0 = ox * f;
            let x1 = ((ox + 1) * f).min(w);
            let mut acc = [0f32; 3];
            let mut n = 0u32;
            for y in y0..y1 {
                let row_base = y * w;
                for x in x0..x1 {
                    let p = src.rgb(row_base + x);
                    acc[0] += p[0];
                    acc[1] += p[1];
                    acc[2] += p[2];
                    n += 1;
                }
            }
            let inv = 1.0 / n.max(1) as f32;
            row[ox * 3] = acc[0] * inv;
            row[ox * 3 + 1] = acc[1] * inv;
            row[ox * 3 + 2] = acc[2] * inv;
        }
    });
    ImageF32 {
        width: ow as u32,
        height: oh as u32,
        data,
    }
}
