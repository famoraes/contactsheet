# Development

## Stack

Tauri 2 desktop app: a Rust image core and a React + TypeScript UI.

| Path | What |
|---|---|
| `crates/negative/` | Pure Rust core with no Tauri dependency: decoding (TIFF, RAW), analysis, the inversion pipeline, geometry, AutoCrop, rendering, export, metadata, ICC profile |
| `src-tauri/` | Tauri shell. `commands.rs` holds the in-memory image store and IPC commands; `hotfolder.rs` the folder watcher |
| `src/` | React UI: `components/`, `state/store.ts` (Zustand), `lib/` (IPC wrappers, types, crop geometry, roll files, metadata, shortcuts) |
| `docs/` | This documentation |

```sh
nvm use && npm install
npm run tauri dev          # run the app
cargo test --workspace     # Rust tests (core + hot folder)
npm run typecheck          # TypeScript
cargo run -p negative --example make_negative -- ~/Desktop/synthetic.tif   # a synthetic test negative
```

`[profile.dev]` builds dependencies and the core crate with `opt-level = 3`,
since pixel loops are 10–50× slower unoptimized.

## The inversion

1. Each channel is converted to density, `D = −log10(T)`.
2. **Analysis** finds per-channel black and white points (`lo`, `hi`) at the
   0.05% / 99.95% percentiles inside the analysis region (8% inset, or the crop
   minus 1%). Roll analysis pools the histograms of several frames.
3. Normalizing `E = (D − lo) / (hi − lo)` removes the orange mask and the
   per-layer gamma differences in one step.
4. Density, C/M/Y and contrast act on `E`, like exposure and filtration on an
   enlarger: offsets per channel and a slope `2^contrast` pivoting at 0.7.
5. `E` maps to scene light over the look's log10 print range,
   `L = (10^((E−1)·R) − 10^−R) / (1 − 10^−R)`, then sRGB encoding and a gentle
   paper S-curve.

All of this folds into one power law per channel (`Prepared`), so rendering is
a `powf` per channel per pixel. Because the maths is log-domain and normalized
per channel, film base colour, light source brightness, bit depth and any
power-law input gamma cancel out.

The neutral picker solves for a per-channel slope and offset in `E` (one point:
offset only) and folds it back into `lo` / `hi`, so rendering needs no extra
state.

## Preview and zoom

Each loaded image keeps a 2000-pixel f32 proxy and a 320-pixel thumbnail in
memory. The UI asks Rust to render the proxy and receives raw RGBA bytes
(`tauri::ipc::Response`). A latest-wins loop keeps at most one render in flight,
so slider drags never queue up. 100% zoom renders just the visible region from
the full-resolution scan, kept in a two-entry LRU. Export decodes the full scan
again and runs the same code path.

## Geometry

Display = mirror ∘ rotate(quarter turns). The crop is defined in display space,
normalized, with an `angle` that rotates the image about its centre before
the upright crop rectangle is taken. Untilted crops are pixel copies; tilted
ones are sampled bilinearly. `src/lib/crop.ts` mirrors the Rust `Crop` /
`Framed` maths for the UI.

## Roll file format

`roll.contactsheet` is JSON:

```json
{
  "app": "ContactSheet",
  "version": 1,
  "archiveLevels": ["Binder", "Page", "Strip"],
  "frames": {
    "scan_001.tif": {
      "adjustments": { "density": 0.1, "contrast": 0, "saturation": 0, "cyan": 0, "magenta": 0, "yellow": 0, "grayscale": false, "look": "normal" },
      "analysis": { "lo": [0.53, 0.88, 1.18], "hi": [1.12, 1.67, 2.14] },
      "autoAnalysis": { "lo": [0.53, 0.88, 1.18], "hi": [1.12, 1.67, 2.14] },
      "orientation": { "rotation": 0, "mirror": false },
      "crop": { "x": 0.02, "y": 0, "w": 0.97, "h": 1, "angle": 0 },
      "neutralPoints": [],
      "sharedBase": false,
      "meta": { "filmStock": "Kodak Portra 400", "dateShot": "2026-03", "archive": ["3", "12"], "...": "..." }
    }
  }
}
```

Missing fields fall back to defaults, so older files keep loading.

## Conventions

- Issues track the roadmap. Commits that complete one end with `Closes #N`.
- Update the matching page in `docs/` in the same change as a feature.
- Licence: Apache-2.0. New dependencies should be permissively licensed, or
  noted in `THIRD_PARTY_LICENSES.md`.
