// Crop geometry, mirroring crates/negative/src/geometry.rs (`Crop`, `Framed`):
// the image (W x H after orientation) is rotated `angle` degrees clockwise about its
// centre on a W x H canvas, and the crop is a normalized rectangle on that canvas.

import type { Crop, Orientation } from "./types";

export const FULL_CROP: Crop = { x: 0, y: 0, w: 1, h: 1, angle: 0 };
export const MAX_ANGLE = 15;
const MIN_SIZE = 0.03;
const BISECT_STEPS = 14;

export type Handle = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

export interface Size {
  w: number;
  h: number;
}

export const isFullCrop = (c: Crop | null) =>
  !c ||
  (Math.abs(c.angle) < 1e-3 &&
    c.x < 1e-4 &&
    c.y < 1e-4 &&
    c.w > 1 - 1e-4 &&
    c.h > 1 - 1e-4);

export const sameCrop = (a: Crop | null, b: Crop | null) =>
  (isFullCrop(a) && isFullCrop(b)) ||
  (!!a && !!b && (["x", "y", "w", "h", "angle"] as const).every((k) => Math.abs(a[k] - b[k]) < 1e-6));

/** Display dimensions of a stored `width x height` image under `orientation`. */
export const orientedSize = (width: number, height: number, o: Orientation): Size =>
  o.rotation % 2 === 1 ? { w: height, h: width } : { w: width, h: height };

/** Output pixel size of a crop. */
export const cropPixels = (c: Crop, s: Size): Size => ({
  w: Math.max(1, Math.round(c.w * s.w)),
  h: Math.max(1, Math.round(c.h * s.h)),
});

/** True if every corner of the crop lands inside the (rotated) image. */
export function cropFits(c: Crop, s: Size): boolean {
  const eps = 1e-6;
  if (c.w < MIN_SIZE * 0.5 || c.h < MIN_SIZE * 0.5) return false;
  if (c.x < -eps || c.y < -eps || c.x + c.w > 1 + eps || c.y + c.h > 1 + eps) return false;
  if (Math.abs(c.angle) < 1e-3) return true;
  const t = (c.angle * Math.PI) / 180;
  const [cos, sin] = [Math.cos(t), Math.sin(t)];
  const tol = 0.01; // pixels
  for (const [u, v] of [
    [c.x, c.y],
    [c.x + c.w, c.y],
    [c.x, c.y + c.h],
    [c.x + c.w, c.y + c.h],
  ]) {
    const dx = (u - 0.5) * s.w;
    const dy = (v - 0.5) * s.h;
    const sx = s.w / 2 + dx * cos + dy * sin;
    const sy = s.h / 2 - dx * sin + dy * cos;
    if (sx < -tol || sx > s.w + tol || sy < -tol || sy > s.h + tol) return false;
  }
  return true;
}

/** Largest t in [0, 1] with ok(t), assuming ok(0) holds and validity is monotone. */
function bisect(ok: (t: number) => boolean): number {
  if (ok(1)) return 1;
  let [lo, hi] = [0, 1];
  for (let i = 0; i < BISECT_STEPS; i++) {
    const mid = (lo + hi) / 2;
    if (ok(mid)) lo = mid;
    else hi = mid;
  }
  return lo;
}

const scaled = (c: Crop, k: number): Crop => {
  const [cx, cy] = [c.x + c.w / 2, c.y + c.h / 2];
  return { ...c, x: cx - (c.w * k) / 2, y: cy - (c.h * k) / 2, w: c.w * k, h: c.h * k };
};

/** Shrink (and if needed re-centre) the crop until it fits inside the rotated image. */
export function fitCrop(c: Crop, s: Size): Crop {
  if (cropFits(c, s)) return c;
  const minK = MIN_SIZE / Math.min(c.w, c.h);
  if (cropFits(scaled(c, minK), s)) {
    const k = minK + (1 - minK) * bisect((t) => cropFits(scaled(c, minK + (1 - minK) * t), s));
    return scaled(c, k);
  }
  // Centre is outside the image: slide towards the middle while growing.
  const tiny = scaled({ ...c, x: 0.5 - c.w / 2, y: 0.5 - c.h / 2 }, minK);
  const lerp = (t: number): Crop => ({
    ...c,
    x: tiny.x + (c.x - tiny.x) * t,
    y: tiny.y + (c.y - tiny.y) * t,
    w: tiny.w + (c.w - tiny.w) * t,
    h: tiny.h + (c.h - tiny.h) * t,
  });
  return lerp(bisect((t) => cropFits(lerp(t), s)));
}

