//! Camera RAW and DNG input (camera scanning), via `rawler`.
//!
//! The raw data is only taken as far as linear camera RGB: black/white level
//! scaling, demosaicing and the sensor's active area. White balance, the camera
//! colour matrix and gamma are deliberately skipped - for a negative they would
//! only be undone by the per-channel density normalization, like a scanner's
//! "raw" output.

use std::path::Path;

use crate::{decode::RawImage, Error, Result};

/// File extensions routed to the RAW decoder.
pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "rwl", "pef", "srw", "3fr",
    "fff", "iiq", "erf", "mef", "mos", "mrw", "kdc", "dcr",
];

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .map(|e| RAW_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(feature = "raw")]
pub fn decode_raw(path: &Path) -> Result<RawImage> {
    use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};

    use crate::{decode::Samples, geometry::Orientation};
    use rawler::{decoders::RawDecodeParams, rawsource::RawSource};

    let err = |e: rawler::RawlerError| Error::msg(format!("RAW decode: {e}"));
    let source = RawSource::new(path)?;
    let decoder = rawler::get_decoder(&source).map_err(err)?;
    let params = RawDecodeParams::default();
    let raw = decoder.raw_image(&source, &params, false).map_err(err)?;
    // Many formats keep the camera orientation in EXIF rather than on the raw image.
    let exif_orientation = decoder.raw_metadata(&source, &params).ok().and_then(|m| m.exif.orientation);
    let orientation = Orientation::from_tiff_tag(exif_orientation.unwrap_or_else(|| raw.orientation.to_u16()));
    let bit_depth = raw.bps.clamp(8, 16) as u8;
    let develop = RawDevelop::new_with(&[
        ProcessingStep::Rescale,
        ProcessingStep::Demosaic,
        ProcessingStep::CropActiveArea,
        ProcessingStep::CropDefault,
    ]);
    // Unusual sensors hit `todo!()`s inside the decoder; report them as errors.
    let developed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| develop.develop_intermediate(&raw)))
        .map_err(|_| Error::msg("RAW decode: unsupported sensor layout"))?
        .map_err(|e| Error::msg(format!("RAW develop: {e}")))?;
    let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    let (w, h, samples) = match developed {
        Intermediate::ThreeColor(px) => {
            let d = px.dim();
            (d.w, d.h, px.data.iter().flat_map(|p| p.map(q)).collect::<Vec<u16>>())
        }
        Intermediate::Monochrome(px) => {
            let d = px.dim();
            (d.w, d.h, px.data.iter().flat_map(|&v| [q(v); 3]).collect())
        }
        Intermediate::FourColor(_) => return Err(Error::msg("RAW decode: four-colour sensors are not supported")),
    };
    Ok(RawImage {
        width: w as u32,
        height: h as u32,
        channels: 3,
        bit_depth,
        samples: Samples::U16(samples),
        orientation,
    })
}

#[cfg(not(feature = "raw"))]
pub fn decode_raw(_path: &Path) -> Result<RawImage> {
    Err(Error::msg("this build has no RAW support"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_extensions() {
        assert!(is_raw(Path::new("/a/IMG_0001.CR3")));
        assert!(is_raw(Path::new("/a/scan.dng")));
        assert!(!is_raw(Path::new("/a/scan.tif")));
        assert!(!is_raw(Path::new("/a/noext")));
    }
}
