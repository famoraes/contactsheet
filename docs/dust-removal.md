# Dust and scratch removal

ContactSheet removes dust, hair and scratches automatically when the scan has an
**infrared (IR) channel**.

## How it works

Film dyes are almost transparent to infrared light, but dust, hair and
scratches are not. The IR channel therefore shows a nearly blank frame with only
the defects in it, an exact map of what to repair, with no guessing between
dust and real image detail.

1. **Alignment.** Scanners often capture IR in a separate pass, so it can be
   shifted by a pixel or two and very slightly stretched. ContactSheet measures
   this across the frame and lines the IR up with the image.
2. **Detection.** The IR is compared with its own smooth, local level, so uneven
   IR lighting doesn't matter. The faint "ghost" of the picture that sometimes
   shows in the IR, and Newton's rings, stay below the threshold.
3. **Repair.**
   - **Specks, hair and scratches** are filled from their surroundings, and the
     film grain from right beside them is copied in, so repairs don't show up as
     smooth patches.
   - **Soft, out-of-focus marks** (dust on the scanner glass) are corrected
     gently: only their smooth shading is removed, so the grain and detail
     underneath are kept.

Dust also skews the automatic colour (dust specks read as the densest parts of
the negative). With dust removal on, the automatic analysis, Recalc, the neutral
picker and Roll base all work on the cleaned image.

## Using it

For scans with an IR channel, a **Dust removal** section appears in the Adjust
panel. It is **on by default**.

| Control | Effect |
|---|---|
| **Off / On** | Turn dust removal off or on for this frame |
| **Strength** (0–100, default 50) | Higher also catches fainter marks. Double-click resets it. |
| **Show treated areas** | Tints everything being repaired, to check that only defects are touched |

The preview, filmstrip, 100% zoom and export all use the cleaned image. The
first 100% zoom and each export clean the full-resolution scan, which takes a
couple of seconds. The setting is saved per frame and copied with settings
(⌘C / ⌘V).

## Scanning with IR

- Save the scan **with its infrared channel**. Scanning software usually calls
  this a 64-bit RGBI or "HDRi" raw mode (48-bit RGB plus 16-bit IR).
- **Turn off the scanning software's own dust removal**, so the IR channel
  arrives untouched.
- ContactSheet reads IR stored either as a fourth channel or as a separate
  full-size grey page in the TIFF.

## Limitations

- **Black-and-white silver film and some slide films** block infrared light
  themselves, so IR dust removal doesn't work on them. Use it on colour
  negatives (and chromogenic black-and-white film).
- Marks that don't show in the IR at all, such as flaws in the emulsion itself,
  can't be found this way.
- At 100%, a very faint smooth trace can remain where a long hair was.
