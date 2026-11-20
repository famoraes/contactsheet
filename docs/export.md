# Export

Press **⌘E** or click **Export**.

| Option | Choices |
|---|---|
| **Frames** | The selection, or all frames ("Filtered" when a [filter](metadata.md#filtering) is active) |
| **Format** | **TIFF**: 16-bit sRGB, uncompressed. **JPEG**: 8-bit, quality 50–100 (default 92). Either or both. |
| **Metadata** | Embed film, date, location, archive and keywords as XMP (on by default), see [what gets exported](metadata.md#what-gets-exported) |
| **Size** | Full resolution, or downscale so the long edge is at most N pixels (default 3000) |
| **Folder** | Where the files go |

- Files are named after the scan: `scan.tif` becomes `scan_positive.tif` /
  `scan_positive.jpg`. **Existing files with the same name are overwritten.**
- Export reads the full-resolution scan again and runs the same conversion as
  the preview, then applies crop, straightening and rotation. Downscaling uses
  a Lanczos filter.
- Every file has an embedded **sRGB** colour profile, so colour-managed apps
  display it correctly.
- TIFF files also carry basic TIFF tags (software, and description, camera,
  creator and copyright when they are plain ASCII).
- Progress shows in the toolbar; frames that fail are listed at the end.
