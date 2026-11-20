import { useEffect, useRef, useState, type MouseEvent, type PointerEvent, type WheelEvent } from "react";
import { renderPreview, renderRegion, type PreviewFrame } from "../lib/api";
import { aspectRatio, cropPixels, FULL_CROP, MAX_ANGLE } from "../lib/crop";
import { DEFAULT_ADJUSTMENTS, type Adjustments, type Analysis, type Crop, type Orientation, type ViewOptions } from "../lib/types";
import { displaySize, useCurrentFrame, useStore, type Histogram } from "../state/store";
import { CropOverlay } from "./CropOverlay";

interface Request {
  id: number;
  adjustments: Adjustments;
  analysis: Analysis;
  orientation: Orientation;
  crop: Crop | null;
  view: ViewOptions;
  /** Rendering the "before" state; it must not replace the histogram. */
  before: boolean;
}

interface TileRequest extends Request {
  key: string;
  rect: { x: number; y: number; w: number; h: number };
}

interface Tile {
  key: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

interface Size {
  w: number;
  h: number;
}

/** Extra full-resolution pixels rendered around the visible area, so small pans need no new tile. */
const TILE_MARGIN = 160;
/** Pointer travel (CSS px) below which a press counts as a click, not a pan. */
const CLICK_SLOP = 3;

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

/** Keep the view centre so the image covers the viewport (or stays centred when smaller). */
const clampCenter = (c: number, img: number, view: number) =>
  img <= view ? 0.5 : Math.min(Math.max(c, view / 2 / img), 1 - view / 2 / img);

export function Viewer() {
  const frame = useCurrentFrame();
  const showClipping = useStore((s) => s.showClipping);
  const showNegative = useStore((s) => s.showNegative);
  const showBefore = useStore((s) => s.showBefore);
  const neutralMode = useStore((s) => s.neutralMode);
  const cropMode = useStore((s) => s.cropMode);
  const cropAspect = useStore((s) => s.cropAspect);
  const cropPortrait = useStore((s) => s.cropPortrait);
  const zoom = useStore((s) => s.zoomCenter);
  const { setHistogram, pickNeutral, setCrop, setCropAngle, commitCrop, cancelCrop, autoCrop, setZoom } =
    useStore.getState();
  const viewerRef = useRef<HTMLDivElement>(null);
  const fitRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const tileRef = useRef<HTMLCanvasElement>(null);
  const pending = useRef<Request | null>(null);
  const busy = useRef(false);
  const pendingTile = useRef<TileRequest | null>(null);
  const busyTile = useRef(false);
  const suppressClick = useRef(false);
  const [box, setBox] = useState<Size>({ w: 0, h: 0 });
  const [img, setImg] = useState<Size | null>(null);
  const [tile, setTile] = useState<Tile | null>(null);
  const [renderMs, setRenderMs] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [panning, setPanning] = useState(false);

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
          if (!req.before && !req.view.showNegative && !req.view.showClipping) setHistogram(computeHistogram(out));
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

  // Same pattern for full-resolution tiles at 100% zoom.
  const pumpTile = async () => {
    if (busyTile.current) return;
    busyTile.current = true;
    try {
      while (pendingTile.current) {
        const req = pendingTile.current;
        pendingTile.current = null;
        try {
          const out = await renderRegion(
            req.id,
            req.adjustments,
            req.analysis,
            req.orientation,
            req.crop,
            req.view,
            req.rect,
          );
          const canvas = tileRef.current;
          if (!canvas || pendingTile.current) continue; // superseded: skip drawing
          canvas.width = out.width;
          canvas.height = out.height;
          canvas.getContext("2d")!.putImageData(new ImageData(out.pixels, out.width, out.height), 0, 0);
          setTile({ key: req.key, x: req.rect.x, y: req.rect.y, w: out.width, h: out.height });
        } catch (e) {
          setError(String(e));
        }
      }
    } finally {
      busyTile.current = false;
    }
  };

  // While cropping, show the whole frame (straightened via CSS) under the crop box.
  const renderCrop = cropMode ? null : (frame?.crop ?? null);
  // "Before" is the automatic conversion with no corrections: auto analysis (no
  // neutral picks) and default sliders. Geometry, color/grayscale and the look
  // stay, so the comparison lines up and only the corrections toggle.
  const before = showBefore && !showNegative && !cropMode;
  const request: Request | null = frame && {
    id: frame.id,
    adjustments: before
      ? { ...DEFAULT_ADJUSTMENTS, grayscale: frame.adjustments.grayscale, look: frame.adjustments.look }
      : frame.adjustments,
    analysis: before ? frame.autoAnalysis : frame.analysis,
    orientation: frame.orientation,
    crop: renderCrop,
    view: { showClipping, showNegative },
    before,
  };
  const key = request ? JSON.stringify(request) : "";

  useEffect(() => {
    if (!request) return;
    pending.current = request;
    void pump();
  }, [key]);

  // ---- Geometry -------------------------------------------------------------

  const full = frame ? displaySize(frame) : null;
  const crop = frame?.crop ?? FULL_CROP;
  const ratio = full ? aspectRatio(cropAspect, cropPortrait, full) : null;
  /** Full-resolution size of the frame as rendered (after crop). */
  const out = full ? cropPixels(renderCrop ?? FULL_CROP, full) : null;
  const zoomed = !!zoom && !cropMode && !!out;
  const dpr = window.devicePixelRatio || 1;
  // 100% = one full-resolution pixel per screen (device) pixel.
  const cssPerPx = 1 / dpr;
  const zoomedSize = out ? { w: out.w * cssPerPx, h: out.h * cssPerPx } : { w: 0, h: 0 };

  let display: { left?: number; top?: number; width: number; height: number };
  if (zoomed) {
    const cx = clampCenter(zoom[0], zoomedSize.w, box.w);
    const cy = clampCenter(zoom[1], zoomedSize.h, box.h);
    // Snap to whole device pixels so the 100% tile isn't resampled.
    const snap = (v: number) => Math.round(v * dpr) / dpr;
    display = {
      left: snap(box.w / 2 - cx * zoomedSize.w),
      top: snap(box.h / 2 - cy * zoomedSize.h),
      width: zoomedSize.w,
      height: zoomedSize.h,
    };
  } else {
    // Fit like object-fit: contain, with an explicit size so clicks and overlays
    // map 1:1 onto image coordinates.
    const scale = img && box.w > 0 ? Math.min(box.w / img.w, box.h / img.h) : 0;
    display = img ? { width: img.w * scale, height: img.h * scale } : { width: 0, height: 0 };
  }

  // The visible region (plus a margin) in full-resolution pixels.
  let tileRect: TileRequest["rect"] | null = null;
  if (zoomed && out && display.left !== undefined && display.top !== undefined) {
    const x0 = Math.max(0, Math.floor(-display.left / cssPerPx) - TILE_MARGIN);
    const y0 = Math.max(0, Math.floor(-display.top / cssPerPx) - TILE_MARGIN);
    const x1 = Math.min(out.w, Math.ceil((box.w - display.left) / cssPerPx) + TILE_MARGIN);
    const y1 = Math.min(out.h, Math.ceil((box.h - display.top) / cssPerPx) + TILE_MARGIN);
    tileRect = { x: x0, y: y0, w: Math.max(1, x1 - x0), h: Math.max(1, y1 - y0) };
  }
  // The current tile still covers what is visible (the margin absorbs small pans).
  const tileCovers =
    !!tile &&
    !!tileRect &&
    tile.key === key &&
    tile.x <= tileRect.x + TILE_MARGIN &&
    tile.y <= tileRect.y + TILE_MARGIN &&
    tile.x + tile.w >= tileRect.x + tileRect.w - TILE_MARGIN &&
    tile.y + tile.h >= tileRect.y + tileRect.h - TILE_MARGIN;

  useEffect(() => {
    if (!request || !tileRect || tileCovers) return;
    pendingTile.current = { ...request, key, rect: tileRect };
    void pumpTile();
  }, [zoomed, key, tileRect?.x, tileRect?.y, tileRect?.w, tileRect?.h, tileCovers]);

  // ---- Interaction ----------------------------------------------------------

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    if (!neutralMode || cropMode || !frame) return;
    const r = e.currentTarget.getBoundingClientRect();
    const u = (e.clientX - r.left) / r.width;
    const v = (e.clientY - r.top) / r.height;
    if (u >= 0 && u <= 1 && v >= 0 && v <= 1) void pickNeutral(u, v, e.shiftKey);
  };

