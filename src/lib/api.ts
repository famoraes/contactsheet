import { invoke } from "@tauri-apps/api/core";
import type {
  Adjustments,
  Analysis,
  ExportOptions,
  ExportSummary,
  Crop,
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
): Promise<PreviewFrame> {
  const buf = await invoke<ArrayBuffer>("render_preview", { id, adjustments, analysis, orientation, crop, view });
  const dv = new DataView(buf);
  const width = dv.getUint32(0, true);
  const height = dv.getUint32(4, true);
  return { width, height, pixels: new Uint8ClampedArray(buf, 8, width * height * 4) };
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
export const recalc = (id: number, orientation: Orientation, crop: Crop | null) =>
  invoke<Analysis>("recalc", { id, orientation, crop });

/** Points are normalized display coordinates; `analysis` should be the automatic one. */
export const neutralize = (
  id: number,
  points: [number, number][],
  orientation: Orientation,
  crop: Crop | null,
  analysis: Analysis,
) => invoke<Analysis>("neutralize", { id, points, orientation, crop, analysis });

export const exportImages = (
  items: {
    id: number;
    adjustments: Adjustments;
    analysis: Analysis;
    orientation: Orientation;
    crop: Crop | null;
  }[],
  options: ExportOptions,
) => invoke<ExportSummary>("export_images", { items, options });
