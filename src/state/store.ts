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
import { useShallow } from "zustand/react/shallow";
import { DEFAULT_ARCHIVE_LEVELS, EMPTY_FILTER, isFilterActive, matchesFilter, type FrameFilter } from "../lib/meta";
import { markLoaded, pickSaved, savedFrame, scheduleSave, setRollErrorHandler } from "../lib/roll";
import {
  DEFAULT_ADJUSTMENTS,
  DEFAULT_ORIENTATION,
  EMPTY_META,
  type Adjustments,
  type Analysis,
  type Crop,
  type ExportProgress,
  type FrameMeta,
  type LookPreset,
  type ImageMeta,
  type MetaKey,
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
  /** Descriptive metadata (film stock, dates, archive location...). */
  meta: FrameMeta;
  /** Base correction came from a roll analysis or a paste: crop changes keep it. */
  sharedBase: boolean;
}

/** Copied settings; the base correction is only pasted with "Paste + base". */
interface Clipboard {
  adjustments: Adjustments;
  analysis: Analysis;
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
  clipboard: Clipboard | null;
  showClipping: boolean;
  /** Show the untouched auto conversion (B). */
  showBefore: boolean;
  /** Show the raw negative (⇧B). */
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
  panelTab: "adjust" | "info";
  /** Tab: hide toolbar, panel and filmstrip to show only the image. */
  panelsHidden: boolean;
  /** Names of the physical archive levels, e.g. Binder / Page / Strip. */
  archiveLevels: string[];
  filter: FrameFilter;

  loadPaths: (paths: string[]) => Promise<void>;
  select: (id: number, mods?: { shift?: boolean; toggle?: boolean }) => void;
  selectAll: () => void;
  go: (delta: number) => void;
  goTo: (index: number) => void;
  setAdjustment: (key: SliderKey, value: number) => void;
  nudge: (key: SliderKey, delta: number) => void;
  toggleGrayscale: () => void;
  setLook: (look: LookPreset) => void;
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
  /** Detect the frame edges: sets the box in crop mode, else crops every selected frame. */
  autoCrop: () => Promise<void>;
  resetCurrent: () => void;
  recalcCurrent: () => Promise<void>;
  copy: () => void;
  /** Paste adjustments (sliders, look, grayscale) to the selected frames. */
  paste: () => void;
  /** Paste adjustments and the base correction, so the frames match the copied one. */
  pasteWithBase: () => void;
  /** Analyse the selected frames (or all shown frames) together: one shared base. */
  rollAnalysis: () => Promise<void>;
  removeSelection: () => Promise<void>;
  toggleClipping: () => void;
  toggleBefore: () => void;
  toggleNegative: () => void;
  setHistogram: (h: Histogram | null) => void;
  setExportOpen: (open: boolean) => void;
  setExportProgress: (p: ExportProgress | null) => void;
  dismissErrors: () => void;
  setPanelTab: (tab: "adjust" | "info") => void;
  togglePanels: (hidden?: boolean) => void;
  /** Set metadata fields on every selected frame. */
  setMeta: (patch: Partial<Pick<FrameMeta, MetaKey>>) => void;
  setArchiveValue: (level: number, value: string) => void;
  setArchiveLevels: (names: string[]) => void;
  setFilter: (filter: FrameFilter) => void;
}

// ---- Preferences (app-wide, not per roll) -----------------------------------

interface Prefs {
  archiveLevels: string[];
  /** Pre-filled on frames that have no saved metadata. */
  creator: string;
  copyright: string;
}

const PREFS_KEY = "contactsheet.prefs";
const DEFAULT_PREFS: Prefs = { archiveLevels: DEFAULT_ARCHIVE_LEVELS, creator: "", copyright: "" };

