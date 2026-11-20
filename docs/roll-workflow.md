# Working with a roll

Frames from one roll were developed together and usually shot in similar light,
so they should look like they belong together. ContactSheet converts each frame
on its own by default; these tools tie a roll together.

## Selecting frames

In the filmstrip: click selects one frame, **Shift-click** selects a range,
**⌘-click** adds or removes a frame, **⌘A** selects all frames shown. Most
tools below act on the selection.

## Copy and paste settings

- **Copy** (⌘C) copies the current frame's adjustments: sliders, look and
  colour/grayscale.
- **Paste** (⌘V) applies them to the selected frames. Their own base
  correction, crop, rotation and metadata are kept.
- **Paste + base** (⇧⌘V) also copies the frame's **base correction**,
  including any neutral pick. Use it when one frame is right and the others
  should match it exactly.

## Roll base

**Roll base** (⇧R) analyses the selected frames together and gives them one
shared base correction. With only one frame selected it uses all frames
shown in the filmstrip. The densities of all the frames are pooled, so a dark
frame and a bright frame keep their relative exposure instead of each being
stretched to fill the range.

Per-frame adjustments (Density, Contrast, colour sliders) still apply on top, so
you can fine-tune individual frames without losing the shared base.

## How a shared base behaves

A frame whose base came from Roll base or Paste + base is marked: the **Roll
base** button is highlighted and the panel says "Shared base correction".

- **Changing the crop keeps the shared base.** Without the mark, applying a new
  crop would re-measure the frame on its own.
- **A neutral pick** on a shared-base frame corrects that frame only, on top of
  the shared base.
- **Recalc** (R) gives the frame its own base correction again.
- **Before / After** and **Reset** treat the shared base as the frame's
  automatic result.

The mark is saved in the [roll file](metadata.md#roll-files).
