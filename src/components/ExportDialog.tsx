import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useState } from "react";
import { exportImages } from "../lib/api";
import type { ExportProgress, ExportSummary } from "../lib/types";
import { useStore } from "../state/store";

type Scope = "selection" | "all";

export function ExportDialog() {
  const frames = useStore((s) => s.frames);
  const selection = useStore((s) => s.selection);
  const { setExportOpen, setExportProgress } = useStore.getState();
  const [scope, setScope] = useState<Scope>(selection.length > 1 ? "selection" : "all");
  const [tiff, setTiff] = useState(true);
  const [jpeg, setJpeg] = useState(false);
  const [quality, setQuality] = useState(92);
  const [resize, setResize] = useState(false);
  const [maxSize, setMaxSize] = useState(3000);
  const [outDir, setOutDir] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ExportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const targets = scope === "all" ? frames : frames.filter((f) => selection.includes(f.id));

  const pickDir = async () => {
    const dir = await open({ directory: true, title: "Export folder" });
    if (typeof dir === "string") setOutDir(dir);
  };

  const run = async () => {
    if (!outDir) return;
    setRunning(true);
    setError(null);
    setResult(null);
    setExportProgress({ done: 0, total: targets.length, name: "", error: null });
    const unlisten = await listen<ExportProgress>("export-progress", (e) => setExportProgress(e.payload));
    try {
      const summary = await exportImages(
        targets.map((f) => ({
          id: f.id,
          adjustments: f.adjustments,
          analysis: f.analysis,
          orientation: f.orientation,
          crop: f.crop,
        })),
        { outDir, tiff, jpeg, jpegQuality: quality, maxSize: resize ? maxSize : null },
      );
      setResult(summary);
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten();
      setExportProgress(null);
      setRunning(false);
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={() => !running && setExportOpen(false)}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h2>Export</h2>

        <div className="field">
          <label>Frames</label>
          <div className="segmented">
            <button className={scope === "selection" ? "is-on" : ""} onClick={() => setScope("selection")}>
              Selection ({selection.length})
            </button>
            <button className={scope === "all" ? "is-on" : ""} onClick={() => setScope("all")}>
              All ({frames.length})
            </button>
          </div>
        </div>

        <div className="field">
          <label>Format</label>
          <label className="check">
            <input type="checkbox" checked={tiff} onChange={(e) => setTiff(e.target.checked)} />
            TIFF · 16-bit sRGB, uncompressed
          </label>
          <label className="check">
            <input type="checkbox" checked={jpeg} onChange={(e) => setJpeg(e.target.checked)} />
            JPEG · quality
            <input
              type="number"
              min={50}
              max={100}
              value={quality}
              disabled={!jpeg}
              onChange={(e) => setQuality(Math.max(50, Math.min(100, Number(e.target.value) || 92)))}
            />
          </label>
        </div>

        <div className="field">
          <label>Size</label>
          <label className="check">
            <input type="checkbox" checked={resize} onChange={(e) => setResize(e.target.checked)} />
            Downscale long edge to
            <input
              type="number"
              min={256}
              step={100}
              value={maxSize}
              disabled={!resize}
              onChange={(e) => setMaxSize(Math.max(256, Number(e.target.value) || 3000))}
            />
            px
          </label>
        </div>

        <div className="field">
          <label>Folder</label>
          <div className="folder">
            <span className="folder-path" title={outDir ?? ""}>
              {outDir ?? "No folder chosen"}
            </span>
            <button className="btn" onClick={pickDir}>
              Choose…
            </button>
          </div>
        </div>

        {result && (
          <div className={`notice${result.errors.length ? " notice-warn" : ""}`}>
            Wrote {result.written.length} file{result.written.length === 1 ? "" : "s"}.
            {result.errors.map((e) => (
              <div key={e}>{e}</div>
            ))}
          </div>
        )}
        {error && <div className="notice notice-warn">{error}</div>}

        <div className="modal-actions">
          <button className="btn" disabled={running} onClick={() => setExportOpen(false)}>
            {result ? "Close" : "Cancel"}
          </button>
          <button
            className="btn btn-primary"
            disabled={running || !outDir || (!tiff && !jpeg) || targets.length === 0}
            onClick={run}
          >
            {running ? "Exporting…" : `Export ${targets.length} frame${targets.length === 1 ? "" : "s"}`}
          </button>
        </div>
      </div>
    </div>
  );
}
