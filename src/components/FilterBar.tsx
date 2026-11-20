import { fieldValue, FILTER_FIELDS, isFilterActive, matchesFilter, EMPTY_FILTER, type FilterField } from "../lib/meta";
import { useStore } from "../state/store";

const ANY = "\u0000any";
const UNSET = "\u0000unset";

/** Search and per-field filters over the open frames' metadata. */
export function FilterBar() {
  const frames = useStore((s) => s.frames);
  const filter = useStore((s) => s.filter);
  const levels = useStore((s) => s.archiveLevels);
  const setFilter = useStore((s) => s.setFilter);
  const active = isFilterActive(filter);
  const matching = active ? frames.filter((f) => matchesFilter(f.name, f.meta, filter, levels)).length : frames.length;

  const setField = (key: FilterField, raw: string) => {
    const fields = { ...filter.fields };
    if (raw === ANY) delete fields[key];
    else fields[key] = raw === UNSET ? "" : raw;
    setFilter({ ...filter, fields });
  };

  return (
    <div className="filterbar">
      <input
        type="search"
        className="filter-search"
        placeholder="Search film, place, roll, keywords…"
        value={filter.text}
        spellCheck={false}
        onChange={(e) => setFilter({ ...filter, text: e.target.value })}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setFilter({ ...filter, text: "" });
            e.currentTarget.blur();
          }
        }}
      />
      {FILTER_FIELDS.map(({ key, label }) => {
        const values = [...new Set(frames.map((f) => fieldValue(f.meta, key)))];
        const set = values.filter(Boolean).sort();
        const hasUnset = values.includes("");
        const current = filter.fields[key];
        const name = label(levels);
        return (
          <select
            key={key}
            className={current !== undefined ? "is-on" : ""}
            value={current === undefined ? ANY : current === "" ? UNSET : current}
            disabled={!set.length && current === undefined}
            onChange={(e) => {
              setField(key, e.target.value);
              e.currentTarget.blur();
            }}
          >
            <option value={ANY}>{name}: any</option>
            {set.map((v) => (
              <option key={v} value={v}>
                {name}: {v}
              </option>
            ))}
            {hasUnset && <option value={UNSET}>{name}: not set</option>}
          </select>
        );
      })}
      {active && (
        <>
          <span className="filter-count">
            {matching} of {frames.length}
          </span>
          <button className="link" onClick={() => setFilter(EMPTY_FILTER)}>
            Clear
          </button>
        </>
      )}
    </div>
  );
}
