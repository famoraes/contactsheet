import { useEffect, useRef, useState } from "react";
import { renderPreview, type PreviewFrame } from "../lib/api";
import type { Adjustments, Analysis, ViewOptions } from "../lib/types";
import { useCurrentFrame, useStore, type Histogram } from "../state/store";

interface Request {
  id: number;
  adjustments: Adjustments;
  analysis: Analysis;
  view: ViewOptions;
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
  const setHistogram = useStore((s) => s.setHistogram);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const pending = useRef<Request | null>(null);
  const busy = useRef(false);
  const [renderMs, setRenderMs] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

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
          const img = await renderPreview(req.id, req.adjustments, req.analysis, req.view);
          const canvas = canvasRef.current;
          if (!canvas) continue;
          if (canvas.width !== img.width || canvas.height !== img.height) {
            canvas.width = img.width;
            canvas.height = img.height;
          }
          canvas.getContext("2d")!.putImageData(new ImageData(img.pixels, img.width, img.height), 0, 0);
          if (!req.view.showNegative && !req.view.showClipping) setHistogram(computeHistogram(img));
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

  useEffect(() => {
    if (!frame) return;
    pending.current = {
      id: frame.id,
      adjustments: frame.adjustments,
      analysis: frame.analysis,
      view: { showClipping, showNegative },
    };
    void pump();
  }, [frame?.id, frame?.adjustments, frame?.analysis, showClipping, showNegative]);

  if (!frame) return <div className="viewer" />;

  return (
    <div className="viewer">
      <canvas ref={canvasRef} className="viewer-canvas" />
      <div className="viewer-badges">
        {showNegative && <span className="badge badge-accent">Before</span>}
        {showClipping && (
          <span className="badge">
            Clipping <i className="swatch swatch-hi" /> highlights <i className="swatch swatch-lo" /> shadows
          </span>
        )}
      </div>
      <div className="viewer-info">
        {frame.name} · {frame.width}×{frame.height} · {frame.bitDepth}-bit
        {renderMs != null && <> · {renderMs.toFixed(0)} ms</>}
      </div>
      {error && <div className="viewer-error">{error}</div>}
    </div>
  );
}
