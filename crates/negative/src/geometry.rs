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
