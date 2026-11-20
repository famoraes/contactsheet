# Metadata, saving and filtering

## The Info panel

Open the **Info** tab (I) to describe frames. Edits apply to **every selected
frame**. When the selected frames have different values, the field shows
"Mixed", and typing sets that value on all of them.

| Section | Fields |
|---|---|
| **Film** | Film stock, format, process, ISO / EI, roll |
| **Shot** | Date, camera, lens, place, city, country, description |
| **Archive** | Where the physical negatives are kept, e.g. Binder / Page / Strip |
| **Keywords & rights** | Keywords, creator, copyright |

- **Suggestions**: film stock, format and process offer common values, and
  fields suggest values already used on other open frames.
- **Date** accepts `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, since film dates are
  often approximate. `2026/3/5` is tidied to `2026-03-05`; anything else is
  marked invalid and not exported.
- **Keywords** are comma-separated; `>` or `|` makes a hierarchy:
  `family, Travel > Brazil`.
- **Archive levels**: click **Rename levels** to change their names or use
  one to five levels. Level names are shared by all rolls; values are per
  frame.
- **Creator and copyright** are remembered and pre-filled on new scans.

## Filtering

The bar above the filmstrip narrows down the frames shown:

- **Search** matches any metadata field and the file name (all words must
  match). Esc clears it.
- **Dropdowns** for film stock, year, roll, the first archive level and camera.
  Each lists the values in use, plus "not set".

Navigation, ⌘A and Export's "Filtered" scope follow the filter. The current
frame stays visible even if an edit makes it stop matching, until you move on.

## Roll files

Edits and metadata save automatically to **`roll.contactsheet`** in each scan
folder, about half a second after a change and when the window closes.
Reopening the scans restores everything: sliders, look, base correction,
neutral picks, crop, rotation and metadata.

- One file per folder holds all frames in it, keyed by **file name**. Moving
  or copying the folder keeps the edits; **renaming a scan** loses them.
- A folder only gets a roll file once something in it is edited.
- If a roll file can't be read, ContactSheet reports it and never overwrites
  it.
- The file is JSON, so it is readable, and easy to back up together with the
  scans.

## What gets exported

When **Embed metadata** is on in the [export](export.md) dialog, each file gets
standard XMP metadata (IPTC Core, Dublin Core and EXIF fields) that photo
management apps read and can filter on:

| Field | Exported as (standard field) |
|---|---|
| Film stock, format, process | Hierarchical keywords: `Film › Stock › …`, `Film › Format › …`, `Film › Process › …` |
| Archive location | Hierarchical keyword `Archive › Binder 3 › Page 12`, plus IPTC Instructions |
| Roll | IPTC Job Identifier |
| Date shot | EXIF date taken and IPTC Date Created |
| Camera, lens, ISO | EXIF camera model, lens model and ISO |
| Description | IPTC Description (caption) |
| Place, city, country | IPTC Sublocation, City, Country |
| Keywords | IPTC Keywords (flat and hierarchical) |
| Creator, copyright | IPTC Creator and Copyright Notice |
| Source scan, conversion settings | XMP "derived from" reference, and ContactSheet's own fields |

Film has no standard fields for stock or archive location, so they become
hierarchical keywords, which photo management apps can filter on. Rating,
flags and titles are left to those apps. The exact schema properties are listed
in the [development notes](development.md#xmp-mapping).
