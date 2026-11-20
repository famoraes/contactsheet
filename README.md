# ContactSheet

Film negative → positive converter for raw scans (TIFF for now). The UI is modeled on Filmomat SmartConvert.

**Stack:** Tauri 2 · Rust image core (`crates/negative`) · React + TypeScript + Vite · Zustand

## Run

```sh
nvm use                 # Node 22 (.nvmrc)
npm install
npm run tauri dev       # launches the desktop app
cargo test -p negative  # core pipeline tests
```

Need a test file? `cargo run -p negative --example make_negative -- ~/Desktop/synthetic.tif` writes a synthetic orange-masked 16-bit negative.

## Layout

| Path | What |
|---|---|
| `crates/negative/` | Pure Rust core with no Tauri dependency: TIFF decode, analysis, inversion pipeline, render, export |
| `src-tauri/` | Tauri shell. `commands.rs` holds the image store and IPC commands |
| `src/` | React UI: `components/`, `state/store.ts`, `lib/` (IPC wrappers, types, shortcuts) |

## How the inversion works

1. Each channel is converted to density, `D = −log10(T)`.
2. **Analysis** finds per-channel density black and white points (`lo`, `hi`) inside the analysis region. The default region is an 8% inset that skips the film border. **Recalc** reruns this step.
3. Normalizing `E = (D − lo)/(hi − lo)` removes the orange mask and the per-layer gamma differences in one step.
4. Density, C/M/Y and contrast act on `E`, like exposure and filtration on an enlarger.
5. `E` maps to scene light over a 2.2 log10 range, then goes through sRGB encoding and a gentle paper S-curve.

The math is log-domain and normalized per channel, so film base, light source brightness, bit depth and any power-law input gamma all cancel out.

Preview: Rust renders a 2000 px proxy and sends RGBA bytes to the canvas. Only the newest slider value is rendered, with one render in flight at a time. Export decodes the full-resolution file again and runs the same code path.

## Saving and metadata

- **Roll files:** edits (sliders, analysis, neutral picks, crop, orientation) and metadata are autosaved to `roll.contactsheet` (JSON) in each scan folder, keyed by file name. Reopening a scan restores them. A folder only gets a file once something in it is edited.
- **Info panel** (key `I`): film stock, format, process, ISO, roll, date shot, camera, lens, place, city, country, description, archive location, keywords, creator and copyright. Edits apply to every selected frame. Archive level names (default Binder / Page / Strip) are an app preference.
- **Filter bar** above the filmstrip: free-text search plus film / year / roll / archive / camera filters. Navigation, select-all and "Export all" follow the filter.
- **Export** embeds an XMP packet in TIFF (tag 700) and JPEG (APP1) files:

  | Field | Written as | Shown in Lightroom as |
  |---|---|---|
  | Film stock, format, process | `lr:hierarchicalSubject` keywords `Film\|Stock\|…` | Keywords |
  | Archive location | keyword `Archive\|Binder 3\|Page 12`, plus `photoshop:Instructions` | Keywords / Instructions |
  | Roll | `photoshop:TransmissionReference` | Job |
  | Date shot | `exif:DateTimeOriginal`, `photoshop:DateCreated` | Capture date |
  | Camera, lens, ISO | `tiff:Model`, `exifEX:LensModel`/`aux:Lens`, `exif:ISOSpeedRatings` | Camera / Lens / ISO |
  | Description, place, city, country | `dc:description`, `Iptc4xmpCore:Location`, `photoshop:City/Country` | Caption / Location |
  | Creator, copyright | `dc:creator`, `dc:rights` | Creator / Copyright |
  | Source scan, conversion settings | `xmpMM:DerivedFrom`, `cs:Settings` | - |
