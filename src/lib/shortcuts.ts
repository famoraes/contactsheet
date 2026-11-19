import { useStore } from "../state/store";
import type { SliderKey } from "./types";

const STEP = 0.05;

/** Single-key adjustments, SmartConvert-style: everything reachable without the mouse. */
const NUDGES: Record<string, [SliderKey, number]> = {
  q: ["density", STEP],
  a: ["density", -STEP],
  w: ["contrast", STEP],
  s: ["contrast", -STEP],
  e: ["saturation", STEP],
  d: ["saturation", -STEP],
  "1": ["cyan", -STEP],
  "2": ["cyan", STEP],
  "3": ["magenta", -STEP],
  "4": ["magenta", STEP],
  "5": ["yellow", -STEP],
  "6": ["yellow", STEP],
};

export const SHORTCUT_HELP: [string, string][] = [
  ["← / → / Enter", "Previous / next frame"],
  ["Q / A", "Density + / −"],
  ["W / S", "Contrast + / −"],
  ["E / D", "Saturation + / −"],
  ["1 2 · 3 4 · 5 6", "Cyan · Magenta · Yellow − / +"],
  ["G", "Color / grayscale"],
  ["R", "Recalc"],
  ["X", "Show clipping"],
  ["B", "Before / after"],
  ["⌘C / ⌘V", "Copy / paste settings"],
  ["⌘A", "Select all frames"],
  ["⌘O", "Add images"],
  ["⌘E", "Export"],
  ["⌫", "Remove selected frames"],
];

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return target.type !== "range" && target.type !== "checkbox";
  return target instanceof HTMLTextAreaElement || target.isContentEditable;
}

export function handleKeyDown(e: KeyboardEvent, actions: { addImages: () => void }) {
  if (isTyping(e.target)) return;
  const s = useStore.getState();
  if (s.exportOpen) {
    if (e.key === "Escape") s.setExportOpen(false);
    return;
  }
  const mod = e.metaKey || e.ctrlKey;
  const key = e.key.toLowerCase();

  if (mod) {
    const map: Record<string, () => void> = {
      c: s.copy,
      v: s.paste,
      a: s.selectAll,
      o: actions.addImages,
      e: () => s.frames.length && s.setExportOpen(true),
    };
    const fn = map[key];
    if (fn) {
      e.preventDefault();
      fn();
    }
    return;
  }
  if (e.altKey) return;

  const nudge = NUDGES[key];
  if (nudge) {
    e.preventDefault();
    s.nudge(nudge[0], nudge[1]);
    return;
  }
  switch (e.key) {
    case "ArrowRight":
    case "Enter":
      e.preventDefault();
      s.go(1);
      break;
    case "ArrowLeft":
      e.preventDefault();
      s.go(-1);
      break;
    case "Backspace":
    case "Delete":
      e.preventDefault();
      void s.removeSelection();
      break;
    default:
      switch (key) {
        case "x":
          s.toggleClipping();
          break;
        case "b":
          s.toggleNegative();
          break;
        case "g":
          s.toggleGrayscale();
          break;
        case "r":
          void s.recalcCurrent();
          break;
      }
  }
}