  // Double-click: zoom to 100% keeping the clicked point under the pointer, or back to fit.
  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    if (cropMode || neutralMode || !out || !viewerRef.current) return;
    if (zoomed) {
      setZoom(null);
      return;
    }
    const r = e.currentTarget.getBoundingClientRect();
    const [u, v] = [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height];
    // The zoomed view fills the whole viewer, so place (u, v) relative to it.
    const vr = viewerRef.current.getBoundingClientRect();
    const [px, py] = [e.clientX - vr.left, e.clientY - vr.top];
    setZoom([
      (vr.width / 2 - (px - u * zoomedSize.w)) / zoomedSize.w,
      (vr.height / 2 - (py - v * zoomedSize.h)) / zoomedSize.h,
    ]);
  };

  const panTo = (cx: number, cy: number) =>
    setZoom([clampCenter(cx, zoomedSize.w, box.w), clampCenter(cy, zoomedSize.h, box.h)]);

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!zoomed || e.button !== 0) return;
    const [sx, sy] = [e.clientX, e.clientY];
    const [cx, cy] = [clampCenter(zoom[0], zoomedSize.w, box.w), clampCenter(zoom[1], zoomedSize.h, box.h)];
    let moved = false;
    const move = (ev: globalThis.PointerEvent) => {
      const [dx, dy] = [ev.clientX - sx, ev.clientY - sy];
      if (!moved && Math.hypot(dx, dy) < CLICK_SLOP) return;
      moved = true;
      setPanning(true);
      panTo(cx - dx / zoomedSize.w, cy - dy / zoomedSize.h);
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      setPanning(false);
      // The click that ends a drag must not pick a neutral point.
      if (moved) suppressClick.current = true;
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };

  // Trackpad / wheel scrolling pans when zoomed.
  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (!zoomed || e.ctrlKey) return;
    const [cx, cy] = [clampCenter(zoom[0], zoomedSize.w, box.w), clampCenter(zoom[1], zoomedSize.h, box.h)];
    panTo(cx + e.deltaX / zoomedSize.w, cy + e.deltaY / zoomedSize.h);
  };

  return (
    <div className="viewer" ref={viewerRef}>
      <div
        className={`viewer-fit${zoomed ? " is-zoomed" : ""}${panning ? " is-panning" : ""}`}
        ref={fitRef}
        onPointerDown={onPointerDown}
        onWheel={onWheel}
      >
        <div
          className={`viewer-image${neutralMode ? " is-picking" : ""}${cropMode ? " is-cropping" : ""}`}
          style={display}
          onClick={onClick}
          onDoubleClick={onDoubleClick}
        >
          <canvas
            ref={canvasRef}
            className="viewer-canvas"
            style={cropMode && crop.angle ? { transform: `rotate(${crop.angle}deg)` } : undefined}
          />
          <canvas
            ref={tileRef}
            className="viewer-tile"
            style={
              zoomed && tile && tile.key === key
                ? { left: tile.x * cssPerPx, top: tile.y * cssPerPx, width: tile.w * cssPerPx, height: tile.h * cssPerPx }
                : { display: "none" }
            }
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
        {zoomed && <span className="badge">100%{!tileCovers && <span className="badge-dim"> · rendering…</span>}</span>}
        {neutralMode && (
          <span className="badge badge-accent">
            Neutral picker: click a gray · ⇧-click a second gray in another tone · Esc to exit
          </span>
        )}
        {before && <span className="badge badge-accent">Before · auto conversion</span>}
        {showNegative && <span className="badge badge-accent">Negative</span>}
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
          <button className="btn" onClick={() => void autoCrop()} title="Find the frame edges (⇧C)">
            Auto
          </button>
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
