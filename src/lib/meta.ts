import type { ExportMetadata, FrameMeta } from "./types";

export const DEFAULT_ARCHIVE_LEVELS = ["Binder", "Page", "Strip"];

export const FILM_STOCKS = [
  "Kodak Portra 160",
  "Kodak Portra 400",
  "Kodak Portra 800",
  "Kodak Ektar 100",
  "Kodak Gold 200",
  "Kodak UltraMax 400",
  "Kodak ColorPlus 200",
  "Kodak Ektachrome E100",
  "Kodak Tri-X 400",
  "Kodak T-Max 100",
  "Kodak T-Max 400",
  "Kodak Vision3 250D",
  "Kodak Vision3 500T",
  "Fujifilm Fujicolor C200",
  "Fujifilm Superia X-TRA 400",
  "Fujifilm Pro 400H",
  "Fujifilm Velvia 50",
  "Fujifilm Velvia 100",
  "Fujifilm Provia 100F",
  "Fujifilm Acros 100 II",
  "Ilford HP5 Plus",
  "Ilford FP4 Plus",
  "Ilford Delta 100",
  "Ilford Delta 400",
  "Ilford Delta 3200",
  "Ilford Pan F Plus 50",
  "Ilford XP2 Super",
  "Kentmere Pan 100",
  "Kentmere Pan 400",
  "Fomapan 100",
  "Fomapan 200",
  "Fomapan 400",
  "CineStill 50D",
  "CineStill 400D",
  "CineStill 800T",
  "Harman Phoenix 200",
  "Lomography Color Negative 100",
  "Lomography Color Negative 400",
  "Lomography Color Negative 800",
  "Rollei RPX 25",
  "Rollei RPX 100",
  "Rollei RPX 400",
  "Adox CHS 100 II",
];

export const FORMATS = [
  "35mm",
  "35mm half frame",
  "120 6×4.5",
  "120 6×6",
  "120 6×7",
  "120 6×8",
  "120 6×9",
  "120 6×12",
  "120 6×17",
  "4×5",
  "5×7",
  "8×10",
  "110",
  "APS",
];

export const PROCESSES = ["C-41", "E-6", "B&W", "ECN-2", "Cross (E-6 in C-41)"];

/** Accepts YYYY, YYYY-MM, YYYY-MM-DD (as XMP does). */
export function isValidDate(s: string): boolean {
  const m = /^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/.exec(s);
  if (!m) return false;
  const [y, mo, d] = [Number(m[1]), m[2] ? Number(m[2]) : 1, m[3] ? Number(m[3]) : 1];
  return y >= 1800 && y <= 2999 && mo >= 1 && mo <= 12 && d >= 1 && d <= 31;
}

/** Tidy common date spellings: 2026/3/5, 2026.03.05 → 2026-03-05. */
export function normalizeDate(s: string): string {
  const t = s.trim();
  const m = /^(\d{4})[-/.](\d{1,2})(?:[-/.](\d{1,2}))?$/.exec(t);
  if (!m) return t;
  const pad = (v: string) => v.padStart(2, "0");
  return m[3] ? `${m[1]}-${pad(m[2])}-${pad(m[3])}` : `${m[1]}-${pad(m[2])}`;
}

/** Keywords as typed ("Travel > Brazil, family") → hierarchical paths ("Travel|Brazil"). */
export const parseKeywords = (s: string) =>
  s
    .split(",")
    .map((k) =>
      k
        .split(/[>|]/)
        .map((p) => p.trim())
        .filter(Boolean)
        .join("|"),
    )
    .filter(Boolean);

export function toExportMetadata(m: FrameMeta, levels: string[]): ExportMetadata {
  const iso = parseInt(m.iso, 10);
  return {
    filmStock: m.filmStock,
    format: m.format,
    process: m.process,
    camera: m.camera,
    lens: m.lens,
    iso: Number.isFinite(iso) && iso > 0 ? iso : null,
    dateShot: isValidDate(m.dateShot.trim()) ? m.dateShot.trim() : "",
    rollId: m.rollId,
    description: m.description,
    location: m.location,
    city: m.city,
    country: m.country,
    archive: levels.map((name, i) => ({ name, value: m.archive[i] ?? "" })),
    keywords: parseKeywords(m.keywords),
    creator: m.creator,
    copyright: m.copyright,
  };
}

/** "Binder 3 / Page 12" */
export const archiveLabel = (m: FrameMeta, levels: string[]) =>
  levels
    .map((name, i) => (m.archive[i]?.trim() ? `${name} ${m.archive[i].trim()}` : ""))
    .filter(Boolean)
    .join(" / ");

// ---- Filtering --------------------------------------------------------------

export type FilterField = "filmStock" | "rollId" | "year" | "archive" | "camera";

export interface FrameFilter {
  text: string;
  /** Exact value per field; "" matches frames where the field is not set. */
  fields: Partial<Record<FilterField, string>>;
}

export const EMPTY_FILTER: FrameFilter = { text: "", fields: {} };

export const FILTER_FIELDS: { key: FilterField; label: (levels: string[]) => string }[] = [
  { key: "filmStock", label: () => "Film" },
  { key: "year", label: () => "Year" },
  { key: "rollId", label: () => "Roll" },
  { key: "archive", label: (levels) => levels[0] || "Archive" },
  { key: "camera", label: () => "Camera" },
];

/** The value a frame has for a filter field ("" when unset). */
export function fieldValue(m: FrameMeta, field: FilterField): string {
  switch (field) {
    case "year":
      return /^\d{4}/.exec(m.dateShot.trim())?.[0] ?? "";
    case "archive":
      return m.archive[0]?.trim() ?? "";
    default:
      return m[field].trim();
  }
}

export const isFilterActive = (f: FrameFilter) =>
  f.text.trim() !== "" || Object.values(f.fields).some((v) => v !== undefined);

export function matchesFilter(name: string, m: FrameMeta, f: FrameFilter, levels: string[]): boolean {
  for (const [k, v] of Object.entries(f.fields) as [FilterField, string | undefined][]) {
    if (v !== undefined && fieldValue(m, k) !== v) return false;
  }
  const words = f.text.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const hay = [
    name,
    m.filmStock,
    m.format,
    m.process,
    m.iso,
    m.camera,
    m.lens,
    m.dateShot,
    m.location,
    m.city,
    m.country,
    m.rollId,
    archiveLabel(m, levels),
    m.description,
    m.keywords,
    m.creator,
  ]
    .join("\n")
    .toLowerCase();
  return words.every((w) => hay.includes(w));
}
