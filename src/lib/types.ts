// Mirrors the serde structs in crates/negative and src-tauri/src/commands.rs.

export type Rgb = [number, number, number];

export interface Adjustments {
  density: number;
  contrast: number;
  saturation: number;
  cyan: number;
  magenta: number;
  yellow: number;
  grayscale: boolean;
  /** Paper preset: normal (punchy, open shadows) or flat (a base for further editing). */
  look: LookPreset;
}

export type LookPreset = "normal" | "flat";

export interface Analysis {
  lo: Rgb;
  hi: Rgb;
}

/** Quarter turns clockwise, then an optional left-right mirror (vertical flip = mirror + 180°). */
export interface Orientation {
  rotation: number;
  mirror: boolean;
}

export const DEFAULT_ORIENTATION: Orientation = { rotation: 0, mirror: false };

/** Display-space crop, normalized to the oriented image; `angle` straightens (degrees, clockwise). */
export interface Crop {
  x: number;
  y: number;
  w: number;
  h: number;
  angle: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ViewOptions {
  showNegative: boolean;
  showClipping: boolean;
}

export interface ImageMeta {
  id: number;
  name: string;
  path: string;
  /** Stored (unrotated) pixel size. */
  width: number;
  height: number;
  bitDepth: number;
  analysis: Analysis;
  /** From the file's TIFF Orientation tag. */
  orientation: Orientation;
}

export interface ExportOptions {
  outDir: string;
  tiff: boolean;
  jpeg: boolean;
  jpegQuality: number;
  maxSize: number | null;
}

export interface ExportProgress {
  done: number;
  total: number;
  name: string;
  error: string | null;
}

export interface ExportSummary {
  written: string[];
  errors: string[];
}

export const DEFAULT_ADJUSTMENTS: Adjustments = {
  density: 0,
  contrast: 0,
  saturation: 0,
  cyan: 0,
  magenta: 0,
  yellow: 0,
  grayscale: false,
  look: "normal",
};

export type SliderKey = "density" | "contrast" | "saturation" | "cyan" | "magenta" | "yellow";

/** Descriptive metadata for a frame, saved in the roll file and embedded on export. */
export interface FrameMeta {
  filmStock: string;
  format: string;
  process: string;
  /** Box speed / EI, as typed. */
  iso: string;
  camera: string;
  lens: string;
  /** YYYY, YYYY-MM or YYYY-MM-DD. */
  dateShot: string;
  location: string;
  city: string;
  country: string;
  rollId: string;
  /** Values for the archive levels (names are an app preference, e.g. Binder / Page / Strip). */
  archive: string[];
  description: string;
  /** Comma-separated; `>` or `|` makes a hierarchy, e.g. "Travel > Brazil". */
  keywords: string;
  creator: string;
  copyright: string;
}

export type MetaKey = Exclude<keyof FrameMeta, "archive">;

export const EMPTY_META: FrameMeta = {
  filmStock: "",
  format: "",
  process: "",
  iso: "",
  camera: "",
  lens: "",
  dateShot: "",
  location: "",
  city: "",
  country: "",
  rollId: "",
  archive: [],
  description: "",
  keywords: "",
  creator: "",
  copyright: "",
};

/** Shape sent to Rust (crates/negative/src/metadata.rs `Metadata`). */
export interface ExportMetadata {
  filmStock: string;
  format: string;
  process: string;
  camera: string;
  lens: string;
  iso: number | null;
  dateShot: string;
  rollId: string;
  description: string;
  location: string;
  city: string;
  country: string;
  archive: { name: string; value: string }[];
  keywords: string[];
  creator: string;
  copyright: string;
}
