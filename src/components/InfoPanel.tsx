import { useMemo, useState } from "react";
import { FILM_STOCKS, FORMATS, isValidDate, normalizeDate, PROCESSES } from "../lib/meta";
import type { MetaKey } from "../lib/types";
import { useStore, type Frame } from "../state/store";

const MAX_LEVELS = 5;

interface FieldProps {
  label: string;
  /** null = the selected frames disagree. */
  value: string | null;
  onChange: (v: string) => void;
  onBlur?: () => void;
  list?: string[];
  placeholder?: string;
  invalid?: boolean;
  multiline?: boolean;
  id: string;
  wide?: boolean;
}

function Field({ label, value, onChange, onBlur, list, placeholder, invalid, multiline, id, wide }: FieldProps) {
  const common = {
    id,
    value: value ?? "",
    placeholder: value === null ? "Mixed - type to set all" : placeholder,
    onChange: (e: { target: { value: string } }) => onChange(e.target.value),
    onBlur,
    spellCheck: false,
    className: invalid ? "is-invalid" : undefined,
  };
  return (
    <label className={`info-field${wide ? " info-wide" : ""}`} htmlFor={id}>
      <span>{label}</span>
      {multiline ? (
        <textarea rows={3} {...common} spellCheck />
      ) : (
        <input type="text" autoComplete="off" list={list ? `${id}-list` : undefined} {...common} />
      )}
      {list && (
        <datalist id={`${id}-list`}>
          {list.map((v) => (
            <option key={v} value={v} />
          ))}
        </datalist>
      )}
    </label>
  );
}

/** Sorted unique non-empty values, used ones first, then presets. */
function suggestions(frames: Frame[], key: MetaKey, presets: string[] = []) {
  const used = new Set(frames.map((f) => f.meta[key].trim()).filter(Boolean));
  return [...[...used].sort(), ...presets.filter((p) => !used.has(p))];
}

export function InfoPanel() {
  const frames = useStore((s) => s.frames);
  const selection = useStore((s) => s.selection);
  const currentId = useStore((s) => s.currentId);
  const levels = useStore((s) => s.archiveLevels);
  const { setMeta, setArchiveValue, setArchiveLevels } = useStore.getState();
  const [editLevels, setEditLevels] = useState(false);

  const targets = useMemo(() => {
    const ids = new Set(selection.length ? selection : currentId != null ? [currentId] : []);
    return frames.filter((f) => ids.has(f.id));
  }, [frames, selection, currentId]);

  if (!targets.length) {
    return <p className="panel-hint info-empty">Add scans to describe them.</p>;
  }

  const common = (get: (f: Frame) => string): string | null => {
    const first = get(targets[0]);
    return targets.every((f) => get(f) === first) ? first : null;
  };
  const value = (k: MetaKey) => common((f) => f.meta[k]);
  const field = (k: MetaKey, label: string, extra: Partial<FieldProps> = {}) => (
    <Field id={`meta-${k}`} label={label} value={value(k)} onChange={(v) => setMeta({ [k]: v })} {...extra} />
  );
  const date = value("dateShot");

  return (
    <div className="info">
      <div className="info-target">
        {targets.length > 1 ? (
          <>
            <b>{targets.length} frames</b> - edits apply to all selected
          </>
        ) : (
          targets[0].name
        )}
      </div>

      <section className="panel-section">
        <h3>Film</h3>
        <div className="info-grid">
          {field("filmStock", "Film stock", { list: suggestions(frames, "filmStock", FILM_STOCKS), wide: true })}
          {field("format", "Format", { list: suggestions(frames, "format", FORMATS) })}
          {field("process", "Process", { list: suggestions(frames, "process", PROCESSES) })}
          {field("iso", "ISO / EI", { placeholder: "400" })}
          {field("rollId", "Roll", { placeholder: "2026-014", list: suggestions(frames, "rollId") })}
        </div>
      </section>

      <section className="panel-section">
        <h3>Shot</h3>
        <div className="info-grid">
          {field("dateShot", "Date", {
            placeholder: "YYYY-MM-DD, YYYY-MM or YYYY",
            invalid: !!date?.trim() && !isValidDate(date.trim()),
            onBlur: () => date && date !== normalizeDate(date) && setMeta({ dateShot: normalizeDate(date) }),
            wide: true,
          })}
          {field("camera", "Camera", { list: suggestions(frames, "camera") })}
          {field("lens", "Lens", { list: suggestions(frames, "lens") })}
          {field("location", "Place", { placeholder: "e.g. Ibirapuera Park", wide: true })}
          {field("city", "City", { list: suggestions(frames, "city") })}
          {field("country", "Country", { list: suggestions(frames, "country") })}
          {field("description", "Description", { multiline: true, wide: true })}
        </div>
      </section>

      <section className="panel-section">
        <div className="info-head">
          <h3>Archive</h3>
          <button className="link" onClick={() => setEditLevels((v) => !v)}>
            {editLevels ? "Done" : "Rename levels"}
          </button>
        </div>
        {editLevels ? (
          <div className="info-levels">
            {levels.map((name, i) => (
              <div key={i} className="info-level">
                <input
                  type="text"
                  value={name}
                  spellCheck={false}
                  onChange={(e) => setArchiveLevels(levels.map((n, j) => (j === i ? e.target.value : n)))}
                />
                <button
                  className="tool tool-small"
                  disabled={levels.length <= 1}
                  onClick={() => setArchiveLevels(levels.filter((_, j) => j !== i))}
                  title="Remove level"
                >
                  ×
                </button>
              </div>
            ))}
            {levels.length < MAX_LEVELS && (
              <button className="btn" onClick={() => setArchiveLevels([...levels, `Level ${levels.length + 1}`])}>
                + Add level
              </button>
            )}
            <p className="panel-hint">Level names are shared by all rolls; values are per frame.</p>
          </div>
        ) : (
          <div className="info-grid info-grid-levels" style={{ gridTemplateColumns: `repeat(${levels.length}, 1fr)` }}>
            {levels.map((name, i) => (
              <Field
                key={i}
                id={`meta-archive-${i}`}
                label={name || `Level ${i + 1}`}
                value={common((f) => f.meta.archive[i] ?? "")}
                onChange={(v) => setArchiveValue(i, v)}
              />
            ))}
          </div>
        )}
      </section>

      <section className="panel-section">
        <h3>Keywords &amp; rights</h3>
        <div className="info-grid">
          {field("keywords", "Keywords", { placeholder: "family, Travel > Brazil", wide: true })}
          {field("creator", "Creator", { wide: true })}
          {field("copyright", "Copyright", { placeholder: "© 2026 Your Name", wide: true })}
        </div>
        <p className="panel-hint">
          Creator and copyright are remembered for new scans. Everything is saved to <code>roll.contactsheet</code> in
          the scans&apos; folder and embedded on export.
        </p>
      </section>
    </div>
  );
}
