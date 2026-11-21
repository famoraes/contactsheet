import { invoke } from "@tauri-apps/api/core";
import type {
  Adjustments,
  Analysis,
  ExportOptions,
  ExportSummary,
  Crop,
  DustParams,
  ExportMetadata,
  ImageMeta,
  Orientation,
  ViewOptions,
} from "./types";

export interface PreviewFrame {
  width: number;
  height: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

export const addImage = (path: string) => invoke<ImageMeta>("add_image", { path });

export const removeImages = (ids: number[]) => invoke<void>("remove_images", { ids });

export async function renderPreview(
  id: number,
  adjustments: Adjustments,
  analysis: Analysis,
  orientation: Orientation,
  crop: Crop | null,
  view: ViewOptions,
  dust: DustParams,
): Promise<PreviewFrame> {
  const buf = await invoke<ArrayBuffer>("render_preview", { id, adjustments, analysis, orientation, crop, view, dust });
  const dv = new DataView(buf);
  const width = dv.getUint32(0, true);
  const height = dv.getUint32(4, true);
  return { width, height, pixels: new Uint8ClampedArray(buf, 8, width * height * 4) };
}

export interface RegionTile extends PreviewFrame {
  /** Full-resolution frame size (after orientation and crop). */
  fullWidth: number;
  fullHeight: number;
}

/** Part of the frame at full resolution (100% zoom); x/y/w/h in full-res frame pixels. */
export async function renderRegion(
  id: number,
  adjustments: Adjustments,
  analysis: Analysis,
  orientation: Orientation,
  crop: Crop | null,
  view: ViewOptions,
  rect: { x: number; y: number; w: number; h: number },
  dust: DustParams,
): Promise<RegionTile> {
  const buf = await invoke<ArrayBuffer>("render_region", { id, adjustments, analysis, orientation, crop, view, ...rect, dust });
  const dv = new DataView(buf);
  const [width, height] = [dv.getUint32(0, true), dv.getUint32(4, true)];
  return {
    width,
    height,
    fullWidth: dv.getUint32(8, true),
    fullHeight: dv.getUint32(12, true),
    pixels: new Uint8ClampedArray(buf, 16, width * height * 4),
  };
}

export async function renderThumbnail(
  id: number,
  adjustments: Adjustments,
  analysis: Analysis,
  orientation: Orientation,
  crop: Crop | null,
): Promise<Blob> {
  const buf = await invoke<ArrayBuffer>("render_thumbnail", { id, adjustments, analysis, orientation, crop });
  return new Blob([buf], { type: "image/jpeg" });
}

/** Re-analyse from the cropped frame (or the default border inset when uncropped). */
export const recalc = (id: number, orientation: Orientation, crop: Crop | null, dust: DustParams) =>
  invoke<Analysis>("recalc", { id, orientation, crop, dust });

/** One base correction shared by all given frames (pooled analysis). */
export const rollAnalysis = (items: { id: number; orientation: Orientation; crop: Crop | null; dust: DustParams }[]) =>
  invoke<Analysis>("roll_analysis", { items });

/** Picture area of the frame as displayed, or null when it has no border. */
export const autoCrop = (id: number, orientation: Orientation) =>
  invoke<Crop | null>("auto_crop", { id, orientation });

/** Points are normalized display coordinates; `analysis` should be the automatic one. */
export const neutralize = (
  id: number,
  points: [number, number][],
  orientation: Orientation,
  crop: Crop | null,
  analysis: Analysis,
  dust: DustParams,
) => invoke<Analysis>("neutralize", { id, points, orientation, crop, analysis, dust });

export const exportImages = (
  items: {
    id: number;
    adjustments: Adjustments;
    analysis: Analysis;
    orientation: Orientation;
    crop: Crop | null;
    metadata: ExportMetadata | null;
    dust: DustParams;
  }[],
  options: ExportOptions,
) => invoke<ExportSummary>("export_images", { items, options });

/** Watch `dir` for new scans (events "hot-folder" / "hot-folder-error"); null stops. */
export const watchFolder = (dir: string | null) => invoke<void>("watch_folder", { dir });

/** Contents (JSON) of the roll file in `dir`, or null when there is none. */
export const readRoll = (dir: string) => invoke<string | null>("read_roll", { dir });

export const writeRoll = (dir: string, contents: string) => invoke<void>("write_roll", { dir, contents });
