import { invoke } from "@tauri-apps/api/core";
import type {
  Adjustments,
  Analysis,
  ExportOptions,
  ExportSummary,
  ImageMeta,
  Rect,
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
  view: ViewOptions,
): Promise<PreviewFrame> {
  const buf = await invoke<ArrayBuffer>("render_preview", { id, adjustments, analysis, view });
  const dv = new DataView(buf);
  const width = dv.getUint32(0, true);
  const height = dv.getUint32(4, true);
  return { width, height, pixels: new Uint8ClampedArray(buf, 8, width * height * 4) };
}

export async function renderThumbnail(
  id: number,
  adjustments: Adjustments,
  analysis: Analysis,
): Promise<Blob> {
  const buf = await invoke<ArrayBuffer>("render_thumbnail", { id, adjustments, analysis });
  return new Blob([buf], { type: "image/jpeg" });
}

export const recalc = (id: number, region: Rect | null) =>
  invoke<Analysis>("recalc", { id, region });

export const exportImages = (
  items: { id: number; adjustments: Adjustments; analysis: Analysis }[],
  options: ExportOptions,
) => invoke<ExportSummary>("export_images", { items, options });
