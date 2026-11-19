//! Write a synthetic 16-bit negative TIFF for trying the app without real scans.
//!
//! cargo run -p negative --example make_negative -- /path/to/out.tif [width] [height]

use negative::{export::write_tiff16, synth::demo_negative};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "synthetic_negative.tif".into());
    let width: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let height: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2000);
    let img = demo_negative(width, height);
    let rgb: Vec<u16> = img
        .data
        .iter()
        .map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16)
        .collect();
    write_tiff16(std::path::Path::new(&out), width, height, &rgb).expect("write TIFF");
    println!("wrote {out} ({width}x{height}, 16-bit RGB)");
}
