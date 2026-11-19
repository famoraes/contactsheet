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
