//! A standard sRGB (IEC 61966-2.1) ICC v2 display profile, generated in code so
//! exports can be tagged without shipping a third-party profile file.
//!
//! Matrix/TRC form: D50-adapted sRGB colorants (the values used by the common
//! sRGB profiles) and the sRGB tone curve as a 1024-entry table.

const COLORANTS: [[f64; 3]; 3] = [
    [0.436066, 0.222488, 0.013916], // red
    [0.385147, 0.716873, 0.097076], // green
    [0.143066, 0.060608, 0.714096], // blue
];
const D50: [f64; 3] = [0.9642, 1.0, 0.8249];
const CURVE_POINTS: usize = 1024;
pub const DESCRIPTION: &str = "sRGB IEC61966-2.1";

fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(v: [f64; 3]) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for c in v {
        t.extend_from_slice(&s15f16(c));
    }
    t
}

/// sRGB decoding (encoded -> linear).
fn srgb_to_linear(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn curve_tag() -> Vec<u8> {
    let mut t = b"curv\0\0\0\0".to_vec();
    t.extend_from_slice(&(CURVE_POINTS as u32).to_be_bytes());
    for i in 0..CURVE_POINTS {
        let y = srgb_to_linear(i as f64 / (CURVE_POINTS - 1) as f64);
        t.extend_from_slice(&((y * 65535.0).round() as u16).to_be_bytes());
    }
    t
}

/// v2 textDescriptionType: ASCII part plus empty Unicode and ScriptCode parts.
fn desc_tag(text: &str) -> Vec<u8> {
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend_from_slice(&(text.len() as u32 + 1).to_be_bytes());
    t.extend_from_slice(text.as_bytes());
    t.push(0);
    t.extend_from_slice(&[0; 4]); // Unicode language code
    t.extend_from_slice(&[0; 4]); // Unicode count
    t.extend_from_slice(&[0; 2]); // ScriptCode code
    t.push(0); // ScriptCode count
    t.extend_from_slice(&[0; 67]); // ScriptCode string
    t
}

fn text_tag(text: &str) -> Vec<u8> {
    let mut t = b"text\0\0\0\0".to_vec();
    t.extend_from_slice(text.as_bytes());
    t.push(0);
    t
}

/// The profile bytes.
pub fn srgb_profile() -> Vec<u8> {
    let curve = curve_tag();
    // (signature, data); the three TRCs share one curve.
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc_tag(DESCRIPTION)),
        (b"cprt", text_tag("No copyright, use freely")),
        (b"wtpt", xyz_tag(D50)),
        (b"rXYZ", xyz_tag(COLORANTS[0])),
        (b"gXYZ", xyz_tag(COLORANTS[1])),
        (b"bXYZ", xyz_tag(COLORANTS[2])),
        (b"rTRC", curve.clone()),
        (b"gTRC", Vec::new()),
        (b"bTRC", Vec::new()),
    ];
    let table_len = 4 + tags.len() * 12;
    let mut data = Vec::new();
    let mut entries = Vec::new();
    let mut curve_at = (0u32, 0u32);
    for (sig, bytes) in &tags {
        let (offset, size) = if bytes.is_empty() {
            curve_at // shared TRC
        } else {
            let offset = (128 + table_len + data.len()) as u32;
            data.extend_from_slice(bytes);
            while data.len() % 4 != 0 {
                data.push(0);
            }
            (offset, bytes.len() as u32)
        };
        if sig == &b"rTRC" {
            curve_at = (offset, size);
        }
        entries.push((*sig, offset, size));
    }
    let total = 128 + table_len + data.len();

    let mut p = Vec::with_capacity(total);
    p.extend_from_slice(&(total as u32).to_be_bytes());
    p.extend_from_slice(&[0; 4]); // preferred CMM
    p.extend_from_slice(&[0x02, 0x10, 0x00, 0x00]); // version 2.1
    p.extend_from_slice(b"mntr");
    p.extend_from_slice(b"RGB ");
    p.extend_from_slice(b"XYZ ");
    for v in [2026u16, 1, 1, 0, 0, 0] {
        p.extend_from_slice(&v.to_be_bytes()); // creation date
    }
    p.extend_from_slice(b"acsp");
    p.extend_from_slice(&[0; 4]); // platform
    p.extend_from_slice(&[0; 4]); // flags
    p.extend_from_slice(&[0; 4]); // manufacturer
    p.extend_from_slice(&[0; 4]); // model
    p.extend_from_slice(&[0; 8]); // attributes
    p.extend_from_slice(&[0; 4]); // rendering intent: perceptual
    for c in D50 {
        p.extend_from_slice(&s15f16(c)); // PCS illuminant
    }
    p.extend_from_slice(&[0; 4]); // creator
    p.resize(128, 0); // profile ID + reserved
    p.extend_from_slice(&(tags.len() as u32).to_be_bytes());
    for (sig, offset, size) in entries {
        p.extend_from_slice(sig);
        p.extend_from_slice(&offset.to_be_bytes());
        p.extend_from_slice(&size.to_be_bytes());
    }
    p.extend_from_slice(&data);
    debug_assert_eq!(p.len(), total);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(p: &[u8], i: usize) -> u32 {
        u32::from_be_bytes(p[i..i + 4].try_into().unwrap())
    }

    #[test]
    fn profile_is_well_formed() {
        let p = srgb_profile();
        assert_eq!(u32_at(&p, 0) as usize, p.len());
        assert_eq!(&p[36..40], b"acsp");
        let n = u32_at(&p, 128) as usize;
        assert_eq!(n, 9);
        for k in 0..n {
            let e = 132 + k * 12;
            let (off, size) = (u32_at(&p, e + 4) as usize, u32_at(&p, e + 8) as usize);
            assert!(off % 4 == 0 && off + size <= p.len(), "tag {k} in bounds and aligned");
        }
        // Colorants add up to the D50 white.
        for c in 0..3 {
            let sum: f64 = COLORANTS.iter().map(|r| r[c]).sum();
            assert!((sum - D50[c]).abs() < 1e-3);
        }
    }
}
