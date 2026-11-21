// Per-folder roll files: `roll.contactsheet` (JSON) next to the scans holds each
// frame's edits and metadata, keyed by file name, so a roll reopens as you left it
// and the file moves with the folder.

import * as api from "./api";
import {
  DEFAULT_ADJUSTMENTS,
  DEFAULT_DUST,
  EMPTY_META,
  type Adjustments,
  type Analysis,
  type Crop,
  type Dust,
  type FrameMeta,
  type Orientation,
} from "./types";

export interface SavedFrame {
  adjustments: Adjustments;
  analysis: Analysis;
  autoAnalysis: Analysis;
  orientation: Orientation;
  crop: Crop | null;
  neutralPoints: [number, number][];
  meta: FrameMeta;
  /** Base correction is shared with other frames (roll analysis / paste). */
  sharedBase: boolean;
  dust: Dust;
}

interface RollFile {
  app: "ContactSheet";
  version: 1;
  /** Archive level names at the time of saving, so the values stay meaningful. */
  archiveLevels?: string[];
  frames: Record<string, SavedFrame>;
}

export interface SaveInput {
  id: number;
  path: string;
  saved: SavedFrame;
}

/** The saved fields of a frame, in a fixed order (so JSON comparisons are stable). */
export const pickSaved = (f: SavedFrame): SavedFrame => ({
  adjustments: f.adjustments,
  analysis: f.analysis,
  autoAnalysis: f.autoAnalysis,
  orientation: f.orientation,
  crop: f.crop,
  neutralPoints: f.neutralPoints,
  meta: f.meta,
  sharedBase: f.sharedBase,
  dust: f.dust,
});

const SAVE_DELAY_MS = 600;

export const dirOf = (p: string) => {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i > 0 ? p.slice(0, i) : p.slice(0, i + 1) || ".";
};
export const baseOf = (p: string) => p.slice(Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")) + 1);

/** Loaded roll files by folder. Unreadable files are kept out so they are never overwritten. */
const rolls = new Map<string, Promise<RollFile | null>>();
const lastWritten = new Map<string, string>();
/** Frames as first loaded (JSON): a frame is only written once it differs. */
const pristine = new Map<number, string>();

const emptyRoll = (): RollFile => ({ app: "ContactSheet", version: 1, frames: {} });

function loadRoll(dir: string): Promise<RollFile | null> {
  let p = rolls.get(dir);
  if (!p) {
    p = api.readRoll(dir).then((text) => {
      if (text == null) return emptyRoll();
      const parsed = JSON.parse(text) as Partial<RollFile>;
      if (!parsed || typeof parsed.frames !== "object" || parsed.frames === null) {
        throw new Error("not a ContactSheet roll file");
      }
      lastWritten.set(dir, text);
      return { ...emptyRoll(), ...parsed, frames: parsed.frames } as RollFile;
    });
    // Keep the rejection for callers, but remember the folder as unwritable.
    p = p.catch((e) => {
      onError(`Could not read ${dir}/roll.contactsheet (${e}); edits in this folder won't be saved.`);
      return null;
    });
    rolls.set(dir, p);
  }
  return p;
}

let onError: (msg: string) => void = () => undefined;
export const setRollErrorHandler = (fn: (msg: string) => void) => {
  onError = fn;
};

/** Saved state for a scan, if its folder's roll file has it. */
export async function savedFrame(path: string): Promise<SavedFrame | null> {
  const roll = await loadRoll(dirOf(path));
  const s = roll?.frames[baseOf(path)];
  if (!s || !s.analysis) return null;
  return {
    adjustments: { ...DEFAULT_ADJUSTMENTS, ...s.adjustments },
    analysis: s.analysis,
    autoAnalysis: s.autoAnalysis ?? s.analysis,
    orientation: s.orientation ?? { rotation: 0, mirror: false },
    crop: s.crop ?? null,
    neutralPoints: s.neutralPoints ?? [],
    meta: { ...EMPTY_META, ...s.meta, archive: s.meta?.archive ?? [] },
    sharedBase: s.sharedBase ?? false,
    dust: { ...DEFAULT_DUST, ...s.dust },
  };
}

/** Remember a frame's state at load; it is written only after it changes. */
export const markLoaded = (id: number, f: SavedFrame) => pristine.set(id, JSON.stringify(pickSaved(f)));

let timer: number | undefined;
let chain: Promise<void> = Promise.resolve();
let source: (() => { frames: SaveInput[]; archiveLevels: string[] }) | null = null;

async function writeAll() {
  if (!source) return;
  const { frames, archiveLevels } = source();
  const byDir = new Map<string, SaveInput[]>();
  for (const f of frames) {
    const d = dirOf(f.path);
    byDir.set(d, [...(byDir.get(d) ?? []), f]);
  }
  for (const [dir, list] of byDir) {
    const roll = await loadRoll(dir);
    if (!roll) continue;
    let touched = false;
    for (const { id, path, saved } of list) {
      const name = baseOf(path);
      // Untouched frames are not written, so just viewing a folder creates no file.
      if (!roll.frames[name] && pristine.get(id) === JSON.stringify(saved)) continue;
      roll.frames[name] = saved;
      touched = true;
    }
    if (!touched && !lastWritten.has(dir)) continue;
    roll.archiveLevels = archiveLevels;
    const json = JSON.stringify(roll, null, 1);
    if (json === lastWritten.get(dir)) continue;
    try {
      await api.writeRoll(dir, json);
      lastWritten.set(dir, json);
    } catch (e) {
      onError(String(e));
    }
  }
}

/** Debounced save of every frame; `get` is called when the save actually runs. */
export function scheduleSave(get: NonNullable<typeof source>) {
  source = get;
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    timer = undefined;
    chain = chain.then(writeAll);
  }, SAVE_DELAY_MS);
}

/** Write any pending changes now (e.g. before the window closes). */
export async function flushSaves() {
  if (timer !== undefined) {
    window.clearTimeout(timer);
    timer = undefined;
    chain = chain.then(writeAll);
  }
  await chain;
}