/** Largest crop of pixel aspect `ratio` (w/h) inside `c`, centred on it, then fitted. */
export function conformAspect(c: Crop, ratio: number | null, s: Size): Crop {
  if (!ratio) return fitCrop(c, s);
  let pw = c.w * s.w;
  let ph = c.h * s.h;
  if (pw / ph > ratio) pw = ph * ratio;
  else ph = pw / ratio;
  const [w, h] = [pw / s.w, ph / s.h];
  return fitCrop({ ...c, x: c.x + (c.w - w) / 2, y: c.y + (c.h - h) / 2, w, h }, s);
}

/** Apply a handle drag of (dx, dy) normalized units, honouring aspect lock and bounds. */
export function dragCrop(start: Crop, handle: Handle, dx: number, dy: number, ratio: number | null, s: Size): Crop {
  const propose = (t: number): Crop => {
    const [ddx, ddy] = [dx * t, dy * t];
    if (handle === "move") {
      return {
        ...start,
        x: Math.min(Math.max(start.x + ddx, 0), 1 - start.w),
        y: Math.min(Math.max(start.y + ddy, 0), 1 - start.h),
      };
    }
    let [l, r, top, b] = [start.x, start.x + start.w, start.y, start.y + start.h];
    if (handle.includes("w")) l = Math.min(l + ddx, r - MIN_SIZE);
    if (handle.includes("e")) r = Math.max(r + ddx, l + MIN_SIZE);
    if (handle.includes("n")) top = Math.min(top + ddy, b - MIN_SIZE);
    if (handle.includes("s")) b = Math.max(b + ddy, top + MIN_SIZE);
    if (ratio) {
      const nr = (ratio * s.h) / s.w; // normalized w / h
      let [w, h] = [r - l, b - top];
      if (handle.length === 2) {
        // Corner: the larger relative change wins; the opposite corner stays put.
        if (w / nr > h) h = w / nr;
        else w = h * nr;
        if (handle.includes("w")) l = r - w;
        else r = l + w;
        if (handle.includes("n")) top = b - h;
        else b = top + h;
      } else if (handle === "e" || handle === "w") {
        h = w / nr;
        const cy = start.y + start.h / 2;
        [top, b] = [cy - h / 2, cy + h / 2];
      } else {
        w = h * nr;
        const cx = start.x + start.w / 2;
        [l, r] = [cx - w / 2, cx + w / 2];
      }
    }
    return { ...start, x: l, y: top, w: r - l, h: b - top };
  };
  // Go as far along the drag as stays valid (the start is always valid).
  return propose(bisect((t) => cropFits(propose(t), s)));
}

/** Transform a display-space crop when the display rotates clockwise by quarter turns. */
export function rotateCrop(c: Crop, quarterTurnsCw: number): Crop {
  let out = c;
  const n = ((quarterTurnsCw % 4) + 4) % 4;
  for (let i = 0; i < n; i++) out = { ...out, x: 1 - out.y - out.h, y: out.x, w: out.h, h: out.w };
  return out;
}

/** Transform a display-space crop when the display is mirrored. */
export function flipCrop(c: Crop, axis: "horizontal" | "vertical"): Crop {
  return axis === "horizontal"
    ? { ...c, x: 1 - c.x - c.w, angle: -c.angle }
    : { ...c, y: 1 - c.y - c.h, angle: -c.angle };
}

export const ASPECTS: { id: string; label: string; ratio: number | null }[] = [
  { id: "free", label: "Free", ratio: null },
  { id: "original", label: "Original", ratio: null },
  { id: "1:1", label: "1:1 (6×6)", ratio: 1 },
  { id: "7:6", label: "7:6 (6×7)", ratio: 7 / 6 },
  { id: "4:3", label: "4:3 (6×4.5)", ratio: 4 / 3 },
  { id: "5:4", label: "5:4 (4×5)", ratio: 5 / 4 },
  { id: "3:2", label: "3:2 (35mm)", ratio: 3 / 2 },
  { id: "16:9", label: "16:9", ratio: 16 / 9 },
];

/** Pixel w/h ratio for an aspect choice, or null when free. */
export function aspectRatio(id: string, portrait: boolean, full: Size): number | null {
  const base = id === "original" ? full.w / full.h : ASPECTS.find((a) => a.id === id)?.ratio ?? null;
  if (!base) return null;
  const landscape = base >= 1 ? base : 1 / base;
  return portrait ? 1 / landscape : landscape;
}
