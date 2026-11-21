export type ColumnKey =
  | "url"
  | "status"
  | "contentType"
  | "title"
  | "titleLength"
  | "metaDescription"
  | "metaDescriptionLength"
  | "h1"
  | "h1Count"
  | "canonical"
  | "indexable"
  | "depth"
  | "inlinks"
  | "wordCount"
  | "size"
  | "elapsedMs";

/** Les 16 colonnes, dans l’ordre du CSV et de la liste « colonnes ». */
export const ALL_COLUMN_KEYS: ColumnKey[] = [
  "url",
  "status",
  "contentType",
  "title",
  "titleLength",
  "metaDescription",
  "metaDescriptionLength",
  "h1",
  "h1Count",
  "canonical",
  "indexable",
  "depth",
  "inlinks",
  "wordCount",
  "size",
  "elapsedMs",
];

export const DEFAULT_COLUMN_KEYS: ColumnKey[] = [
  "url",
  "status",
  "contentType",
  "title",
  "h1",
  "indexable",
  "depth",
  "inlinks",
  "wordCount",
];

const STORAGE_KEY = "arpenteur.columns";

/** Garde les clés connues, ajoute « url », remet l’ordre des colonnes. */
export function normalizeColumnKeys(keys: unknown[]): ColumnKey[] {
  const wanted = new Set<unknown>([...keys, "url"]);
  return ALL_COLUMN_KEYS.filter((k) => wanted.has(k));
}

export function loadColumnKeys(): ColumnKey[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return DEFAULT_COLUMN_KEYS;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return DEFAULT_COLUMN_KEYS;
    const known = parsed.filter((k) => ALL_COLUMN_KEYS.includes(k as ColumnKey));
    return known.length === 0 ? DEFAULT_COLUMN_KEYS : normalizeColumnKeys(known);
  } catch {
    return DEFAULT_COLUMN_KEYS;
  }
}

export function saveColumnKeys(keys: ColumnKey[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(normalizeColumnKeys(keys)));
  } catch {
    // stockage indisponible : le choix vaut pour la session seulement
  }
}