function loadPrefs(): Prefs {
  try {
    return { ...DEFAULT_PREFS, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
  } catch {
    return DEFAULT_PREFS;
  }
}

const savePrefs = (p: Partial<Prefs>) => localStorage.setItem(PREFS_KEY, JSON.stringify({ ...loadPrefs(), ...p }));

/** Frames shown in the filmstrip: those matching the filter, plus the current one. */
export function visibleFrames(s: Pick<State, "frames" | "filter" | "currentId" | "archiveLevels">): Frame[] {
  if (!isFilterActive(s.filter)) return s.frames;
  return s.frames.filter(
    (f) => f.id === s.currentId || matchesFilter(f.name, f.meta, s.filter, s.archiveLevels),
  );
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

  const visible = () => visibleFrames(get());

  /** Frames that metadata edits and paste apply to. */
  const targets = () => {
    const { selection, currentId } = get();
    return new Set(selection.length ? selection : currentId != null ? [currentId] : []);
  };

  const updateTargets = (fn: (f: Frame) => Frame) => {
    const ids = targets();
    set((s) => ({ frames: s.frames.map((f) => (ids.has(f.id) ? fn(f) : f)) }));
  };

  /** Re-analyse a frame from its current crop; drops neutral picks (they were relative to the old analysis). */
  const recalcFrame = async (id: number) => {
    const f = get().frames.find((x) => x.id === id);
    if (!f) return;
    try {
      const analysis = await api.recalc(id, f.orientation, f.crop);
      updateFrame(id, (x) => ({ ...x, analysis, autoAnalysis: analysis, neutralPoints: [], sharedBase: false }));
    } catch (e) {
      fail(e);
    }
  };

  /** After a crop change: re-analyse, unless the frame uses a shared base. */
  const recalcAfterCrop = async (id: number) => {
    if (!get().frames.find((x) => x.id === id)?.sharedBase) await recalcFrame(id);
  };

  const setCurrentCrop = (crop: Crop) => updateCurrent((f) => ({ ...f, crop }));

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
    showBefore: false,
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
    panelTab: "adjust",
    panelsHidden: false,
    archiveLevels: loadPrefs().archiveLevels,
    filter: EMPTY_FILTER,

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
            const [meta, saved] = await Promise.all([api.addImage(p), savedFrame(p).catch(() => null)]);
            const prefs = loadPrefs();
            const frame: Frame = {
              ...meta,
              adjustments: { ...DEFAULT_ADJUSTMENTS },
              orientation: meta.orientation ?? { ...DEFAULT_ORIENTATION },
              crop: null,
              neutralPoints: [],
              autoAnalysis: meta.analysis,
              meta: { ...EMPTY_META, creator: prefs.creator, copyright: prefs.copyright },
              sharedBase: false,
              // Edits and metadata from the folder's roll file win.
              ...(saved ?? {}),
            };
            markLoaded(frame.id, frame);
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
      const { selection, anchorId } = get();
      const frames = visible();
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
      set({ selection: visible().map((f) => f.id) });
    },

    go(delta) {
      const { currentId } = get();
      const frames = visible();
      if (!frames.length) return;
      const i = frames.findIndex((f) => f.id === currentId);
      get().goTo(Math.max(0, Math.min(frames.length - 1, i + delta)));
    },

    /** Index into the visible (filtered) frames. */
    goTo(index) {
      const f = visible()[index];
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

    setLook(look) {
      updateCurrent((f) => ({ ...f, adjustments: { ...f.adjustments, look } }));
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
      if (!sameCrop(crop, cropBackup)) await recalcAfterCrop(f.id);
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
        void recalcAfterCrop(f.id);
      }
    },

    async autoCrop() {
      const { cropMode, cropAspect, cropPortrait } = get();
      const f = current();
      if (cropMode) {
        if (!f) return;
        try {
          const found = await api.autoCrop(f.id, f.orientation);
          const size = displaySize(f);
          const c = found ?? { ...FULL_CROP };
          const ratio = aspectRatio(cropAspect, cropPortrait, size);
          setCurrentCrop(ratio ? conformAspect(c, ratio, size) : fitCrop(c, size));
        } catch (e) {
          fail(e);
        }
        return;
      }
      const ids = [...targets()];
      const misses: string[] = [];
      await Promise.all(
        ids.map(async (id) => {
          const x = get().frames.find((fr) => fr.id === id);
          if (!x) return;
          try {
            const found = await api.autoCrop(id, x.orientation);
            const crop = found ? fitCrop(found, displaySize(x)) : null;
            if (!found) misses.push(x.name);
            if (sameCrop(crop, x.crop)) return;
            updateFrame(id, (y) => ({ ...y, crop }));
            await recalcAfterCrop(id);
          } catch (e) {
            fail(e);
          }
        }),
      );
      if (misses.length && ids.length === 1) fail(`AutoCrop: no border found on ${misses[0]}.`);
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
      if (f) set({ clipboard: { adjustments: { ...f.adjustments }, analysis: f.analysis } });
    },

    paste() {
      const { clipboard } = get();
      if (!clipboard) return;
      updateTargets((f) => ({ ...f, adjustments: { ...clipboard.adjustments } }));
    },

    pasteWithBase() {
      const { clipboard } = get();
      if (!clipboard) return;
      // The copied frame's corrected base (including any neutral pick) becomes the
      // targets' automatic base, so Reset and Before keep it.
      updateTargets((f) => ({
        ...f,
        adjustments: { ...clipboard.adjustments },
        analysis: clipboard.analysis,
        autoAnalysis: clipboard.analysis,
        neutralPoints: [],
        sharedBase: true,
      }));
    },

    async rollAnalysis() {
      const { selection } = get();
      const ids = new Set(selection.length > 1 ? selection : visible().map((f) => f.id));
      const frames = get().frames.filter((f) => ids.has(f.id));
      if (!frames.length) return;
      leaveModes();
      try {
        const analysis = await api.rollAnalysis(
          frames.map((f) => ({ id: f.id, orientation: f.orientation, crop: f.crop })),
        );
        set((s) => ({
          frames: s.frames.map((f) =>
            ids.has(f.id) ? { ...f, analysis, autoAnalysis: analysis, neutralPoints: [], sharedBase: true } : f,
          ),
        }));
      } catch (e) {
        fail(e);
      }
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
    toggleBefore: () => set((s) => ({ showBefore: !s.showBefore, showNegative: false })),
    toggleNegative: () => set((s) => ({ showNegative: !s.showNegative, showBefore: false })),
    setHistogram: (histogram) => set({ histogram }),
    setExportOpen: (exportOpen) => set({ exportOpen }),
    setExportProgress: (exportProgress) => set({ exportProgress }),
    dismissErrors: () => set({ errors: [] }),
    setPanelTab: (panelTab) => set({ panelTab }),
    togglePanels: (hidden) => set((s) => ({ panelsHidden: hidden ?? !s.panelsHidden })),

    setMeta(patch) {
      updateTargets((f) => ({ ...f, meta: { ...f.meta, ...patch } }));
      const prefs: Partial<Prefs> = {};
      if (patch.creator !== undefined) prefs.creator = patch.creator;
      if (patch.copyright !== undefined) prefs.copyright = patch.copyright;
      if (Object.keys(prefs).length) savePrefs(prefs);
    },

    setArchiveValue(level, value) {
      updateTargets((f) => {
        const archive = Array.from({ length: Math.max(f.meta.archive.length, level + 1) }, (_, i) =>
          i === level ? value : (f.meta.archive[i] ?? ""),
        );
        return { ...f, meta: { ...f.meta, archive } };
      });
    },

    setArchiveLevels(names) {
      savePrefs({ archiveLevels: names });
      set({ archiveLevels: names });
    },

    setFilter(filter) {
      set({ filter });
      // Keep the current frame and the selection within what is shown.
      const shown = visibleFrames({ ...get(), currentId: null });
      if (!shown.length) return;
      const ids = new Set(shown.map((f) => f.id));
      const { currentId, selection } = get();
      if (currentId == null || !ids.has(currentId)) get().select(shown[0].id);
      else set({ selection: selection.filter((id) => ids.has(id)) });
    },
  };
});

export const useVisibleFrames = () => useStore(useShallow(visibleFrames));

// ---- Autosave to the per-folder roll files ----------------------------------

setRollErrorHandler((msg) => useStore.setState((s) => ({ errors: [...s.errors, msg] })));

useStore.subscribe((s, prev) => {
  if (s.frames === prev.frames && s.cropMode === prev.cropMode && s.archiveLevels === prev.archiveLevels) return;
  scheduleSave(() => {
    const st = useStore.getState();
    return {
      archiveLevels: st.archiveLevels,
      frames: st.frames.map((f) => ({
        id: f.id,
        path: f.path,
        // An in-progress crop isn't saved until it's applied.
        saved: pickSaved(st.cropMode && f.id === st.currentId ? { ...f, crop: st.cropBackup } : f),
      })),
    };
  });
});

export const useCurrentFrame = () =>
  useStore((s) => s.frames.find((f) => f.id === s.currentId) ?? null);
