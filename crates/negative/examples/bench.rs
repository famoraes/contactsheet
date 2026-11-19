//! Time the app's hot paths on a real file.
//!
//! cargo run -p negative --release --example bench -- scan.tif [out_dir]

use std::time::Instant;

use negative::{
    analyze, decode_tiff, downsample,
    export::{export_file, ExportSettings},
    render::render_preview,
    Adjustments, Prepared, Rect, ViewOptions,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let input = std::path::PathBuf::from(args.next().expect("usage: bench <scan.tif> [out_dir]"));
    let out_dir = args.next().map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);

    let t = Instant::now();
    let raw = decode_tiff(&input).expect("decode");
    println!("decode            {:>7.1} ms  ({}x{}, {}-bit)", ms(t), raw.width, raw.height, raw.bit_depth);

    let t = Instant::now();
    let proxy = downsample(&raw, 2000);
    println!("proxy             {:>7.1} ms  ({}x{})", ms(t), proxy.width, proxy.height);
    drop(raw);

    let t = Instant::now();
    let an = analyze(&proxy, Rect::DEFAULT_INSET);
    println!("analyze           {:>7.1} ms  lo={:?} hi={:?}", ms(t), an.lo, an.hi);

    let n = 20;
    let t = Instant::now();
    for i in 0..n {
        let adj = Adjustments { density: i as f32 / n as f32 - 0.5, ..Default::default() };
        std::hint::black_box(render_preview(&proxy, &Prepared::new(&an, &adj), ViewOptions::default()));
    }
    println!("preview render    {:>7.1} ms  (avg of {n})", ms(t) / n as f64);

    let settings = ExportSettings { out_dir, tiff16: true, jpeg_quality: Some(92), max_size: None };
    let t = Instant::now();
    let files = export_file(&input, &an, &Adjustments::default(), &settings).expect("export");
    println!("export tif+jpg    {:>7.1} ms  -> {files:?}", ms(t));
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
