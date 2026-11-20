import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { ExportDialog } from "./components/ExportDialog";
import { Filmstrip } from "./components/Filmstrip";
import { RightPanel } from "./components/RightPanel";
import { Toolbar } from "./components/Toolbar";
import { Viewer } from "./components/Viewer";
import { flushSaves } from "./lib/roll";
import { handleKeyDown } from "./lib/shortcuts";
import { useStore } from "./state/store";

export default function App() {
  const hasFrames = useStore((s) => s.frames.length > 0 || s.pendingLoads > 0);
  const exportOpen = useStore((s) => s.exportOpen);
  const errors = useStore((s) => s.errors);
  const panelsHidden = useStore((s) => s.panelsHidden);
  const { loadPaths, dismissErrors } = useStore.getState();
  const [dragging, setDragging] = useState(false);

  const addImages = useCallback(async () => {
    const picked = await open({
      multiple: true,
      directory: false,
      title: "Add negative scans",
      filters: [{ name: "TIFF scans", extensions: ["tif", "tiff", "TIF", "TIFF"] }],
    });
    if (Array.isArray(picked) && picked.length) void loadPaths(picked);
  }, [loadPaths]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => handleKeyDown(e, { addImages });
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [addImages]);

  // Write pending roll-file changes before the window goes away.
  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async () => {
      await flushSaves().catch(() => undefined);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        void loadPaths(p.paths);
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [loadPaths]);

  return (
    <div className={`app${panelsHidden ? " is-bare" : ""}`}>
      <Toolbar onAddImages={addImages} />
      <main className="main">
        <div className="stage">
          {hasFrames ? (
            <Viewer />
          ) : (
            <div className="empty" onClick={addImages}>
              <div className="empty-frame">
                <div className="empty-title">Drop negative scans here</div>
                <div className="empty-sub">16-bit or 8-bit TIFF · color or B&amp;W · or click to browse</div>
              </div>
            </div>
          )}
          {dragging && (
            <div className="drop-overlay">
              <div>Release to add scans</div>
            </div>
          )}
        </div>
        <RightPanel />
      </main>
      <Filmstrip />
      {exportOpen && <ExportDialog />}
      {errors.length > 0 && (
        <div className="toast" onClick={dismissErrors} title="Click to dismiss">
          {errors.slice(-3).map((e, i) => (
            <div key={i}>{e}</div>
          ))}
        </div>
      )}
    </div>
  );
}
