import { memo, useEffect, useRef, useState } from "react";
import { renderThumbnail } from "../lib/api";
import { useStore, useVisibleFrames, type Frame } from "../state/store";
import { FilterBar } from "./FilterBar";

const THUMB_DEBOUNCE_MS = 250;

const Thumb = memo(function Thumb({
  frame,
  index,
  current,
  selected,
}: {
  frame: Frame;
  index: number;
  current: boolean;
  selected: boolean;
}) {
  const select = useStore((s) => s.select);
  const [url, setUrl] = useState<string | null>(null);
  const urlRef = useRef<string | null>(null);
  const ref = useRef<HTMLButtonElement>(null);
  const first = useRef(true);

  useEffect(() => {
    let cancelled = false;
    const delay = first.current ? 0 : THUMB_DEBOUNCE_MS;
    first.current = false;
    const timer = window.setTimeout(async () => {
      try {
        const blob = await renderThumbnail(frame.id, frame.adjustments, frame.analysis, frame.orientation, frame.crop);
        if (cancelled) return;
        if (urlRef.current) URL.revokeObjectURL(urlRef.current);
        urlRef.current = URL.createObjectURL(blob);
        setUrl(urlRef.current);
      } catch {
        /* frame may have been removed */
      }
    }, delay);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [frame.id, frame.adjustments, frame.analysis, frame.orientation, frame.crop]);

  useEffect(
    () => () => {
      if (urlRef.current) URL.revokeObjectURL(urlRef.current);
    },
    [],
  );

  useEffect(() => {
    if (current) ref.current?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  }, [current]);

  return (
    <button
      ref={ref}
      className={`thumb${current ? " is-current" : ""}${selected ? " is-selected" : ""}`}
      onClick={(e) => select(frame.id, { shift: e.shiftKey, toggle: e.metaKey || e.ctrlKey })}
      title={frame.name}
    >
      <div className="thumb-img">{url ? <img src={url} alt="" draggable={false} /> : <span className="spinner" />}</div>
      <div className="thumb-label">
        <span className="thumb-num">{index + 1}</span>
        <span className="thumb-name">{frame.name}</span>
      </div>
    </button>
  );
});

export function Filmstrip() {
  const all = useStore((s) => s.frames);
  const frames = useVisibleFrames();
  const currentId = useStore((s) => s.currentId);
  const selection = useStore((s) => s.selection);
  const pending = useStore((s) => s.pendingLoads);
  const selected = new Set(selection);

  return (
    <div className="strip">
      {all.length > 0 && <FilterBar />}
      <div className="filmstrip">
        {frames.map((f) => (
          <Thumb
            key={f.id}
            frame={f}
            index={all.indexOf(f)}
            current={f.id === currentId}
            selected={selected.has(f.id)}
          />
        ))}
        {Array.from({ length: pending }, (_, i) => (
          <div key={`p${i}`} className="thumb is-loading">
            <div className="thumb-img">
              <span className="spinner" />
            </div>
            <div className="thumb-label">Loading…</div>
          </div>
        ))}
      </div>
    </div>
  );
}
