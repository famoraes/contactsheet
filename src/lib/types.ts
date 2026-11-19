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
}

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
  width: number;
  height: number;
  bitDepth: number;
  analysis: Analysis;
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
};

export type SliderKey = "density" | "contrast" | "saturation" | "cyan" | "magenta" | "yellow";
