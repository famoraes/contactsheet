import { useState } from "react";
import { ASPECTS } from "../lib/crop";
import { SHORTCUT_HELP } from "../lib/shortcuts";
import { useCurrentFrame, useStore } from "../state/store";
import { HistogramView } from "./HistogramView";
import { InfoPanel } from "./InfoPanel";
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
  const neutralMode = useStore((st) => st.neutralMode);
  const cropMode = useStore((st) => st.cropMode);
  const cropAspect = useStore((st) => st.cropAspect);
  const cropPortrait = useStore((st) => st.cropPortrait);
  const tab = useStore((st) => st.panelTab);
  const s = useStore.getState();
  const [showKeys, setShowKeys] = useState(false);
  const disabled = !frame;
  const adj = frame?.adjustments;
  const mono = adj?.grayscale ?? false;

  return (
    <aside className="panel">
      <div className="panel-tabs segmented">
        <button className={tab === "adjust" ? "is-on" : ""} onClick={() => s.setPanelTab("adjust")}>
          Adjust
        </button>
        <button className={tab === "info" ? "is-on" : ""} onClick={() => s.setPanelTab("info")}>
          Info <kbd>I</kbd>
        </button>
      </div>

      {tab === "info" ? (
        <InfoPanel />
      ) : (
        <>
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
            <div className="btn-row btn-row-top">
              <button
                className={`btn btn-toggle${neutralMode ? " is-on" : ""}`}
                disabled={disabled}
                onClick={() => s.toggleNeutralMode()}
                title="Click something that should be gray. Shift-click a second gray in a different tone (e.g. a shadow and a highlight) to also fix casts that differ between shadows and highlights."
              >
                <Icon d="m2 22 1-1h3l9-9M3 21v-3l9-9m3-3 3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3l.4.4Z" /> Neutral <kbd>N</kbd>
              </button>
              <button
                className="btn"
                disabled={disabled}
                onClick={() => void s.recalcCurrent()}
                title="Recompute the base correction from the image area (excluding borders)"
              >
                Recalc <kbd>R</kbd>
              </button>
            </div>
            {neutralMode && (
              <p className="panel-hint">
                Click a gray. <b>⇧-click</b> a second gray in another tone to fix shadow/highlight casts too.
                {frame && frame.neutralPoints.length > 0 && <> Using {frame.neutralPoints.length} point{frame.neutralPoints.length > 1 ? "s" : ""}.</>}
              </p>
            )}
          </section>

          <section className="panel-section">
            <h3>Crop &amp; Rotate</h3>
            <div className="btn-row">
              <button
                className={`btn btn-toggle${cropMode ? " is-on" : ""}`}
                disabled={disabled}
                onClick={() => (cropMode ? void s.commitCrop() : s.enterCrop())}
                title="Crop and straighten (C). Enter applies, Esc cancels."
              >
                <Icon d="M6 2v14a2 2 0 0 0 2 2h14M18 22V8a2 2 0 0 0-2-2H2" /> {cropMode ? "Done" : "Crop"} <kbd>C</kbd>
              </button>
              <button
                className="btn"
                disabled={disabled}
                onClick={() => void s.autoCrop()}
                title="Find the frame edges and crop away holder, rebate and sprocket holes (⇧C). Applies to all selected frames."
              >
                Auto{!cropMode && selectionCount > 1 ? ` → ${selectionCount}` : ""}
              </button>
              <button
                className="btn"
                disabled={disabled || (!cropMode && !frame?.crop)}
                onClick={s.resetCrop}
                title="Remove crop and straightening"
              >
                Reset
              </button>
            </div>
            <div className="aspect-row">
              <select
                value={cropAspect}
                disabled={disabled}
                onChange={(e) => {
                  s.setCropAspect(e.target.value);
                  e.currentTarget.blur();
                }}
                title="Aspect ratio lock"
              >
                {ASPECTS.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.label}
                  </option>
                ))}
              </select>
              <button
                className="tool tool-small"
                disabled={disabled || cropAspect === "free" || cropAspect === "1:1"}
                onClick={() => s.setCropAspect(cropAspect, !cropPortrait)}
                title={cropPortrait ? "Portrait - switch to landscape" : "Landscape - switch to portrait"}
              >
                <Icon d={cropPortrait ? "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z" : "M3 8a1 1 0 0 1 1-1h16a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8Z"} />
              </button>
            </div>
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
        </>
      )}

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
