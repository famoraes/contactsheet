# ContactSheet

Film negative → positive converter for linear film scans (TIFF) and camera
scans (RAW / DNG), built roll by roll.

**Stack:** Tauri 2 · Rust image core (`crates/negative`) · React + TypeScript + Vite · Zustand

## Features

- **Automatic conversion** that removes the orange mask and per-layer differences, with Density, Contrast, Saturation and C/M/Y filtration, Normal / Flat looks, and a one- or two-point neutral picker for colour and crossover casts
- **Roll workflow**: shared base correction across a roll, copy/paste settings with or without the base
- **Crop and straighten**, **AutoCrop** (frame edge and tilt detection), rotate and flip
- **Dust and scratch removal** from the scan's infrared channel, with grain-preserving repair
- **100% zoom** with panning, before/after, clipping view, histogram, image-only mode
- **Metadata**: film stock, date, location, archive location, keywords. Searchable and filterable, and embedded on export as standard XMP / IPTC metadata that photo management apps read
- **Autosave** to a `roll.contactsheet` file next to the scans
- **Hot folder** that converts scans as the scanner saves them
- **Export** to 16-bit TIFF and/or JPEG with an embedded sRGB profile

## Quick start

```sh
nvm use                 # Node 22 (.nvmrc)
npm install
npm run tauri dev       # launches the desktop app
cargo test --workspace  # Rust tests
```

## Documentation

See **[docs/](docs/README.md)**: the user guide (getting started, converting,
roll workflow, crop, viewing, metadata, hot folder, export, file formats,
shortcuts) and the [development notes](docs/development.md).

## License

ContactSheet is licensed under the [Apache License 2.0](LICENSE). RAW decoding
uses rawler (LGPL-2.1); see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
