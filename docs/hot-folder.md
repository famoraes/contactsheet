# Hot folder

A hot folder converts scans while you scan: point your scanner software's
output at a folder, and every new scan appears in ContactSheet as soon as it
has been saved.

## Using it

1. Click **Hot folder** in the toolbar and choose the folder.
2. Choose what happens to new scans:
   - **Use the settings of the frame on screen**: new scans get its
     adjustments and look, and its base correction when that is
     [shared](roll-workflow.md). Set up the first frame of a roll, and the rest
     follow.
   - **AutoCrop**: crop away the holder and rebate automatically.
3. Scan. Each new frame is added and shown.

While watching, the toolbar button shows the folder name. Open the dialog again
to change folders or **Stop watching**. Both options are remembered.

## What it picks up

- TIFF, DNG and camera RAW files saved **after** you start watching. Files
  already in the folder are left alone.
- A file is added only once its size has stopped changing for a moment, so
  half-written scans are never opened.
- Hidden files and ContactSheet's own exports (`*_positive.tif`) are ignored,
  so exporting into the hot folder is safe.

Tip: after the roll is scanned, select all frames and use **Roll base** (⇧R) for
a final, roll-wide colour balance.
