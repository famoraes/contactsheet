use serde::{Deserialize, Serialize};

use crate::image::RgbSource;

/// Frame orientation: `rotation` quarter turns clockwise, then an optional
/// left-right mirror in display space. These two cover all 8 orientations
/// (a vertical flip is `mirror` + 180°).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Orientation {
    pub rotation: u8,
    pub mirror: bool,
}

impl Orientation {
    pub fn is_identity(self) -> bool {
        self.rotation % 4 == 0 && !self.mirror
    }

    /// Map a TIFF/EXIF Orientation tag (1-8) to the transform that displays it upright.
    pub fn from_tiff_tag(tag: u16) -> Self {
        let (rotation, mirror) = match tag {
            2 => (0, true),  // mirrored horizontally
            3 => (2, false), // rotated 180°
            4 => (2, true),  // mirrored vertically
            5 => (1, true),  // transposed
            6 => (1, false), // needs 90° clockwise
            7 => (3, true),  // transversed
            8 => (3, false), // needs 90° anticlockwise
            _ => (0, false),
        };
        Orientation { rotation, mirror }
    }

    /// Output dimensions for a `width` x `height` source.
    pub fn dims(self, width: u32, height: u32) -> (u32, u32) {
        if self.rotation % 2 == 1 {
            (height, width)
        } else {
            (width, height)
        }
    }
}

/// View of a source through an orientation; rendering code stays unaware.
pub struct Oriented<'a, S: RgbSource> {
    src: &'a S,
    rotation: u8,
    mirror: bool,
    sw: usize,
    sh: usize,
    ow: usize,
    oh: usize,
}

impl<'a, S: RgbSource> Oriented<'a, S> {
    pub fn new(src: &'a S, o: Orientation) -> Self {
        let (sw, sh) = (src.width(), src.height());
        let (ow, oh) = o.dims(sw, sh);
        Oriented {
            src,
            rotation: o.rotation % 4,
            mirror: o.mirror,
            sw: sw as usize,
            sh: sh as usize,
            ow: ow as usize,
            oh: oh as usize,
        }
    }

    /// Output pixel -> source pixel.
    #[inline]
    pub fn source_xy(&self, ox: usize, oy: usize) -> (usize, usize) {
        let ox = if self.mirror { self.ow - 1 - ox } else { ox };
        match self.rotation {
            0 => (ox, oy),
            1 => (oy, self.sh - 1 - ox),
            2 => (self.sw - 1 - ox, self.sh - 1 - oy),
            _ => (self.sw - 1 - oy, ox),
        }
    }
}

impl<S: RgbSource> RgbSource for Oriented<'_, S> {
    fn width(&self) -> u32 {
        self.ow as u32
    }
    fn height(&self) -> u32 {
        self.oh as u32
    }
    #[inline]
    fn rgb(&self, i: usize) -> [f32; 3] {
        let (sx, sy) = self.source_xy(i % self.ow, i / self.ow);
        self.src.rgb(sy * self.sw + sx)
    }
}

/// Crop in display space (after orientation), normalized to the oriented image.
///
/// `angle` straightens the image: the picture is rotated `angle` degrees clockwise
/// about its centre on a canvas of the same size, and `x, y, w, h` select a
/// rectangle on that canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub angle: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Crop { x: 0.0, y: 0.0, w: 1.0, h: 1.0, angle: 0.0 }
    }
}

/// Below this many degrees, straightening is ignored and cropping is a plain copy.
const ANGLE_EPSILON: f32 = 1e-3;

/// A source seen through orientation, straightening and crop - everything the
/// user sees on screen. Rendering, analysis and sampling all work on this view.
pub struct Framed<'a, S: RgbSource> {
    view: Oriented<'a, S>,
    vw: usize,
    vh: usize,
    ow: usize,
    oh: usize,
    /// Crop origin on the canvas, in pixels.
    x0: f32,
    y0: f32,
    cos: f32,
    sin: f32,
    straighten: bool,
}

