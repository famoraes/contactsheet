import { useEffect, useRef, useState, type MouseEvent } from "react";
import { renderPreview, type PreviewFrame } from "../lib/api";
import { aspectRatio, cropPixels, FULL_CROP, MAX_ANGLE } from "../lib/crop";
import type { Adjustments, Analysis, Crop, Orientation, ViewOptions } from "../lib/types";
import { displaySize, useCurrentFrame, useStore, type Histogram } from "../state/store";
import { CropOverlay } from "./CropOverlay";

interface Request {
  id: number;
  adjustments: Adjustments;
  analysis: Analysis;
  orientation: Orientation;
  crop: Crop | null;
  view: ViewOptions;
}

interface Size {
  w: number;
  h: number;
}

function computeHistogram({ pixels }: PreviewFrame): Histogram {
  const r = new Uint32Array(256);
  const g = new Uint32Array(256);
  const b = new Uint32Array(256);
  for (let i = 0; i < pixels.length; i += 4) {
    r[pixels[i]]++;
    g[pixels[i + 1]]++;
    b[pixels[i + 2]]++;
  }
  return { r, g, b };
}

export function Viewer() {
  const frame = useCurrentFrame();
  const showClipping = useStore((s) => s.showClipping);
  const showNegative = useStore((s) => s.showNegative);
  const neutralMode = useStore((s) => s.neutralMode);
  const cropMode = useStore((s) => s.cropMode);
  const cropAspect = useStore((s) => s.cropAspect);
  const cropPortrait = useStore((s) => s.cropPortrait);
  const { setHistogram, pickNeutral, setCrop, setCropAngle, commitCrop, cancelCrop } = useStore.getState();
  const fitRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const pending = useRef<Request | null>(null);
  const busy = useRef(false);
  const [box, setBox] = useState<Size>({ w: 0, h: 0 });
  const [img, setImg] = useState<Size | null>(null);
  const [renderMs, setRenderMs] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const el = fitRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) =>
      setBox({ w: entry.contentRect.width, h: entry.contentRect.height }),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Latest-wins render loop: at most one request in flight, intermediate slider
  // positions are dropped so dragging never queues up stale renders.
  const pump = async () => {
    if (busy.current) return;
    busy.current = true;
    try {
      while (pending.current) {
        const req = pending.current;
        pending.current = null;
        const t0 = performance.now();
        try {
          const out = await renderPreview(req.id, req.adjustments, req.analysis, req.orientation, req.crop, req.view);
          const canvas = canvasRef.current;
          if (!canvas) continue;
          if (canvas.width !== out.width || canvas.height !== out.height) {
            canvas.width = out.width;
            canvas.height = out.height;
          }
          canvas.getContext("2d")!.putImageData(new ImageData(out.pixels, out.width, out.height), 0, 0);
          setImg((old) => (old?.w === out.width && old?.h === out.height ? old : { w: out.width, h: out.height }));
          if (!req.view.showNegative && !req.view.showClipping) setHistogram(computeHistogram(out));
          setRenderMs(performance.now() - t0);
          setError(null);
        } catch (e) {
          setError(String(e));
        }
      }
    } finally {
      busy.current = false;
    }
  };

  // While cropping, show the whole frame (straightened via CSS) under the crop box.
  const renderCrop = cropMode ? null : (frame?.crop ?? null);

  useEffect(() => {
    if (!frame) return;
    pending.current = {
      id: frame.id,
      adjustments: frame.adjustments,
      analysis: frame.analysis,
      orientation: frame.orientation,
      crop: renderCrop,
      view: { showClipping, showNegative },
    };
    void pump();
  }, [frame?.id, frame?.adjustments, frame?.analysis, frame?.orientation, renderCrop, showClipping, showNegative]);

  // Fit the image into the available box (like object-fit: contain), but with an
  // explicit size so clicks and overlays map 1:1 onto image coordinates.
  const scale = img && box.w > 0 ? Math.min(box.w / img.w, box.h / img.h) : 0;
  const display = img ? { width: img.w * scale, height: img.h * scale } : { width: 0, height: 0 };

  const full = frame ? displaySize(frame) : null;
  const crop = frame?.crop ?? FULL_CROP;
  const ratio = full ? aspectRatio(cropAspect, cropPortrait, full) : null;

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    if (!neutralMode || cropMode || !frame) return;
    const r = e.currentTarget.getBoundingClientRect();
    const u = (e.clientX - r.left) / r.width;
    const v = (e.clientY - r.top) / r.height;
    if (u >= 0 && u <= 1 && v >= 0 && v <= 1) void pickNeutral(u, v, e.shiftKey);
  };

  return (
    <div className="viewer">
      <div className="viewer-fit" ref={fitRef}>
        <div
          className={`viewer-image${neutralMode ? " is-picking" : ""}${cropMode ? " is-cropping" : ""}`}
          style={display}
          onClick={onClick}
        >
          <canvas
            ref={canvasRef}
            className="viewer-canvas"
            style={cropMode && crop.angle ? { transform: `rotate(${crop.angle}deg)` } : undefined}
          />
          {cropMode && full && <CropOverlay crop={crop} size={full} ratio={ratio} onChange={setCrop} />}
          {neutralMode &&
            frame?.neutralPoints.map(([u, v], i) => (
              <span key={i} className="neutral-marker" style={{ left: `${u * 100}%`, top: `${v * 100}%` }}>
                {i + 1}
              </span>
            ))}
        </div>
      </div>
      <div className="viewer-badges">
        {neutralMode && (
          <span className="badge badge-accent">
            Neutral picker: click a gray · ⇧-click a second gray in another tone · Esc to exit
          </span>
        )}
        {showNegative && <span className="badge badge-accent">Before</span>}
        {showClipping && (
          <span className="badge">
            Clipping <i className="swatch swatch-hi" /> highlights <i className="swatch swatch-lo" /> shadows
          </span>
        )}
      </div>
      {cropMode && frame && full && (
        <div className="crop-bar">
          <label>Straighten</label>
          <input
            type="range"
            min={-MAX_ANGLE}
            max={MAX_ANGLE}
            step={0.05}
            value={crop.angle}
            onChange={(e) => setCropAngle(parseFloat(e.target.value))}
            onDoubleClick={() => setCropAngle(0)}
            onPointerUp={(e) => e.currentTarget.blur()}
          />
          <span className="crop-angle">
            {crop.angle > 0 ? "+" : ""}
            {crop.angle.toFixed(2)}°
          </span>
          <span className="crop-dims">
            {cropPixels(crop, full).w} × {cropPixels(crop, full).h}
          </span>
          <button className="btn" onClick={cancelCrop}>
            Cancel <kbd>Esc</kbd>
          </button>
          <button className="btn btn-primary" onClick={() => void commitCrop()}>
            Done <kbd>↵</kbd>
          </button>
        </div>
      )}
      {frame && full && !cropMode && (
        <div className="viewer-info">
          {frame.name} · {full.w}×{full.h}
          {frame.crop && (
            <>
              {" "}
              → crop {cropPixels(frame.crop, full).w}×{cropPixels(frame.crop, full).h}
            </>
          )}{" "}
          · {frame.bitDepth}-bit
          {renderMs != null && <> · {renderMs.toFixed(0)} ms</>}
        </div>
      )}
      {error && <div className="viewer-error">{error}</div>}
    </div>
  );
}
