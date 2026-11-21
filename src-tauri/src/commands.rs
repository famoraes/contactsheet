use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex, RwLock,
    },
};

use negative::{
    analyze, analyze_many, autocrop::auto_crop as detect_crop, decode_image, downsample,
    decode::Samples,
    export::{encode_jpeg, export_file, export_source, ExportSettings},
    ir::{attenuation, clean, clean_image, estimate_alignment, DustParams, IrAlignment, IrPlane},
    metadata::{embed, Metadata, Provenance},
    neutral::{neutralize as neutralize_analysis, sample_density},
    render::{render_preview as render_preview_buf, render_rgb8, render_rgba},
    Adjustments, Analysis, Crop, Framed, ImageF32, Orientation, Prepared, RawImage, Rect, RgbSource, ViewOptions,
    Window,
};
use serde::{Deserialize, Serialize};
use tauri::{ipc::Response, AppHandle, Emitter, State};

/// Long edge of the in-memory preview proxy.
const PROXY_LONG: u32 = 2000;
/// Long edge of filmstrip thumbnails.
const THUMB_LONG: u32 = 320;
/// Analysis margin inside a user crop (the crop already excludes the border).
const CROP_INSET: Rect = Rect { x: 0.01, y: 0.01, w: 0.98, h: 0.98 };

struct LoadedImage {
    path: PathBuf,
    proxy: Arc<ImageF32>,
    thumb: ImageF32,
    ir: Option<IrInfo>,
    /// Dust-cleaned proxy for the last strength used.
    cleaned: Mutex<Option<(u32, Arc<ImageF32>)>>,
}

/// What dust removal needs, computed once when a scan with IR is loaded.
struct IrInfo {
    /// IR-to-RGB alignment at full resolution.
    align: IrAlignment,
    /// Attenuation (1 = clean) at proxy resolution; independent of strength.
    proxy_atten: Vec<f32>,
    /// Proxy downsampling factor.
    factor: usize,
}

/// Cache key for a dust setting (None = off).
fn dust_key(dust: Option<DustParams>) -> Option<u32> {
    dust.map(|d| (d.strength.clamp(0.0, 1.0) * 1000.0).round() as u32)
}

impl LoadedImage {
    /// The proxy to render and analyse: dust-cleaned when requested and possible.
    fn source(&self, dust: Option<DustParams>) -> Arc<ImageF32> {
        let (Some(params), Some(ir)) = (dust, &self.ir) else {
            return self.proxy.clone();
        };
        let key = dust_key(Some(params)).unwrap_or_default();
        if let Ok(mut c) = self.cleaned.lock() {
            if let Some((k, img)) = c.as_ref() {
                if *k == key {
                    return img.clone();
                }
            }
            let img = Arc::new(clean(self.proxy.as_ref(), &ir.proxy_atten, params, 1.0 / ir.factor as f32));
            *c = Some((key, img.clone()));
            return img;
        }
        self.proxy.clone()
    }
}

/// Decoded full-resolution scans kept for 100% zoom (most recent last), keyed by
/// image id and dust setting.
const FULL_RES_CACHE: usize = 2;

type FullKey = (u32, Option<u32>);

#[derive(Default)]
struct Shared {
    images: RwLock<HashMap<u32, Arc<LoadedImage>>>,
    next_id: AtomicU32,
    full: Mutex<Vec<(FullKey, Arc<RawImage>)>>,
}

/// A cleaned full-resolution image as a 16-bit `RawImage` (half the memory of f32).
fn to_raw_u16(img: ImageF32, orientation: Orientation, bit_depth: u8) -> RawImage {
    let samples = img.data.iter().map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16).collect();
    RawImage { width: img.width, height: img.height, channels: 3, bit_depth, samples: Samples::U16(samples), orientation, ir: None }
}

