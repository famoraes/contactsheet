import { useStore } from "../state/store";
import type { SliderKey } from "./types";

/** One slider unit (sliders display -100..100). Shift multiplies by BIG_STEP. */
const STEP = 0.01;
const BIG_STEP = 5;

/**
 * Single-key adjustments, SmartConvert-style: everything reachable without the mouse.
 * Keyed by `KeyboardEvent.code` so Shift+digit still matches (e.key would be "!" etc.).
 */
const NUDGES: Record<string, [SliderKey, number]> = {
  KeyQ: ["density", 1],
  KeyA: ["density", -1],
  KeyW: ["contrast", 1],
  KeyS: ["contrast", -1],
  KeyE: ["saturation", 1],
  KeyD: ["saturation", -1],
  Digit1: ["cyan", -1],
  Digit2: ["cyan", 1],
  Digit3: ["magenta", -1],
  Digit4: ["magenta", 1],
  Digit5: ["yellow", -1],
  Digit6: ["yellow", 1],
};

export const SHORTCUT_HELP: [string, string][] = [
  ["← / → / Enter", "Previous / next frame"],
  ["Q / A", "Density + / −"],
  ["W / S", "Contrast + / −"],
  ["E / D", "Saturation + / −"],
  ["1 2 · 3 4 · 5 6", "Cyan · Magenta · Yellow − / +"],
  ["⇧ + adjust key", "Step by 5 instead of 1"],
  ["[ / ]", "Rotate left / right"],
  ["H / V", "Flip horizontal / vertical"],
  ["G", "Color / grayscale"],
  ["N", "Neutral picker (⇧-click: 2nd point)"],
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
  if (e.key === "Escape" && s.neutralMode) {
    s.toggleNeutralMode(false);
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

  const nudge = NUDGES[e.code];
  if (nudge) {
    e.preventDefault();
    s.nudge(nudge[0], nudge[1] * STEP * (e.shiftKey ? BIG_STEP : 1));
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
        case "[":
          s.rotate(-1);
          break;
        case "]":
          s.rotate(1);
          break;
        case "h":
          s.flip("horizontal");
          break;
        case "v":
          s.flip("vertical");
          break;
        case "r":
          void s.recalcCurrent();
          break;
        case "n":
          s.toggleNeutralMode();
          break;
      }
  }
}
