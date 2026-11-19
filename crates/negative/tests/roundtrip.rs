use negative::{
    analyze,
    export::{export_file, write_tiff16, ExportSettings},
    synth::{demo_negative, log_ramp, negative_of},
    Adjustments, Analysis, ImageF32, Orientation, Prepared, Rect, RgbSource,
};

/// A neutral scene ramp (L_MIN..1, log spaced) rendered as an orange-masked negative.
fn neutral_ramp(width: u32) -> ImageF32 {
    let height = 4;
    let mut data = Vec::new();
    for _ in 0..height {
        for x in 0..width {
            let l = log_ramp(x as f32 / (width - 1) as f32);
            data.extend_from_slice(&negative_of([l; 3]));
        }
    }
    ImageF32 { width, height, data }
}

fn out_at(img: &ImageF32, prep: &Prepared, x: u32) -> [f32; 3] {
    prep.apply(img.rgb(x as usize)).0
}

fn ramp_setup() -> (ImageF32, Analysis) {
    let img = neutral_ramp(512);
    let an = analyze(&img, Rect::FULL);
    (img, an)
}

#[test]
fn neutral_ramp_inverts_to_neutral_monotonic_greys() {
    let (img, an) = ramp_setup();
    let prep = Prepared::new(&an, &Adjustments::default());
    let mut prev = -1.0f32;
    for x in 0..img.width {
        let [r, g, b] = out_at(&img, &prep, x);
        let spread = r.max(g).max(b) - r.min(g).min(b);
        assert!(spread < 0.02, "column {x}: not neutral ({r:.3},{g:.3},{b:.3})");
        assert!(g >= prev - 1e-4, "column {x}: tone order broken ({prev} -> {g})");
        prev = g;
    }
    assert!(out_at(&img, &prep, 0)[1] < 0.03, "black point should be near black");
    assert!(out_at(&img, &prep, img.width - 1)[1] > 0.97, "white point should be near white");
}

#[test]
fn mid_grey_lands_mid_scale() {
    let (_img, an) = ramp_setup();
    let prep = Prepared::new(&an, &Adjustments::default());
    let grey = prep.apply(negative_of([0.18; 3])).0[1];
    assert!((0.38..0.58).contains(&grey), "18% grey rendered at {grey}");
}

#[test]
fn adjustments_move_in_the_expected_direction() {
    let (_img, an) = ramp_setup();
    let neg = negative_of([0.18; 3]);
    let base = Prepared::new(&an, &Adjustments::default()).apply(neg).0;

    let darker = Prepared::new(&an, &Adjustments { density: 0.5, ..Default::default() }).apply(neg).0;
    assert!(darker[1] < base[1] - 0.05, "density+ should darken");

    let yellow = Prepared::new(&an, &Adjustments { yellow: 0.5, ..Default::default() }).apply(neg).0;
    assert!(yellow[0] > yellow[2] + 0.02, "yellow+ should cut blue: {yellow:?}");

    let cyan = Prepared::new(&an, &Adjustments { cyan: 0.5, ..Default::default() }).apply(neg).0;
    assert!(cyan[0] < cyan[1] - 0.02, "cyan+ should cut red: {cyan:?}");

    let light = negative_of([0.6; 3]);
    let shadow = negative_of([0.02; 3]);
    let flat = Prepared::new(&an, &Adjustments { contrast: -0.5, ..Default::default() });
    let punchy = Prepared::new(&an, &Adjustments { contrast: 0.5, ..Default::default() });
    let span = |p: &Prepared| p.apply(light).0[1] - p.apply(shadow).0[1];
    assert!(span(&punchy) > span(&flat) + 0.1, "contrast+ should widen tonal span");

    let mono = Prepared::new(&an, &Adjustments { grayscale: true, ..Default::default() })
        .apply(negative_of([0.4, 0.1, 0.05]))
        .0;
    assert!((mono[0] - mono[1]).abs() < 1e-6 && (mono[1] - mono[2]).abs() < 1e-6);
}

#[test]
fn colour_patch_keeps_its_hue() {
    let (_img, an) = ramp_setup();
    let prep = Prepared::new(&an, &Adjustments::default());
    let [r, g, b] = prep.apply(negative_of([0.40, 0.08, 0.06])).0;
    assert!(r > g + 0.2 && r > b + 0.2, "red patch came out ({r:.2},{g:.2},{b:.2})");
}

#[test]
fn default_inset_analysis_ignores_the_film_border() {
    let img = demo_negative(600, 400);
    let inset = analyze(&img, Rect::DEFAULT_INSET);
    let full = analyze(&img, Rect::FULL);
    // The rebate is pure film base (lowest density); including it drags `lo` down.
    for c in 0..3 {
        assert!(full.lo[c] <= inset.lo[c] + 1e-3);
    }
    assert!(inset.lo.iter().zip(inset.hi.iter()).all(|(l, h)| h > l));
}

#[test]
fn tiff_roundtrip_and_export() {
    let dir = std::env::temp_dir().join(format!("negative-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let neg = demo_negative(300, 200);
    let rgb: Vec<u16> = neg.data.iter().map(|v| (v * 65535.0).round() as u16).collect();
    let input = dir.join("scan.tif");
    write_tiff16(&input, 300, 200, &rgb).unwrap();

    let raw = negative::decode_tiff(&input).unwrap();
    assert_eq!((raw.width, raw.height, raw.channels, raw.bit_depth), (300, 200, 3, 16));
    let px = raw.rgb(150 * 300 + 150);
    let orig = neg.rgb(150 * 300 + 150);
    for c in 0..3 {
        assert!((px[c] - orig[c]).abs() < 1e-4);
    }

    let an = analyze(&raw, Rect::DEFAULT_INSET);
    let settings = ExportSettings {
        out_dir: dir.join("out"),
        tiff16: true,
        jpeg_quality: Some(90),
        max_size: None,
    };
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), &settings).unwrap();
    assert_eq!(files.len(), 2);
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height, out.bit_depth), (300, 200, 16));

    let small = ExportSettings { max_size: Some(150), jpeg_quality: None, ..settings };
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), &small).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height), (150, 100));

    let portrait = Orientation { rotation: 1, mirror: false };
    let files = export_file(&input, &an, &Adjustments::default(), portrait, &small).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height), (100, 150), "rotated export swaps dimensions");

    std::fs::remove_dir_all(&dir).ok();
}