impl<'a, S: RgbSource> Framed<'a, S> {
    pub fn new(src: &'a S, orientation: Orientation, crop: Option<Crop>) -> Self {
        let view = Oriented::new(src, orientation);
        let (vw, vh) = (view.width() as usize, view.height() as usize);
        let c = crop.unwrap_or_default();
        let x = c.x.clamp(0.0, 1.0);
        let y = c.y.clamp(0.0, 1.0);
        let w = c.w.clamp(0.0, 1.0 - x);
        let h = c.h.clamp(0.0, 1.0 - y);
        let straighten = c.angle.abs() > ANGLE_EPSILON;
        let (x0, y0) = if straighten {
            (x * vw as f32, y * vh as f32)
        } else {
            // Pixel-aligned so the copy path never resamples.
            ((x * vw as f32).round(), (y * vh as f32).round())
        };
        let ow = ((w * vw as f32).round() as usize).clamp(1, vw - (x0 as usize).min(vw - 1));
        let oh = ((h * vh as f32).round() as usize).clamp(1, vh - (y0 as usize).min(vh - 1));
        let (sin, cos) = c.angle.to_radians().sin_cos();
        Framed { view, vw, vh, ow, oh, x0, y0, cos, sin, straighten }
    }

    #[inline]
    fn at(&self, x: usize, y: usize) -> [f32; 3] {
        self.view.rgb(y * self.vw + x)
    }

