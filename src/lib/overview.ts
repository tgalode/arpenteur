import { filterRows, type StatusClass } from "./table";
import type { UrlRow } from "./types";

export type Grouping = "priorite" | "theme";

type ResponseClass = Exclude<StatusClass, "all">;

export const RESPONSE_CLASSES: ResponseClass[] = ["2xx", "3xx", "4xx", "5xx", "error"];

const GROUPINGS: Grouping[] = ["priorite", "theme"];
const STORAGE_KEY = "arpenteur.grouping";

/** Chaque classe est comptée par le filtre du tableau, pour que les deux nombres concordent. */
export function crawlSummary(rows: UrlRow[]): {
  urls: number;
  byStatus: Record<ResponseClass, number>;
  /** Lignes d'aucune des cinq classes (statut « ignorée »). */
  ignored: number;
  indexable: number;
} {
  const byStatus = {} as Record<ResponseClass, number>;
  for (const c of RESPONSE_CLASSES) byStatus[c] = filterRows(rows, { query: "", statusClass: c }).length;
  const classed = RESPONSE_CLASSES.reduce((sum, c) => sum + byStatus[c], 0);
  return { urls: rows.length, byStatus, ignored: rows.length - classed, indexable: rows.filter((r) => r.indexable === "yes").length };
}

export function loadGrouping(): Grouping {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return GROUPINGS.find((g) => g === raw) ?? "priorite";
  } catch {
    return "priorite";
  }
}

export function saveGrouping(grouping: Grouping): void {
  try {
    localStorage.setItem(STORAGE_KEY, grouping);
  } catch {
    // stockage indisponible : le choix vaut pour la session seulement
  }
}
