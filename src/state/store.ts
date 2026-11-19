import { create } from "zustand";
import * as api from "../lib/api";
import {
  aspectRatio,
  conformAspect,
  fitCrop,
  flipCrop,
  FULL_CROP,
  isFullCrop,
  orientedSize,
  rotateCrop,
  sameCrop,
  type Size,
} from "../lib/crop";
import {
  DEFAULT_ADJUSTMENTS,
  DEFAULT_ORIENTATION,
  type Adjustments,
  type Analysis,
  type Crop,
  type ExportProgress,
  type ImageMeta,
  type Orientation,
  type SliderKey,
} from "../lib/types";

export interface Frame extends ImageMeta {
  adjustments: Adjustments;
  /** Geometry, kept apart from adjustments so copy/paste and Reset leave it alone. */
  orientation: Orientation;
  /** Display-space crop + straighten; null = whole frame. */
  crop: Crop | null;
  /** Neutral picks in display coordinates (markers only; the correction lives in `analysis`). */
  neutralPoints: [number, number][];
  /** Analysis from import / last Recalc; Reset returns to it. */
  autoAnalysis: Analysis;
}

export interface Histogram {
  r: Uint32Array;
  g: Uint32Array;
  b: Uint32Array;
}

interface State {
  frames: Frame[];
  currentId: number | null;
  selection: number[];
  anchorId: number | null;
  clipboard: Adjustments | null;
  showClipping: boolean;
  showNegative: boolean;
  neutralMode: boolean;
  cropMode: boolean;
  /** Crop when crop mode was entered, for Cancel. */
  cropBackup: Crop | null;
  cropAspect: string;
  cropPortrait: boolean;
  pendingLoads: number;
  errors: string[];
  histogram: Histogram | null;
  exportOpen: boolean;
  exportProgress: ExportProgress | null;

  loadPaths: (paths: string[]) => Promise<void>;
  select: (id: number, mods?: { shift?: boolean; toggle?: boolean }) => void;
  selectAll: () => void;
  go: (delta: number) => void;
  goTo: (index: number) => void;
  setAdjustment: (key: SliderKey, value: number) => void;
  nudge: (key: SliderKey, delta: number) => void;
  toggleGrayscale: () => void;
  toggleNeutralMode: (on?: boolean) => void;
  pickNeutral: (u: number, v: number, addSecond: boolean) => Promise<void>;
  rotate: (quarterTurnsCw: number) => void;
  flip: (axis: "horizontal" | "vertical") => void;
  enterCrop: () => void;
  commitCrop: () => Promise<void>;
  cancelCrop: () => void;
  setCrop: (crop: Crop) => void;
  setCropAngle: (angle: number) => void;
  setCropAspect: (id: string, portrait?: boolean) => void;
  resetCrop: () => void;
  resetCurrent: () => void;
  recalcCurrent: () => Promise<void>;
  copy: () => void;
  paste: () => void;
  removeSelection: () => Promise<void>;
  toggleClipping: () => void;
  toggleNegative: () => void;
  setHistogram: (h: Histogram | null) => void;
  setExportOpen: (open: boolean) => void;
  setExportProgress: (p: ExportProgress | null) => void;
  dismissErrors: () => void;
}

const TIFF_RE = /\.tiff?$/i;
const LOAD_CONCURRENCY = 3;

const clamp = (v: number) => Math.max(-1, Math.min(1, Math.round(v * 100) / 100));

/** Size of the frame as displayed (after rotation), before cropping. */
export const displaySize = (f: Frame): Size => orientedSize(f.width, f.height, f.orientation);

