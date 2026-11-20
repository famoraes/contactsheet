# Converting negatives

## The automatic conversion

Every frame is converted as it loads, with no settings needed:

1. Each colour channel is turned into **density** (how much light the film
   blocks), which is how film itself responds to exposure.
2. ContactSheet measures the darkest and lightest densities in each channel
   inside the **analysis area**. Without a crop that is the frame minus an 8%
   border, which keeps the film rebate and holder out. With a crop it is the
   cropped area minus 1%.
3. Each channel is stretched between those two points. This removes the
   orange mask of colour negative film and the different contrast of the
   three dye layers in one step, which is the job of a "film base" or "base
   correction" step in other tools.
4. The result is printed onto a virtual paper (the [look](#looks)) and shown
   in sRGB.

The extremes are measured at the 0.05% and 99.95% points rather than the
absolute minimum and maximum, so a few dust specks or sprocket holes don't
throw it off.

### Recalc

**Recalc** (R) re-measures the current frame from its current crop. Use it
after changing a crop by hand if the colour looks off, or to give a frame back
its own base correction after [sharing one](roll-workflow.md). Applying a
changed crop recalculates automatically, unless the frame uses a shared base.

## Adjustments

All sliders run from −100 to +100, with 0 meaning the automatic result.
Keyboard nudges move a slider by 1, or by 5 with Shift. Double-click a slider
to reset it.

| Slider | Keys | Effect |
|---|---|---|
| **Density** | Q / A | Print exposure: + darker, − lighter (about ±2 stops at the ends) |
| **Contrast** | W / S | Slope from 0.5× to 2×, pivoting around mid-grey |
| **Saturation** | E / D | −100 is monochrome, +100 doubles saturation |
| **Cyan** | 1 / 2 | Filtration like an enlarger's colour head: + adds cyan (less red) |
| **Magenta** | 3 / 4 | + adds magenta (less green) |
| **Yellow** | 5 / 6 | + adds yellow (less blue) |

**Color / Grayscale** (G) switches to true monochrome output; colour sliders
are disabled in grayscale.

**Reset** (Settings section) returns the frame to the automatic conversion:
sliders, look, grayscale and neutral picks. Crop and rotation stay.

## Looks

The look is the "paper" the negative is printed on. Pick it in the Tone header
or press **L**.

- **Normal**: punchy, but with open shadows.
- **Flat**: lower contrast and more shadow detail, a good base when you plan
  to finish the image in Lightroom or similar.

The look is saved per frame and copied with settings.

## Neutral picker

Click **Neutral** (N), then click something in the image that should be
neutral grey: a road, a wall, a grey card.

- **One point** fixes a cast that is the same everywhere.
- **Shift-click a second point** in a clearly different tone (for example a
  shadow and a highlight) to also fix **crossover** casts, where shadows lean
  one way and highlights another. This is common with scanner "raw" output.
  The two points must differ in brightness; if they are too close, they are
  averaged into one.

Each pick starts from the automatic conversion, so picks never pile up, and it
resets Cyan / Magenta / Yellow to 0, since the pick defines neutral. Numbered
markers show the points while the picker is on. Press Esc to leave the picker.
Picks are cleared when the crop or rotation changes, because they refer to
positions in the old frame.

## Clipping and before/after

- **Clipping** (X) shows pure black areas in blue and pure white areas in red.
- **Before / After** (B) shows the automatic conversion without your
  adjustments and neutral picks, in place. **⇧B** shows the raw negative.

See [Viewing](viewing.md) for zoom and the histogram, and
[Development](development.md#the-inversion) for the maths.
