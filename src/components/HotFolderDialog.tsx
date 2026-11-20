import { open } from "@tauri-apps/plugin-dialog";
import { baseOf } from "../lib/roll";
import { useStore } from "../state/store";

/** Watch a folder and add scans as the scanner writes them. */
export function HotFolderDialog() {
  const hotFolder = useStore((s) => s.hotFolder);
  const applySettings = useStore((s) => s.hotApplySettings);
  const autoCrop = useStore((s) => s.hotAutoCrop);
  const { setHotOpen, setHotOptions, startHotFolder, stopHotFolder } = useStore.getState();

  const pick = async () => {
    const dir = await open({ directory: true, title: "Folder the scanner saves to" });
    if (typeof dir === "string") await startHotFolder(dir);
  };

  return (
    <div className="modal-backdrop" onMouseDown={() => setHotOpen(false)}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h2>Hot folder</h2>
        <p className="panel-hint">
          New TIFFs saved to this folder are added as soon as the scanner finishes writing them, and shown right away.
          Files already in the folder are left alone.
        </p>

        <div className="field">
          <label>Folder</label>
          <div className="folder">
            <span className="folder-path" title={hotFolder ?? ""}>
              {hotFolder ?? "Not watching"}
            </span>
            <button className="btn" onClick={pick}>
              {hotFolder ? "Change…" : "Choose…"}
            </button>
          </div>
        </div>

        <div className="field">
          <label>New scans</label>
          <label className="check">
            <input type="checkbox" checked={applySettings} onChange={(e) => setHotOptions({ applySettings: e.target.checked })} />
            Use the settings of the frame on screen (and its base correction when shared)
          </label>
          <label className="check">
            <input type="checkbox" checked={autoCrop} onChange={(e) => setHotOptions({ autoCrop: e.target.checked })} />
            AutoCrop
          </label>
        </div>

        <div className="modal-actions">
          {hotFolder && (
            <button className="btn" onClick={() => void stopHotFolder()}>
              Stop watching {baseOf(hotFolder)}
            </button>
          )}
          <button className="btn btn-primary" onClick={() => setHotOpen(false)}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
