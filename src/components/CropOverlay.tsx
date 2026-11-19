import { useRef, type PointerEvent } from "react";
import { dragCrop, type Handle, type Size } from "../lib/crop";
import type { Crop } from "../lib/types";

const HANDLES: Exclude<Handle, "move">[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

interface Props {
  crop: Crop;
  /** Display size of the (uncropped) oriented image, in image pixels. */
  size: Size;
  /** Locked pixel aspect ratio (w/h), or null for free. */
  ratio: number | null;
  onChange: (c: Crop) => void;
}

/** Crop box on top of the straightened image: drag inside to move, handles to resize. */
export function CropOverlay({ crop, size, ratio, onChange }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  const start = (handle: Handle) => (e: PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const bounds = ref.current?.getBoundingClientRect();
    if (!bounds) return;
    const from = { ...crop };
    const [sx, sy] = [e.clientX, e.clientY];
    const move = (ev: globalThis.PointerEvent) =>
      onChange(dragCrop(from, handle, (ev.clientX - sx) / bounds.width, (ev.clientY - sy) / bounds.height, ratio, size));
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };

  return (
    <div className="crop-overlay" ref={ref}>
      <div
        className="crop-box"
        style={{
          left: `${crop.x * 100}%`,
          top: `${crop.y * 100}%`,
          width: `${crop.w * 100}%`,
          height: `${crop.h * 100}%`,
        }}
        onPointerDown={start("move")}
      >
        <div className="crop-grid" />
        {HANDLES.map((h) => (
          <span key={h} className={`crop-handle crop-handle-${h}`} onPointerDown={start(h)} />
        ))}
      </div>
    </div>
  );
}