/// Decode a scan and, when requested and possible, remove dust at full resolution.
fn decode_full(path: &Path, info: Option<&IrInfo>, dust: Option<DustParams>) -> Result<RawImage, String> {
    let raw = decode_image(path).map_err(|e| format!("{}: {e}", file_name(path)))?;
    match (dust, info, raw.ir.as_ref()) {
        (Some(params), Some(info), Some(ir)) => {
            let cleaned = clean_image(&raw, ir, &info.align, params, 1);
            Ok(to_raw_u16(cleaned, raw.orientation, raw.bit_depth))
        }
        _ => Ok(raw),
    }
}

/// Grey view of an attenuation map, so it can be framed like the image.
struct MaskSource<'a> {
    width: u32,
    height: u32,
    data: &'a [f32],
}

impl RgbSource for MaskSource<'_> {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn rgb(&self, i: usize) -> [f32; 3] {
        [self.data[i]; 3]
    }
}

/// Tint pixels that dust removal treats (preview buffer: 8-byte header + RGBA).
fn tint_defects(buf: &mut [u8], img: &LoadedImage, orientation: Orientation, crop: Option<Crop>, dust: DustParams) {
    let Some(ir) = &img.ir else { return };
    let mask = MaskSource { width: img.proxy.width, height: img.proxy.height, data: &ir.proxy_atten };
    let framed = Framed::new(&mask, orientation, crop);
    let t = dust.threshold();
    let n = (framed.width() * framed.height()) as usize;
    for i in 0..n {
        let d = 1.0 - framed.rgb(i)[0];
        if d > t {
            let k = (0.45 + d).min(0.9);
            let px = &mut buf[8 + i * 4..8 + i * 4 + 3];
            for (c, target) in px.iter_mut().zip([255.0f32, 70.0, 20.0]) {
                *c = (*c as f32 * (1.0 - k) + target * k) as u8;
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct AppState {
    shared: Arc<Shared>,
}

impl AppState {
    fn get(&self, id: u32) -> Result<Arc<LoadedImage>, String> {
        self.shared
            .images
            .read()
            .map_err(|_| "image store poisoned".to_string())?
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown image id {id}"))
    }

    /// The full-resolution scan (dust-cleaned when requested), decoded on first use
    /// and kept in a small LRU.
    fn full_res(&self, id: u32, dust: Option<DustParams>) -> Result<Arc<RawImage>, String> {
        let img = self.get(id)?;
        let dust = dust.filter(|_| img.ir.is_some());
        let key: FullKey = (id, dust_key(dust));
        let lock = || self.shared.full.lock().map_err(|_| "cache poisoned".to_string());
        {
            let mut cache = lock()?;
            if let Some(i) = cache.iter().position(|(k, _)| *k == key) {
                let hit = cache.remove(i);
                let raw = hit.1.clone();
                cache.push(hit);
                return Ok(raw);
            }
        }
        let raw = Arc::new(decode_full(&img.path, img.ir.as_ref(), dust)?);
        let mut cache = lock()?;
        cache.retain(|(k, _)| k.0 != id);
        cache.push((key, raw.clone()));
        while cache.len() > FULL_RES_CACHE {
            cache.remove(0);
        }
        Ok(raw)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    id: u32,
    name: String,
    path: String,
    width: u32,
    height: u32,
    bit_depth: u8,
    analysis: Analysis,
    /// Initial orientation from the file's TIFF Orientation tag.
    orientation: Orientation,
    /// The scan has an infrared channel (dust removal available).
    has_ir: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    id: u32,
    adjustments: Adjustments,
    analysis: Analysis,
    #[serde(default)]
    orientation: Orientation,
    #[serde(default)]
    crop: Option<Crop>,
    /// Descriptive metadata to embed; `None` writes none.
    #[serde(default)]
    metadata: Option<Metadata>,
    /// IR dust removal; `None` = off.
    #[serde(default)]
    dust: Option<DustParams>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    out_dir: String,
    tiff: bool,
    jpeg: bool,
    jpeg_quality: u8,
    max_size: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    done: usize,
    total: usize,
    name: String,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    written: Vec<String>,
    errors: Vec<String>,
}

/// Run CPU-heavy work off the async runtime's worker threads.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("worker failed: {e}"))?
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Decode a scan, build its preview proxy and thumbnail, and run auto analysis.
#[tauri::command]
pub async fn add_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImageMeta, String> {
    let st = state.inner().clone();
    blocking(move || {
        let path = PathBuf::from(path);
        let raw = decode_image(&path).map_err(|e| format!("{}: {e}", file_name(&path)))?;
        // Stored (unrotated) size; the UI derives display size from the orientation.
        let (width, height, bit_depth, orientation) =
            (raw.width, raw.height, raw.bit_depth, raw.orientation);
        let proxy = downsample(&raw, PROXY_LONG);
        // Dust removal: align the IR once at full resolution, keep a proxy-size map.
        let ir = raw.ir.as_ref().map(|plane: &IrPlane| {
            let align = estimate_alignment(&raw, plane);
            let factor = (width.max(height) as usize).div_ceil(PROXY_LONG as usize).max(1);
            let proxy_atten = attenuation(&plane.downsample(factor), &align.scaled(factor), proxy.width, proxy.height);
            IrInfo { align, proxy_atten, factor }
        });
        drop(raw);
        let loaded = LoadedImage { path: path.clone(), proxy: Arc::new(proxy), thumb: ImageF32 { width: 0, height: 0, data: vec![] }, ir, cleaned: Mutex::new(None) };
        // Dust removal is on by default for IR scans, so analyse (and thumbnail) the
        // cleaned image: dust would otherwise skew the white point.
        let source = loaded.source(loaded.ir.as_ref().map(|_| DustParams::default()));
        let thumb = downsample(source.as_ref(), THUMB_LONG);
        let analysis = analyze(source.as_ref(), Rect::DEFAULT_INSET);
        let has_ir = loaded.ir.is_some();
        let loaded = LoadedImage { thumb, ..loaded };
        let id = st.shared.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let meta = ImageMeta {
            id,
            name: file_name(&path),
            path: path.display().to_string(),
            width,
            height,
            bit_depth,
            analysis,
            orientation,
            has_ir,
        };
        st.shared
            .images
            .write()
            .map_err(|_| "image store poisoned".to_string())?
            .insert(id, Arc::new(loaded));
        Ok(meta)
    })
    .await
}

#[tauri::command]
pub async fn remove_images(state: State<'_, AppState>, ids: Vec<u32>) -> Result<(), String> {
    let mut images = state
        .shared
        .images
        .write()
        .map_err(|_| "image store poisoned".to_string())?;
    if let Ok(mut full) = state.shared.full.lock() {
        full.retain(|(k, _)| !ids.contains(&k.0));
    }
    for id in ids {
        images.remove(&id);
    }
    Ok(())
}

/// Render the preview proxy. Returns an 8-byte header (width, height LE u32) + RGBA8.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn render_preview(
    state: State<'_, AppState>,
    id: u32,
    adjustments: Adjustments,
    analysis: Analysis,
    orientation: Orientation,
    crop: Option<Crop>,
    view: ViewOptions,
    dust: Option<DustParams>,
) -> Result<Response, String> {
    let img = state.get(id)?;
    blocking(move || {
        let prep = Prepared::new(&analysis, &adjustments);
        let source = img.source(dust);
        let src = Framed::new(source.as_ref(), orientation, crop);
        let mut buf = render_preview_buf(&src, &prep, view);
        if view.show_defects {
            tint_defects(&mut buf, &img, orientation, crop, dust.unwrap_or_default());
        }
        Ok(Response::new(buf))
    })
    .await
}

/// Render part of the frame at full resolution (100% zoom). `x, y, w, h` are pixels
/// of the full-resolution frame after orientation and crop. Returns a 16-byte
/// header (tile width, tile height, full frame width, full frame height; LE u32)
/// followed by RGBA8.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn render_region(
    state: State<'_, AppState>,
    id: u32,
    adjustments: Adjustments,
    analysis: Analysis,
    orientation: Orientation,
    crop: Option<Crop>,
    view: ViewOptions,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    dust: Option<DustParams>,
) -> Result<Response, String> {
    let st = state.inner().clone();
    blocking(move || {
        let raw = st.full_res(id, dust)?;
        let prep = Prepared::new(&analysis, &adjustments);
        let frame = Framed::new(raw.as_ref(), orientation, crop);
        let (fw, fh) = (frame.width(), frame.height());
        let tile = Window::new(&frame, x, y, w, h);
        Ok(Response::new(render_rgba(&tile, &prep, view, &[tile.width(), tile.height(), fw, fh])))
    })
    .await
}

/// Render the filmstrip thumbnail as JPEG bytes.
#[tauri::command]
pub async fn render_thumbnail(
    state: State<'_, AppState>,
    id: u32,
    adjustments: Adjustments,
    analysis: Analysis,
    orientation: Orientation,
    crop: Option<Crop>,
) -> Result<Response, String> {
    let img = state.get(id)?;
    blocking(move || {
        let prep = Prepared::new(&analysis, &adjustments);
        let t = Framed::new(&img.thumb, orientation, crop);
        let jpeg = encode_jpeg(t.width(), t.height(), &render_rgb8(&t, &prep), 85)
            .map_err(|e| e.to_string())?;
        Ok(Response::new(jpeg))
    })
    .await
}

/// Recompute the density black/white points from the cropped frame, or from the
/// default border-skipping inset when there is no crop.
#[tauri::command]
pub async fn recalc(
    state: State<'_, AppState>,
    id: u32,
    orientation: Orientation,
    crop: Option<Crop>,
    dust: Option<DustParams>,
) -> Result<Analysis, String> {
    let img = state.get(id)?;
    blocking(move || {
        let region = if crop.is_some() { CROP_INSET } else { Rect::DEFAULT_INSET };
        let source = img.source(dust);
        Ok(analyze(&Framed::new(source.as_ref(), orientation, crop), region))
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollItem {
    id: u32,
    #[serde(default)]
    orientation: Orientation,
    #[serde(default)]
    crop: Option<Crop>,
    #[serde(default)]
    dust: Option<DustParams>,
}

/// One shared base correction for several frames (a roll): their density
/// histograms (each over its crop, or the border-skipping inset) are pooled.
#[tauri::command]
pub async fn roll_analysis(state: State<'_, AppState>, items: Vec<RollItem>) -> Result<Analysis, String> {
    let images = items
        .iter()
        .map(|it| state.get(it.id))
        .collect::<Result<Vec<_>, _>>()?;
    blocking(move || {
        let sources: Vec<Arc<ImageF32>> = images.iter().zip(&items).map(|(img, it)| img.source(it.dust)).collect();
        let views: Vec<Framed<'_, ImageF32>> = sources
            .iter()
            .zip(&items)
            .map(|(src, it)| Framed::new(src.as_ref(), it.orientation, it.crop))
            .collect();
        let frames: Vec<(&Framed<'_, ImageF32>, Rect)> = views
            .iter()
            .zip(&items)
            .map(|(v, it)| (v, if it.crop.is_some() { CROP_INSET } else { Rect::DEFAULT_INSET }))
            .collect();
        if frames.is_empty() {
            return Err("no frames".into());
        }
        Ok(analyze_many(&frames))
    })
    .await
}

/// Find the picture area (excluding holder, film rebate, sprocket holes) of the
/// frame as displayed under `orientation`. `None` means no border was found.
#[tauri::command]
pub async fn auto_crop(
    state: State<'_, AppState>,
    id: u32,
    orientation: Orientation,
) -> Result<Option<Crop>, String> {
    let img = state.get(id)?;
    blocking(move || Ok(detect_crop(&negative::Oriented::new(img.proxy.as_ref(), orientation)))).await
}

/// Make the picked points neutral. `points` are normalized display coordinates
/// (after `orientation`); one point fixes a uniform cast, two points in different
/// tones also fix shadow/highlight crossover. Returns the corrected analysis,
/// computed from `analysis` (pass the automatic one so picks don't compound).
#[tauri::command]
pub async fn neutralize(
    state: State<'_, AppState>,
    id: u32,
    points: Vec<[f32; 2]>,
    orientation: Orientation,
    crop: Option<Crop>,
    analysis: Analysis,
    dust: Option<DustParams>,
) -> Result<Analysis, String> {
    let img = state.get(id)?;
    blocking(move || {
        let source = img.source(dust);
        let p = source.as_ref();
        let radius = (p.width.max(p.height) / 300).max(2) as usize;
        let view = Framed::new(p, orientation, crop);
        let samples: Vec<[f32; 3]> = points
            .iter()
            .map(|&[u, v]| sample_density(&view, u, v, radius))
            .collect();
        Ok(neutralize_analysis(&analysis, &samples))
    })
    .await
}

/// Export full-resolution positives. Emits `export-progress` after each frame.
#[tauri::command]
pub async fn export_images(
    app: AppHandle,
    state: State<'_, AppState>,
    items: Vec<ExportItem>,
    options: ExportOptions,
) -> Result<ExportSummary, String> {
    if !options.tiff && !options.jpeg {
        return Err("choose at least one export format".into());
    }
    let jobs = items
        .into_iter()
        .map(|it| state.get(it.id).map(|img| (img.path.clone(), img, it)))
        .collect::<Result<Vec<_>, _>>()?;
    let settings = ExportSettings {
        out_dir: PathBuf::from(&options.out_dir),
        tiff16: options.tiff,
        jpeg_quality: options.jpeg.then_some(options.jpeg_quality),
        max_size: options.max_size.filter(|&m| m > 0),
    };
    blocking(move || {
        let total = jobs.len();
        let mut summary = ExportSummary {
            written: Vec::new(),
            errors: Vec::new(),
        };
        for (i, (path, img, item)) in jobs.into_iter().enumerate() {
            let name = file_name(&path);
            let embedded = item.metadata.as_ref().map(|m| {
                let settings_json = serde_json::json!({
                    "analysis": item.analysis,
                    "adjustments": item.adjustments,
                    "orientation": item.orientation,
                    "crop": item.crop,
                })
                .to_string();
                embed(m, &Provenance { source: path.display().to_string(), settings_json })
            });
            let result = match (item.dust, &img.ir) {
                // Dust removal: decode, clean at full resolution, then export.
                (Some(params), Some(_)) => decode_full(&path, img.ir.as_ref(), Some(params))
                    .map_err(negative::Error::msg)
                    .and_then(|raw| {
                        export_source(&raw, &path, &item.analysis, &item.adjustments, item.orientation, item.crop, &settings, embedded.as_ref())
                    }),
                _ => export_file(&path, &item.analysis, &item.adjustments, item.orientation, item.crop, &settings, embedded.as_ref()),
            };
            let error = match result {
                Ok(files) => {
                    summary
                        .written
                        .extend(files.iter().map(|p| p.display().to_string()));
                    None
                }
                Err(e) => {
                    let msg = format!("{name}: {e}");
                    summary.errors.push(msg.clone());
                    Some(msg)
                }
            };
            let _ = app.emit(
                "export-progress",
                ExportProgress {
                    done: i + 1,
                    total,
                    name,
                    error,
                },
            );
        }
        Ok(summary)
    })
    .await
}

/// Name of the per-folder file holding edits and metadata for the scans in it.
const ROLL_FILE: &str = "roll.contactsheet";

/// Read the roll file in `dir`, if there is one. The contents are JSON owned by the UI.
#[tauri::command]
pub async fn read_roll(dir: String) -> Result<Option<String>, String> {
    blocking(move || {
        let path = Path::new(&dir).join(ROLL_FILE);
        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    })
    .await
}

/// Write the roll file in `dir` atomically (temp file + rename), so a crash
/// mid-write never leaves a truncated file.
#[tauri::command]
pub async fn write_roll(dir: String, contents: String) -> Result<(), String> {
    blocking(move || {
        let path = Path::new(&dir).join(ROLL_FILE);
        let tmp = Path::new(&dir).join(format!(".{ROLL_FILE}.tmp"));
        std::fs::write(&tmp, contents)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| format!("Saving {}: {e}", path.display()))
    })
    .await
}
