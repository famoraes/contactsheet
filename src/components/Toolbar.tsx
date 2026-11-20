import { useState } from "react";
import { useStore, useVisibleFrames } from "../state/store";

export function Toolbar({ onAddImages }: { onAddImages: () => void }) {
  const frames = useVisibleFrames();
  const currentId = useStore((s) => s.currentId);
  const showClipping = useStore((s) => s.showClipping);
  const showBefore = useStore((s) => s.showBefore);
  const progress = useStore((s) => s.exportProgress);
  const { go, goTo, toggleClipping, toggleBefore, setExportOpen } = useStore.getState();
  const [editing, setEditing] = useState(false);

  const index = frames.findIndex((f) => f.id === currentId);
  const has = frames.length > 0;

  return (
    <header className="toolbar">
      <div className="toolbar-group">
        <span className="brand">ContactSheet</span>
        <button className="btn btn-primary" onClick={onAddImages} title="Add TIFF scans (⌘O)">
          + Add Images
        </button>
      </div>

      <div className="toolbar-group">
        <button className="btn btn-icon" disabled={index <= 0} onClick={() => go(-1)} title="Previous (←)">
          ‹
        </button>
        {editing ? (
          <input
            className="counter-input"
            autoFocus
            defaultValue={index + 1}
            onBlur={() => setEditing(false)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const n = parseInt(e.currentTarget.value, 10);
                if (!Number.isNaN(n)) goTo(n - 1);
                setEditing(false);
              } else if (e.key === "Escape") setEditing(false);
            }}
          />
        ) : (
          <button className="counter" disabled={!has} onClick={() => setEditing(true)} title="Jump to frame">
            {has ? `${index + 1} / ${frames.length}` : "– / –"}
          </button>
        )}
        <button
          className="btn btn-icon"
          disabled={index < 0 || index >= frames.length - 1}
          onClick={() => go(1)}
          title="Next (→ / Enter)"
        >
          ›
        </button>
      </div>

      <div className="toolbar-group">
        <button className={`btn btn-toggle${showClipping ? " is-on" : ""}`} disabled={!has} onClick={toggleClipping}>
          Clipping <kbd>X</kbd>
        </button>
        <button className={`btn btn-toggle${showBefore ? " is-on" : ""}`} disabled={!has} onClick={toggleBefore}>
          Before / After <kbd>B</kbd>
        </button>
        {progress && progress.done < progress.total ? (
          <div className="progress" title={progress.name}>
            <div className="progress-bar" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
            <span>
              Exporting {progress.done}/{progress.total}
            </span>
          </div>
        ) : (
          <button className="btn btn-primary" disabled={!has} onClick={() => setExportOpen(true)} title="Export (⌘E)">
            Export
          </button>
        )}
      </div>
    </header>
  );
}
