import { useState } from "react";
import { SHORTCUT_HELP } from "../lib/shortcuts";
import { useCurrentFrame, useStore } from "../state/store";
import { HistogramView } from "./HistogramView";
import { Slider } from "./Slider";

function Icon({ d }: { d: string }) {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d={d} />
    </svg>
  );
}

export function RightPanel() {
  const frame = useCurrentFrame();
  const hasClipboard = useStore((st) => st.clipboard != null);
  const selectionCount = useStore((st) => st.selection.length);
  const s = useStore.getState();
  const [showKeys, setShowKeys] = useState(false);
  const disabled = !frame;
  const adj = frame?.adjustments;
  const mono = adj?.grayscale ?? false;

  return (
    <aside className="panel">
      <section className="panel-section">
        <HistogramView />
      </section>

      <section className="panel-section">
        <div className="segmented">
          <button className={!mono ? "is-on" : ""} disabled={disabled} onClick={() => mono && s.toggleGrayscale()}>
            Color
          </button>
          <button className={mono ? "is-on" : ""} disabled={disabled} onClick={() => !mono && s.toggleGrayscale()}>
            Grayscale
          </button>
        </div>
      </section>

      <section className="panel-section">
        <h3>Tone</h3>
        <Slider
          label="Density"
          shortcut="Q/A"
          hint="Print density: + darker, − lighter"
          value={adj?.density ?? 0}
          disabled={disabled}
          track="linear-gradient(90deg, #ddd, #222)"
          onChange={(v) => s.setAdjustment("density", v)}
        />
        <Slider
          label="Contrast"
          shortcut="W/S"
          value={adj?.contrast ?? 0}
          disabled={disabled}
          track="linear-gradient(90deg, #777, #888 50%, #000 50.5%, #fff)"
          onChange={(v) => s.setAdjustment("contrast", v)}
        />
        <Slider
          label="Saturation"
          shortcut="E/D"
          value={adj?.saturation ?? 0}
          disabled={disabled || mono}
          track="linear-gradient(90deg, #888, #c44, #ca4, #4a4, #48c, #a4c)"
          onChange={(v) => s.setAdjustment("saturation", v)}
        />
      </section>

      <section className="panel-section">
        <h3>Color</h3>
        <Slider
          label="Cyan"
          shortcut="1/2"
          value={adj?.cyan ?? 0}
          disabled={disabled || mono}
          track="linear-gradient(90deg, #e0524a, #2bc3d8)"
          onChange={(v) => s.setAdjustment("cyan", v)}
        />
        <Slider
          label="Magenta"
          shortcut="3/4"
          value={adj?.magenta ?? 0}
          disabled={disabled || mono}
          track="linear-gradient(90deg, #4fc25a, #d2479f)"
          onChange={(v) => s.setAdjustment("magenta", v)}
        />
        <Slider
          label="Yellow"
          shortcut="5/6"
          value={adj?.yellow ?? 0}
          disabled={disabled || mono}
          track="linear-gradient(90deg, #4a73e0, #e3c534)"
          onChange={(v) => s.setAdjustment("yellow", v)}
        />
        <button
          className="btn btn-block"
          disabled={disabled}
          onClick={() => void s.recalcCurrent()}
          title="Recompute the base correction from the image area (excluding borders)"
        >
          Recalc <kbd>R</kbd>
        </button>
      </section>

      <section className="panel-section">
        <h3>Transform</h3>
        <div className="tool-row">
          <button className="tool" disabled={disabled} onClick={() => s.rotate(-1)} title="Rotate left ( [ )">
            <Icon d="M7.5 6.5H3.5V2.5M3.8 6.3A7 7 0 1 1 3 12" />
          </button>
          <button className="tool" disabled={disabled} onClick={() => s.rotate(1)} title="Rotate right ( ] )">
            <Icon d="M16.5 6.5h4V2.5M20.2 6.3A7 7 0 1 0 21 12" />
          </button>
          <button className="tool" disabled={disabled} onClick={() => s.flip("horizontal")} title="Flip horizontal (H)">
            <Icon d="M12 3v18M9 7 4 12l5 5V7ZM15 7l5 5-5 5V7Z" />
          </button>
          <button className="tool" disabled={disabled} onClick={() => s.flip("vertical")} title="Flip vertical (V)">
            <Icon d="M3 12h18M7 9l5-5 5 5H7ZM7 15l5 5 5-5H7Z" />
          </button>
        </div>
      </section>

      <section className="panel-section">
        <h3>Settings</h3>
        <div className="btn-row">
          <button className="btn" disabled={disabled} onClick={s.copy} title="Copy settings (⌘C)">
            Copy
          </button>
          <button
            className="btn"
            disabled={disabled || !hasClipboard}
            onClick={s.paste}
            title="Paste settings to selected frames (⌘V)"
          >
            Paste{selectionCount > 1 ? ` → ${selectionCount}` : ""}
          </button>
          <button className="btn" disabled={disabled} onClick={s.resetCurrent} title="Reset to automatic conversion">
            Reset
          </button>
        </div>
      </section>

      <div className="panel-spacer" />

      <section className="panel-section panel-keys">
        <button className="link" onClick={() => setShowKeys((v) => !v)}>
          {showKeys ? "Hide" : "Show"} keyboard shortcuts
        </button>
        {showKeys && (
          <dl className="keys">
            {SHORTCUT_HELP.map(([k, d]) => (
              <div key={k}>
                <dt>{k}</dt>
                <dd>{d}</dd>
              </div>
            ))}
          </dl>
        )}
      </section>
    </aside>
  );
}
