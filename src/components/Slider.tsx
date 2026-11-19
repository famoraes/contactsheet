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
}

/** Bipolar -100..100 slider; double-click to reset. */
export function Slider({ label, value, onChange, disabled, hint, track, shortcut }: Props) {
  const display = Math.round(value * 100);
  return (
    <div className={`slider${disabled ? " is-disabled" : ""}`} title={hint}>
      <div className="slider-head" onDoubleClick={() => !disabled && onChange(0)}>
        <span className="slider-label">
          {label}
          {shortcut && <kbd>{shortcut}</kbd>}
        </span>
        <span className={`slider-value${display !== 0 ? " is-changed" : ""}`}>
          {display > 0 ? `+${display}` : display}
        </span>
      </div>
      <input
        type="range"
        min={-1}
        max={1}
        step={0.01}
        value={value}
        disabled={disabled}
        style={{ "--track": track ?? "var(--track-default)", "--fill": `${(value + 1) * 50}%` } as CSSProperties}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        onDoubleClick={() => onChange(0)}
        onPointerUp={(e) => e.currentTarget.blur()}
      />
    </div>
  );
}
