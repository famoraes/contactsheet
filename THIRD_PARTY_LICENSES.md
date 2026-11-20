# Third-party licenses

ContactSheet is licensed under the Apache License 2.0 (see `LICENSE`). It is
built from open-source dependencies whose licenses are listed here when they
are not permissive (MIT / Apache-2.0 / BSD / ISC / Zlib and similar).

## rawler (LGPL-2.1)

Camera RAW and DNG decoding uses [rawler](https://github.com/dnglab/dnglab)
(part of DNGLab), licensed under the GNU Lesser General Public License,
version 2.1. The full license text is in `licenses/LGPL-2.1.txt`.

- Source: https://github.com/dnglab/dnglab (crate: https://crates.io/crates/rawler)
- ContactSheet uses rawler unmodified, as a library.
- ContactSheet's complete source is public, so you can rebuild the app against
  a modified version of rawler (change the `rawler` dependency in
  `crates/negative/Cargo.toml`), as the LGPL requires.
- RAW support can be left out entirely by building the core crate without its
  default `raw` feature.

## MPL-2.0 components

Some crates used internally by Tauri's HTML tooling (`cssparser`,
`cssparser-macros`, `selectors`, `dtoa-short`, `option-ext`) are licensed under
the Mozilla Public License 2.0, a file-level copyleft. They are used
unmodified; their source is available from crates.io.

## Everything else

All other Rust crates and npm packages are under permissive licenses (MIT,
Apache-2.0, BSD, ISC, Zlib, Unicode). Run `cargo metadata` or a license tool
such as `cargo-about` to produce a full list for a release.