    /// Bilinear sample at continuous pixel-centre coordinates, clamped to the edges.
    #[inline]
    fn bilinear(&self, fx: f32, fy: f32) -> [f32; 3] {
        let fx = fx.clamp(0.0, (self.vw - 1) as f32);
        let fy = fy.clamp(0.0, (self.vh - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.vw - 1), (y0 + 1).min(self.vh - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let (a, b, c, d) = (self.at(x0, y0), self.at(x1, y0), self.at(x0, y1), self.at(x1, y1));
        [0, 1, 2].map(|k| {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            top + (bottom - top) * ty
        })
    }
}

impl<S: RgbSource> RgbSource for Framed<'_, S> {
    fn width(&self) -> u32 {
        self.ow as u32
    }
    fn height(&self) -> u32 {
        self.oh as u32
    }
    #[inline]
    fn rgb(&self, i: usize) -> [f32; 3] {
        let (i, j) = (i % self.ow, i / self.ow);
        if !self.straighten {
            return self.at(self.x0 as usize + i, self.y0 as usize + j);
        }
        // Canvas point -> oriented image point: undo the clockwise rotation about the centre.
        let (cx, cy) = (self.vw as f32 / 2.0, self.vh as f32 / 2.0);
        let dx = self.x0 + i as f32 + 0.5 - cx;
        let dy = self.y0 + j as f32 + 0.5 - cy;
        let sx = cx + dx * self.cos + dy * self.sin;
        let sy = cy - dx * self.sin + dy * self.cos;
        self.bilinear(sx - 0.5, sy - 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::ImageF32;

    /// 3x2 image whose red channel holds the pixel index:
    /// 0 1 2
    /// 3 4 5
    fn sample() -> ImageF32 {
        let data = (0..6).flat_map(|i| [i as f32, 0.0, 0.0]).collect();
        ImageF32 { width: 3, height: 2, data }
    }

    fn grid(o: Orientation) -> (u32, u32, Vec<u32>) {
        let img = sample();
        let v = Oriented::new(&img, o);
        let n = (v.width() * v.height()) as usize;
        (v.width(), v.height(), (0..n).map(|i| v.rgb(i)[0] as u32).collect())
    }

    #[test]
    fn tiff_tags_match_their_definitions() {
        // 3x2 source; check where the stored top-left pixel (0) ends up on screen.
        let img = sample();
        let top_left_of = |tag| {
            let v = Oriented::new(&img, Orientation::from_tiff_tag(tag));
            let n = (v.width() * v.height()) as usize;
            let pos = (0..n).position(|i| v.rgb(i)[0] == 0.0).unwrap();
            let w = v.width() as usize;
            (pos % w, pos / w, v.width(), v.height())
        };
        // Stored row 0 is the visual: 1 top, 2 top (mirrored), 3 bottom, 4 bottom (mirrored),
        // 5 left column, 6 right column, 7 right column (from bottom), 8 left column (from bottom).
        assert_eq!(top_left_of(1), (0, 0, 3, 2));
        assert_eq!(top_left_of(2), (2, 0, 3, 2));
        assert_eq!(top_left_of(3), (2, 1, 3, 2));
        assert_eq!(top_left_of(4), (0, 1, 3, 2));
        assert_eq!(top_left_of(5), (0, 0, 2, 3));
        assert_eq!(top_left_of(6), (1, 0, 2, 3));
        assert_eq!(top_left_of(7), (1, 2, 2, 3));
        assert_eq!(top_left_of(8), (0, 2, 2, 3));
    }

    fn values<S: RgbSource>(v: &S) -> (u32, u32, Vec<f32>) {
        let n = (v.width() * v.height()) as usize;
        (v.width(), v.height(), (0..n).map(|i| v.rgb(i)[0]).collect())
    }

    fn square4() -> ImageF32 {
        // 0..15 row-major
        ImageF32 { width: 4, height: 4, data: (0..16).flat_map(|i| [i as f32, 0.0, 0.0]).collect() }
    }

    #[test]
    fn framed_without_crop_matches_oriented() {
        let img = sample();
        let o = Orientation { rotation: 1, mirror: true };
        assert_eq!(values(&Framed::new(&img, o, None)), values(&Oriented::new(&img, o)));
    }

    #[test]
    fn integer_crop_copies_pixels() {
        let img = square4();
        let crop = Crop { x: 0.25, y: 0.25, w: 0.5, h: 0.5, angle: 0.0 };
        assert_eq!(values(&Framed::new(&img, Orientation::default(), Some(crop))), (2, 2, vec![5.0, 6.0, 9.0, 10.0]));
    }

    #[test]
    fn straighten_rotates_clockwise_like_css() {
        // A 90° straighten of a square must equal a quarter turn clockwise.
        let img = square4();
        let crop = Crop { angle: 90.0, ..Crop::default() };
        let (w, h, got) = values(&Framed::new(&img, Orientation::default(), Some(crop)));
        let (_, _, want) = values(&Oriented::new(&img, Orientation { rotation: 1, mirror: false }));
        assert_eq!((w, h), (4, 4));
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-3, "{got:?}");
        }
    }

    #[test]
    fn crop_applies_after_orientation() {
        // Rotated 90° CW, the 3x2 sample becomes 2x3 [3 0 / 4 1 / 5 2]; take the top row.
        let img = sample();
        let crop = Crop { x: 0.0, y: 0.0, w: 1.0, h: 1.0 / 3.0, angle: 0.0 };
        let o = Orientation { rotation: 1, mirror: false };
        assert_eq!(values(&Framed::new(&img, o, Some(crop))), (2, 1, vec![3.0, 0.0]));
    }

    #[test]
    fn all_orientations_map_correctly() {
        let o = |rotation, mirror| Orientation { rotation, mirror };
        assert_eq!(grid(o(0, false)), (3, 2, vec![0, 1, 2, 3, 4, 5]));
        // 90° clockwise: left column (bottom to top) becomes the top row.
        assert_eq!(grid(o(1, false)), (2, 3, vec![3, 0, 4, 1, 5, 2]));
        assert_eq!(grid(o(2, false)), (3, 2, vec![5, 4, 3, 2, 1, 0]));
        assert_eq!(grid(o(3, false)), (2, 3, vec![2, 5, 1, 4, 0, 3]));
        // Mirror = flip horizontally.
        assert_eq!(grid(o(0, true)), (3, 2, vec![2, 1, 0, 5, 4, 3]));
        // Mirror + 180° = flip vertically.
        assert_eq!(grid(o(2, true)), (3, 2, vec![3, 4, 5, 0, 1, 2]));
        assert_eq!(grid(o(1, true)), (2, 3, vec![0, 3, 1, 4, 2, 5]));
    }
}
