import { create } from "zustand";
import * as api from "../lib/api";
import {
  DEFAULT_ADJUSTMENTS,
  type Adjustments,
  type Analysis,
  type ExportProgress,
  type ImageMeta,
  type SliderKey,
} from "../lib/types";

export interface Frame extends ImageMeta {
  adjustments: Adjustments;
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

export const useStore = create<State>((set, get) => {
  const updateCurrent = (fn: (f: Frame) => Frame) => {
    const { currentId, frames } = get();
    if (currentId == null) return;
    set({ frames: frames.map((f) => (f.id === currentId ? fn(f) : f)) });
  };

  return {
    frames: [],
    currentId: null,
    selection: [],
    anchorId: null,
    clipboard: null,
    showClipping: false,
    showNegative: false,
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
              autoAnalysis: meta.analysis,
            };
            set((s) => ({
              frames: [...s.frames, frame],
              currentId: s.currentId ?? frame.id,
              selection: s.currentId == null ? [frame.id] : s.selection,
              anchorId: s.anchorId ?? frame.id,
            }));
          } catch (e) {
            set((s) => ({ errors: [...s.errors, String(e)] }));
          } finally {
            set((s) => ({ pendingLoads: s.pendingLoads - 1 }));
          }
        }
      };
      await Promise.all(Array.from({ length: LOAD_CONCURRENCY }, worker));
    },

    select(id, mods = {}) {
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

    resetCurrent() {
      updateCurrent((f) => ({
        ...f,
        adjustments: { ...DEFAULT_ADJUSTMENTS },
        analysis: f.autoAnalysis,
      }));
    },

    async recalcCurrent() {
      const id = get().currentId;
      if (id == null) return;
      try {
        // M2: pass the crop rectangle here; null = default border-skipping inset.
        const analysis = await api.recalc(id, null);
        set((s) => ({
          frames: s.frames.map((f) =>
            f.id === id ? { ...f, analysis, autoAnalysis: analysis } : f,
          ),
        }));
      } catch (e) {
        set((s) => ({ errors: [...s.errors, String(e)] }));
      }
    },

    copy() {
      const { frames, currentId } = get();
      const f = frames.find((x) => x.id === currentId);
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
