use negative::{
    analyze,
    export::{export_file, write_tiff16, ExportSettings},
    synth::{demo_negative, log_ramp, negative_of},
    Adjustments, Analysis, Crop, ImageF32, Orientation, Prepared, Rect, RgbSource,
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
    write_tiff16(&input, 300, 200, &rgb, None).unwrap();

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
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), None, &settings, None).unwrap();
    assert_eq!(files.len(), 2);
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height, out.bit_depth), (300, 200, 16));

    let small = ExportSettings { max_size: Some(150), jpeg_quality: None, ..settings };
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), None, &small, None).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height), (150, 100));

    let portrait = Orientation { rotation: 1, mirror: false };
    let files = export_file(&input, &an, &Adjustments::default(), portrait, None, &small, None).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height), (100, 150), "rotated export swaps dimensions");

    // Crop the middle half, straightened 3°; the output is the crop at full resolution.
    let crop = Crop { x: 0.25, y: 0.25, w: 0.5, h: 0.5, angle: 3.0 };
    let full = ExportSettings { max_size: None, ..small };
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), Some(crop), &full, None).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    assert_eq!((out.width, out.height), (150, 100), "cropped export is the crop size");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_embeds_metadata() {
    use negative::metadata::{embed, Metadata, Provenance};
    use tiff::{decoder::Decoder, tags::Tag};

    let dir = std::env::temp_dir().join(format!("negative-meta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let neg = demo_negative(60, 40);
    let rgb: Vec<u16> = neg.data.iter().map(|v| (v * 65535.0).round() as u16).collect();
    let input = dir.join("scan.tif");
    write_tiff16(&input, 60, 40, &rgb, None).unwrap();

    let meta = Metadata {
        film_stock: "Ilford HP5+".into(),
        camera: "Nikon FM2".into(),
        city: "Zürich".into(),
        copyright: "© Me".into(),
        ..Default::default()
    };
    let e = embed(&meta, &Provenance { source: input.display().to_string(), settings_json: "{}".into() });
    let settings = ExportSettings { out_dir: dir.join("out"), tiff16: true, jpeg_quality: Some(90), max_size: None };
    let an = analyze(&negative::decode_tiff(&input).unwrap(), Rect::DEFAULT_INSET);
    let files = export_file(&input, &an, &Adjustments::default(), Orientation::default(), None, &settings, Some(&e)).unwrap();

    // TIFF: XMP in tag 700, plain tags in IFD0, image still decodes.
    let mut dec = Decoder::new(std::fs::File::open(&files[0]).unwrap()).unwrap();
    // tiff 0.9 reads long BYTE arrays back as u64 values.
    let xmp: Vec<u8> = dec.get_tag_u64_vec(Tag::Unknown(700)).unwrap().into_iter().map(|b| b as u8).collect();
    let xmp = String::from_utf8(xmp).unwrap();
    assert!(xmp.contains("Film|Stock|Ilford HP5+") && xmp.contains("Zürich"));
    assert_eq!(dec.get_tag_ascii_string(Tag::Model).unwrap(), "Nikon FM2");
    assert!(dec.get_tag_ascii_string(Tag::Copyright).is_err(), "non-ASCII copyright is XMP-only");
    assert_eq!(dec.get_tag_ascii_string(Tag::Software).unwrap(), "ContactSheet");
    let icc = dec.get_tag_u8_vec(Tag::Unknown(34675)).unwrap(); // UNDEFINED reads back as bytes
    assert_eq!(icc, negative::icc::srgb_profile(), "sRGB profile embedded");
    assert_eq!(negative::decode_tiff(&files[0]).unwrap().width, 60);

    // JPEG: XMP APP1 present and the file still decodes.
    let jpg = std::fs::read(&files[1]).unwrap();
    let hay = String::from_utf8_lossy(&jpg);
    assert!(hay.contains("http://ns.adobe.com/xap/1.0/") && hay.contains("Ilford HP5+"));
    assert!(hay.contains("ICC_PROFILE"), "JPEG carries the sRGB profile");
    let decoded = image::load_from_memory(&jpg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (60, 40));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn roll_analysis_pools_frames() {
    use negative::analyze_many;
    let a = neutral_ramp(400);
    // Same scene, one stop "thinner" negative (less exposure -> lower density).
    let thin = ImageF32 { data: a.data.iter().map(|v| (v * 2.0).min(1.0)).collect(), ..a.clone() };
    let one = analyze(&a, Rect::FULL);
    assert_eq!(analyze_many(&[(&a, Rect::FULL)]), one, "a roll of one is the frame's own analysis");
    let roll = analyze_many(&[(&a, Rect::FULL), (&thin, Rect::FULL)]);
    let t = analyze(&thin, Rect::FULL);
    for c in 0..3 {
        // The shared range spans both frames.
        assert!(roll.lo[c] <= one.lo[c].min(t.lo[c]) + 0.01 && roll.hi[c] >= one.hi[c].max(t.hi[c]) - 0.01);
    }
}

#[test]
fn zoom_tile_matches_full_render() {
    use negative::{render::{render_preview, render_rgba}, Framed, ViewOptions, Window};
    let neg = demo_negative(240, 160);
    let an = analyze(&neg, Rect::DEFAULT_INSET);
    let prep = Prepared::new(&an, &Adjustments::default());
    let crop = Crop { x: 0.1, y: 0.05, w: 0.8, h: 0.85, angle: 2.0 };
    let frame = Framed::new(&neg, Orientation { rotation: 1, mirror: true }, Some(crop));
    let full = render_preview(&frame, &prep, ViewOptions::default());
    let fw = frame.width() as usize;
    let (x, y, w, h) = (17u32, 23u32, 40u32, 30u32);
    let tile = render_rgba(&Window::new(&frame, x, y, w, h), &prep, ViewOptions::default(), &[w, h, frame.width(), frame.height()]);
    assert_eq!(u32::from_le_bytes(tile[8..12].try_into().unwrap()), frame.width());
    for ty in 0..h as usize {
        for tx in 0..w as usize {
            let a = &tile[16 + (ty * w as usize + tx) * 4..][..4];
            let b = &full[8 + ((y as usize + ty) * fw + x as usize + tx) * 4..][..4];
            assert_eq!(a, b, "pixel {tx},{ty}");
        }
    }
    // Windows are clamped to the frame.
    let edge = Window::new(&frame, frame.width() - 5, 0, 100, 10);
    assert_eq!((edge.width(), edge.height()), (5, 10));
}

/// A flat grey negative with dark dust specks, and an IR channel that sees only the dust.
fn dusty_scan(w: u32, h: u32) -> (Vec<u16>, Vec<u16>, Vec<(u32, u32)>) {
    let dust: Vec<(u32, u32)> = (0..12).map(|i| (20 + (i * 37) % (w - 40), 20 + (i * 53) % (h - 40))).collect();
    let near = |x: u32, y: u32| dust.iter().any(|&(dx, dy)| x.abs_diff(dx) <= 2 && y.abs_diff(dy) <= 2);
    let (mut rgb, mut ir) = (Vec::new(), Vec::new());
    for y in 0..h {
        for x in 0..w {
            let k = if near(x, y) { 0.05 } else { 1.0 };
            rgb.extend_from_slice(&[(26000.0 * k) as u16, (13000.0 * k) as u16, (6500.0 * k) as u16]);
            ir.push((49000.0 * k) as u16);
        }
    }
    (rgb, ir, dust)
}

#[test]
fn decodes_ir_from_extra_page_or_fourth_channel() {
    use tiff::{encoder::{colortype, TiffEncoder}, tags::Tag};
    let dir = std::env::temp_dir().join(format!("negative-ir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = (80u32, 60u32);
    let (rgb, ir, _) = dusty_scan(w, h);

    // RGB page + full-size grey page flagged as a mask (how some scanner software stores IR).
    let paged = dir.join("paged.tif");
    {
        let mut enc = TiffEncoder::new(std::fs::File::create(&paged).unwrap()).unwrap();
        enc.write_image::<colortype::RGB16>(w, h, &rgb).unwrap();
        let mut img = enc.new_image::<colortype::Gray16>(w, h).unwrap();
        img.encoder().write_tag(Tag::NewSubfileType, 4u32).unwrap();
        img.write_data(&ir).unwrap();
    }
    let raw = negative::decode_tiff(&paged).unwrap();
    assert_eq!((raw.channels, raw.ir.as_ref().map(|p| p.data.len())), (3, Some((w * h) as usize)));
    assert_eq!(raw.ir.unwrap().data, ir);

    // RGBI: the 4th channel is IR when it varies...
    let rgbi: Vec<u16> = (0..(w * h) as usize).flat_map(|i| [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2], ir[i]]).collect();
    let four = dir.join("rgbi.tif");
    TiffEncoder::new(std::fs::File::create(&four).unwrap()).unwrap().write_image::<colortype::RGBA16>(w, h, &rgbi).unwrap();
    assert_eq!(negative::decode_tiff(&four).unwrap().ir.unwrap().data, ir);
    // ...but a constant alpha is just alpha.
    let rgba: Vec<u16> = (0..(w * h) as usize).flat_map(|i| [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2], 65535]).collect();
    let alpha = dir.join("rgba.tif");
    TiffEncoder::new(std::fs::File::create(&alpha).unwrap()).unwrap().write_image::<colortype::RGBA16>(w, h, &rgba).unwrap();
    assert!(negative::decode_tiff(&alpha).unwrap().ir.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_removes_dust_with_ir() {
    use negative::{export::export_source, ir::{clean_image, estimate_alignment, DustParams}, RawImage};
    let dir = std::env::temp_dir().join(format!("negative-dust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = (240u32, 200u32);
    let (rgb, ir, dust) = dusty_scan(w, h);
    let raw = RawImage {
        width: w, height: h, channels: 3, bit_depth: 16,
        samples: negative::decode::Samples::U16(rgb), orientation: Orientation::default(),
        ir: Some(negative::ir::IrPlane { width: w, height: h, data: ir }),
    };
    let plane = raw.ir.as_ref().unwrap();
    let cleaned = clean_image(&raw, plane, &estimate_alignment(&raw, plane), DustParams::default(), 1);
    let an = Analysis { lo: [0.2; 3], hi: [1.4; 3] };
    let settings = ExportSettings { out_dir: dir.join("out"), tiff16: true, jpeg_quality: None, max_size: None };
    let files = export_source(&cleaned, &dir.join("dusty.tif"), &an, &Adjustments::default(), Orientation::default(), None, &settings, None).unwrap();
    let out = negative::decode_tiff(&files[0]).unwrap();
    let clean_px = out.rgb(5 * w as usize + 5);
    for &(x, y) in &dust {
        let p = out.rgb((y * w + x) as usize);
        for c in 0..3 {
            assert!((p[c] - clean_px[c]).abs() < 0.02, "dust at {x},{y} still visible: {p:?} vs {clean_px:?}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}
