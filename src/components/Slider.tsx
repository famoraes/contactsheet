import type { CSSProperties } from "react";

interface Props {
  label: string;
  value: number;
  onChange: (v: number) => void;
  disabled?: boolean;
  hint?: string;
  /** CSS gradient for the track, e.g. cyan -> red for the cyan slider. */
  track?: string;
  shortcut?: string;
  /** Lower end (default -1, a bipolar slider); the upper end is always 1. */
  min?: number;
  /** Value restored by double-click (default 0). */
  resetTo?: number;
}

/** -100..100 slider (or 0..100 with `min={0}`); double-click to reset. */
export function Slider({ label, value, onChange, disabled, hint, track, shortcut, min = -1, resetTo = 0 }: Props) {
  const display = Math.round(value * 100);
  return (
    <div className={`slider${disabled ? " is-disabled" : ""}`} title={hint}>
      <div className="slider-head" onDoubleClick={() => !disabled && onChange(resetTo)}>
        <span className="slider-label">
          {label}
          {shortcut && <kbd>{shortcut}</kbd>}
        </span>
        <span className={`slider-value${display !== Math.round(resetTo * 100) ? " is-changed" : ""}`}>
          {display > 0 && min < 0 ? `+${display}` : display}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={1}
        step={0.01}
        value={value}
        disabled={disabled}
        style={{ "--track": track ?? "var(--track-default)", "--fill": `${((value - min) / (1 - min)) * 100}%` } as CSSProperties}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        onDoubleClick={() => onChange(resetTo)}
        onPointerUp={(e) => e.currentTarget.blur()}
      />
    </div>
  );
}