export const useStore = create<State>((set, get) => {
  const current = () => get().frames.find((f) => f.id === get().currentId) ?? null;

  const updateFrame = (id: number, fn: (f: Frame) => Frame) =>
    set((s) => ({ frames: s.frames.map((f) => (f.id === id ? fn(f) : f)) }));

  const updateCurrent = (fn: (f: Frame) => Frame) => {
    const id = get().currentId;
    if (id != null) updateFrame(id, fn);
  };

  const fail = (e: unknown) => set((s) => ({ errors: [...s.errors, String(e)] }));

  /** Re-analyse a frame from its current crop; drops neutral picks (they were relative to the old analysis). */
  const recalcFrame = async (id: number) => {
    const f = get().frames.find((x) => x.id === id);
    if (!f) return;
    try {
      const analysis = await api.recalc(id, f.orientation, f.crop);
      updateFrame(id, (x) => ({ ...x, analysis, autoAnalysis: analysis, neutralPoints: [] }));
    } catch (e) {
      fail(e);
    }
  };

  /** Leave crop mode before doing anything that changes the current frame. */
  const leaveModes = () => {
    if (get().cropMode) void get().commitCrop();
  };

  return {
    frames: [],
    currentId: null,
    selection: [],
    anchorId: null,
    clipboard: null,
    showClipping: false,
    showNegative: false,
    neutralMode: false,
    cropMode: false,
    cropBackup: null,
    cropAspect: "free",
    cropPortrait: false,
    pendingLoads: 0,
    errors: [],
    histogram: null,
    exportOpen: false,
    exportProgress: null,

    async loadPaths(paths) {
      const known = new Set(get().frames.map((f) => f.path));
      const queue = paths.filter((p) => TIFF_RE.test(p) && !known.has(p));
      const skipped = paths.length - queue.length;
      if (skipped > 0 && queue.length === 0) {
        set((s) => ({ errors: [...s.errors, "Only new .tif/.tiff files can be added."] }));
        return;
      }
      set((s) => ({ pendingLoads: s.pendingLoads + queue.length }));
      const worker = async () => {
        for (let p = queue.shift(); p !== undefined; p = queue.shift()) {
          try {
            const meta = await api.addImage(p);
            const frame: Frame = {
              ...meta,
              adjustments: { ...DEFAULT_ADJUSTMENTS },
              orientation: meta.orientation ?? { ...DEFAULT_ORIENTATION },
              crop: null,
              neutralPoints: [],
              autoAnalysis: meta.analysis,
            };
            set((s) => ({
              frames: [...s.frames, frame],
              currentId: s.currentId ?? frame.id,
              selection: s.currentId == null ? [frame.id] : s.selection,
              anchorId: s.anchorId ?? frame.id,
            }));
          } catch (e) {
            fail(e);
          } finally {
            set((s) => ({ pendingLoads: s.pendingLoads - 1 }));
          }
        }
      };
      await Promise.all(Array.from({ length: LOAD_CONCURRENCY }, worker));
    },

    select(id, mods = {}) {
      if (id !== get().currentId) leaveModes();
      const { frames, selection, anchorId } = get();
      if (mods.shift && anchorId != null) {
        const a = frames.findIndex((f) => f.id === anchorId);
        const b = frames.findIndex((f) => f.id === id);
        const [lo, hi] = a < b ? [a, b] : [b, a];
        set({ currentId: id, selection: frames.slice(lo, hi + 1).map((f) => f.id) });
      } else if (mods.toggle) {
        const has = selection.includes(id);
        const next = has ? selection.filter((s) => s !== id) : [...selection, id];
        set({ currentId: id, selection: next.length ? next : [id], anchorId: id });
      } else {
        set({ currentId: id, selection: [id], anchorId: id });
      }
    },

    selectAll() {
      set((s) => ({ selection: s.frames.map((f) => f.id) }));
    },

    go(delta) {
      const { frames, currentId } = get();
      if (!frames.length) return;
      const i = frames.findIndex((f) => f.id === currentId);
      get().goTo(Math.max(0, Math.min(frames.length - 1, i + delta)));
    },

    goTo(index) {
      const f = get().frames[index];
      if (f) get().select(f.id);
    },

    setAdjustment(key, value) {
      updateCurrent((f) => ({ ...f, adjustments: { ...f.adjustments, [key]: clamp(value) } }));
    },

    nudge(key, delta) {
      updateCurrent((f) => ({
        ...f,
        adjustments: { ...f.adjustments, [key]: clamp(f.adjustments[key] + delta) },
      }));
    },

    toggleGrayscale() {
      updateCurrent((f) => ({
        ...f,
        adjustments: { ...f.adjustments, grayscale: !f.adjustments.grayscale },
      }));
    },

    toggleNeutralMode(on) {
      const next = on ?? !get().neutralMode;
      if (next) leaveModes();
      set({ neutralMode: next });
    },

    // Click = one-point neutral (uniform cast). Shift-click adds/replaces a second point;
    // two points in different tones also fix shadow/highlight crossover casts.
    // Always computed from the automatic analysis so repeated picks don't compound.
    async pickNeutral(u, v, addSecond) {
      const f = current();
      if (!f) return;
      const p: [number, number] = [u, v];
      const points: [number, number][] =
        addSecond && f.neutralPoints.length ? [f.neutralPoints[0], p] : [p];
      try {
        const analysis = await api.neutralize(f.id, points, f.orientation, f.crop, f.autoAnalysis);
        updateFrame(f.id, (x) => ({
          ...x,
          analysis,
          neutralPoints: points,
          // The pick defines neutral; leftover C/M/Y would push it off again.
          adjustments: { ...x.adjustments, cyan: 0, magenta: 0, yellow: 0 },
        }));
      } catch (e) {
        fail(e);
      }
    },

    // Display = mirror^m ∘ rotate(r). Rotating a mirrored image clockwise equals
    // mirroring an anticlockwise rotation, and a vertical flip is mirror ∘ rotate(180°).
    // The crop lives in display space, so it is carried along.
    rotate(quarterTurnsCw) {
      updateCurrent((f) => {
        const { rotation, mirror } = f.orientation;
        const turn = mirror ? -quarterTurnsCw : quarterTurnsCw;
        return {
          ...f,
          orientation: { rotation: (((rotation + turn) % 4) + 4) % 4, mirror },
          crop: f.crop && rotateCrop(f.crop, quarterTurnsCw),
          neutralPoints: [],
        };
      });
      if (get().cropMode) set((s) => ({ cropPortrait: !s.cropPortrait }));
    },

    flip(axis) {
      updateCurrent((f) => {
        const { rotation, mirror } = f.orientation;
        const extra = axis === "vertical" ? 2 : 0;
        return {
          ...f,
          orientation: { rotation: (rotation + extra) % 4, mirror: !mirror },
          crop: f.crop && flipCrop(f.crop, axis),
          neutralPoints: [],
        };
      });
    },

    enterCrop() {
      const f = current();
      if (!f || get().cropMode) return;
      set({ cropMode: true, neutralMode: false, cropBackup: f.crop });
      updateFrame(f.id, (x) => ({ ...x, crop: x.crop ?? { ...FULL_CROP } }));
    },

    async commitCrop() {
      const f = current();
      const { cropMode, cropBackup } = get();
      if (!f || !cropMode) return;
      const crop = isFullCrop(f.crop) ? null : f.crop;
      set({ cropMode: false, cropBackup: null });
      updateFrame(f.id, (x) => ({ ...x, crop }));
      // A new crop changes what the auto analysis should look at (e.g. border removed).
      if (!sameCrop(crop, cropBackup)) await recalcFrame(f.id);
    },

    cancelCrop() {
      const f = current();
      const { cropMode, cropBackup } = get();
      if (!cropMode) return;
      set({ cropMode: false, cropBackup: null });
      if (f) updateFrame(f.id, (x) => ({ ...x, crop: cropBackup }));
    },

    setCrop(crop) {
      updateCurrent((f) => ({ ...f, crop }));
    },

    setCropAngle(angle) {
      updateCurrent((f) => {
        const c = { ...(f.crop ?? FULL_CROP), angle };
        const ratio = aspectRatio(get().cropAspect, get().cropPortrait, displaySize(f));
        return { ...f, crop: conformAspect(fitCrop(c, displaySize(f)), ratio, displaySize(f)) };
      });
    },

    setCropAspect(id, portrait) {
      const f = current();
      const size = f ? displaySize(f) : null;
      const c = f?.crop ?? FULL_CROP;
      // Default the orientation of the ratio to the current crop's shape.
      const isPortrait =
        portrait ?? (size ? c.h * size.h > c.w * size.w : get().cropPortrait);
      set({ cropAspect: id, cropPortrait: isPortrait });
      if (f && size && get().cropMode) {
        const ratio = aspectRatio(id, isPortrait, size);
        updateFrame(f.id, (x) => ({ ...x, crop: conformAspect({ ...c }, ratio, size) }));
      }
    },

    resetCrop() {
      const f = current();
      if (!f) return;
      if (get().cropMode) {
        updateFrame(f.id, (x) => ({ ...x, crop: { ...FULL_CROP } }));
      } else if (f.crop) {
        updateFrame(f.id, (x) => ({ ...x, crop: null }));
        void recalcFrame(f.id);
      }
    },

    resetCurrent() {
      updateCurrent((f) => ({
        ...f,
        adjustments: { ...DEFAULT_ADJUSTMENTS },
        analysis: f.autoAnalysis,
        neutralPoints: [],
      }));
    },

    async recalcCurrent() {
      const id = get().currentId;
      if (id != null) await recalcFrame(id);
    },

    copy() {
      const f = current();
      if (f) set({ clipboard: { ...f.adjustments } });
    },

    paste() {
      const { clipboard, selection, currentId } = get();
      if (!clipboard) return;
      const targets = new Set(selection.length ? selection : currentId != null ? [currentId] : []);
      set((s) => ({
        frames: s.frames.map((f) =>
          targets.has(f.id) ? { ...f, adjustments: { ...clipboard } } : f,
        ),
      }));
    },

    async removeSelection() {
      const { selection, frames, currentId } = get();
      if (!selection.length) return;
      set({ cropMode: false, cropBackup: null, neutralMode: false });
      const gone = new Set(selection);
      const idx = frames.findIndex((f) => f.id === currentId);
      const remaining = frames.filter((f) => !gone.has(f.id));
      const next = remaining[Math.min(Math.max(idx, 0), remaining.length - 1)];
      set({
        frames: remaining,
        currentId: next?.id ?? null,
        selection: next ? [next.id] : [],
        anchorId: next?.id ?? null,
        histogram: next ? get().histogram : null,
      });
      await api.removeImages(selection).catch(() => undefined);
    },

    toggleClipping: () => set((s) => ({ showClipping: !s.showClipping })),
    toggleNegative: () => set((s) => ({ showNegative: !s.showNegative })),
    setHistogram: (histogram) => set({ histogram }),
    setExportOpen: (exportOpen) => set({ exportOpen }),
    setExportProgress: (exportProgress) => set({ exportProgress }),
    dismissErrors: () => set({ errors: [] }),
  };
});

export const useCurrentFrame = () =>
  useStore((s) => s.frames.find((f) => f.id === s.currentId) ?? null);
