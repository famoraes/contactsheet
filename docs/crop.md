# Crop, straighten and rotate

## Crop mode

Press **C** or click **Crop** in the Crop & Rotate section.

- Drag inside the box to move it; drag the eight handles to resize. A
  rule-of-thirds grid helps with composition.
- **Straighten** with the slider under the image (±15°, double-click resets).
  The image rotates under the box, and the box shrinks so no blank corners
  are included. The bar shows the output size in pixels.
- **Aspect ratio**: Free, Original, 1:1 (6×6), 7:6 (6×7), 4:3 (6×4.5),
  5:4 (4×5), 3:2 (35mm), 16:9. The button next to it switches between
  landscape and portrait.
- **Enter** or **Done** applies the crop, **Esc** or **Cancel** discards the
  changes. Moving to another frame applies the crop first.
- **Reset** removes the crop and straightening.

Applying a changed crop re-runs the [automatic analysis](converting.md#the-automatic-conversion)
on the cropped area, which keeps the film border out of the colour
correction, and clears neutral picks. Frames with a
[shared base](roll-workflow.md#how-a-shared-base-behaves) keep it.

Crops are stored relative to the frame, so they work at any resolution: the
preview, thumbnails, neutral picking and full-resolution export all use the
same crop.

## AutoCrop

**Auto** (⇧C) finds the picture area and crops away the film holder, film
rebate and sprocket holes.

- In crop mode it sets the box, which you can then adjust.
- Otherwise it crops **every selected frame** and re-analyses them.
- It also detects a slightly tilted frame (up to 8°) and straightens it.
- If a frame has no border, it is left alone. With a single frame you get a
  "no border found" message.

It looks along each side for the innermost sharp change in density that has a
**uniform** band outside it (film base, holder or glass are uniform; the
picture is not). Thin slivers of rebate at the very edge, as left by a scan
area set just inside the frame, are trimmed too.

Limitations: a picture area that is completely uniform along a whole edge
(for example a cloudless sky filling the top edge) can look like border. Check
the result, and adjust it in crop mode if needed.

## Rotate and flip

| Action | Key |
|---|---|
| Rotate left / right 90° | `[` / `]` |
| Flip horizontal | H |
| Flip vertical | V |

The crop follows rotation and flipping, so it keeps framing the same part of
the picture. The TIFF / EXIF orientation stored in the file is applied when it
loads.
