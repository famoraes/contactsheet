import { useEffect, useRef } from "react";
import { useStore } from "../state/store";

const W = 256;
const H = 90;

export function HistogramView() {
  const histogram = useStore((s) => s.histogram);
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const ctx = ref.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, W, H);
    if (!histogram) return;
    const channels: [Uint32Array, string][] = [
      [histogram.r, "rgba(255,80,70,0.85)"],
      [histogram.g, "rgba(90,220,110,0.85)"],
      [histogram.b, "rgba(80,140,255,0.85)"],
    ];
    // Ignore the extreme bins when scaling so a few clipped pixels don't flatten the graph.
    let peak = 1;
    for (const [h] of channels) for (let i = 1; i < 255; i++) peak = Math.max(peak, h[i]);
    ctx.globalCompositeOperation = "lighter";
    for (const [h, color] of channels) {
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.moveTo(0, H);
      for (let i = 0; i < 256; i++) ctx.lineTo(i, H - Math.min(1, Math.sqrt(h[i] / peak)) * (H - 2));
      ctx.lineTo(W, H);
      ctx.closePath();
      ctx.fill();
    }
    ctx.globalCompositeOperation = "source-over";
  }, [histogram]);

  return <canvas ref={ref} width={W} height={H} className="histogram" />;
}
