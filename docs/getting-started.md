# Getting started

## Install

There are no packaged releases yet; run ContactSheet from source.

Requirements: [Node.js 22](https://nodejs.org) (the repo's `.nvmrc` pins it for
[nvm](https://github.com/nvm-sh/nvm)) and a stable [Rust](https://rustup.rs)
toolchain. Development happens on macOS; other platforms Tauri supports should
work but are untested.

```sh
nvm use
npm install
npm run tauri dev
```

## The window

| Area | What it holds |
|---|---|
| **Toolbar** (top) | Add Images, Hot folder, previous/next and a frame counter (click it to jump to a frame number), Clipping, Before / After, Export |
| **Viewer** (centre) | The current frame on a neutral 50% grey background |
| **Panel** (right) | Two tabs. **Adjust** has the histogram, colour/grayscale, tone and colour sliders, neutral picker, Recalc, Roll base, crop and rotate tools, copy/paste. **Info** has the frame's metadata. |
| **Filmstrip** (bottom) | Thumbnails of the open frames, with a search and filter bar above them |

Press **Tab** to hide everything except the image; press it again (or Esc) to
bring the panels back.

## A first roll

1. **Add scans.** Use **+ Add Images** (⌘O), drag files onto the window, or
   point a [hot folder](hot-folder.md) at your scanner's output. Each frame is
   converted automatically as it loads.
2. **Crop.** Select all frames (⌘A) and press **⇧C** to
   [AutoCrop](crop.md#autocrop) them, or press **C** to crop one by hand.
   Cropping away the film border also improves the automatic colour.
3. **Make the roll consistent.** With the frames selected, click
   **Roll base** (⇧R) so they share one base correction, see
   [Working with a roll](roll-workflow.md).
4. **Adjust.** Step through frames with ← / →. Use Density (Q/A) and
   Contrast (W/S), and the [neutral picker](converting.md#neutral-picker) (N)
   for colour casts. Press **B** to compare with the automatic result.
5. **Describe.** Open the **Info** tab (I) and fill in film stock, date,
   archive location and so on. Edits apply to every selected frame.
6. **Export.** Press ⌘E, choose TIFF and/or JPEG and a folder.

Edits and metadata save automatically to a `roll.contactsheet` file next to
the scans, so reopening the same files later brings everything back. See
[roll files](metadata.md#roll-files).
