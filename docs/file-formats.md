# File formats

## TIFF scans

- 8-bit, 16-bit or 32-bit float; RGB, RGBA, grayscale.
- The scan should be **linear**: the scanner's raw light values, not an
  inverted or gamma-corrected image. In SilverFast, scan in **HDR raw 48 bit**;
  in VueScan, save a 48-bit **raw** file. Gamma-corrected scans still convert,
  but the tones and colours are less accurate.
- The TIFF orientation tag is honoured.
- Scan with a little border around the frame or without, as you prefer:
  [AutoCrop](crop.md#autocrop) removes it, and the automatic analysis ignores
  the outer 8% until you crop.

## Camera RAW and DNG

For camera scanning: DNG, Nikon NEF / NRW, Canon CR2 / CR3 / CRW, Sony ARW /
SRF / SR2, Fujifilm RAF, Olympus ORF, Panasonic RW2, Leica RWL, Pentax PEF,
Samsung SRW, Hasselblad 3FR / FFF, Phase One IIQ, and others.

RAW files are taken only to **linear camera RGB**: black and white levels,
demosaicing and the sensor's active area. White balance, the camera colour
matrix and gamma are skipped on purpose: for a negative, the
[automatic conversion](converting.md#the-automatic-conversion) measures each
colour channel itself, which makes them unnecessary. Orientation comes from
the camera's EXIF data.

Tips for camera scanning: shoot RAW, light the film evenly, and expose so the
film base is bright but not clipped.

Not supported: four-colour sensors (old CYGM cameras). LinearRaw DNGs, as some
scanner software writes, are expected to work but haven't been tested yet.

RAW decoding uses the LGPL-2.1 library rawler; see
[THIRD_PARTY_LICENSES.md](../THIRD_PARTY_LICENSES.md).

## Memory

The preview works from a 2000-pixel copy of each scan. The full resolution is
loaded only for 100% zoom (the two most recent frames are kept) and for export.
