import { t } from "../i18n/fr";
import type { CrawlStatus, UrlRow } from "./types";

// Statuts négatifs de SiteOne (result/visited_url.rs) : aucune réponse HTTP.
const ERROR_CONNECTION_FAIL = -1;
const ERROR_TIMEOUT = -2;
/** URL ignorée par SiteOne sans requête : ce n'est pas une erreur. */
export const ERROR_SKIPPED = -6;

export function statusText(status: number): string {
  if (status >= 0) return String(status);
  switch (status) {
    case ERROR_SKIPPED:
      return t("httpStatus_skipped");
    case ERROR_CONNECTION_FAIL:
      return t("httpStatus_connection");
    case ERROR_TIMEOUT:
      return t("httpStatus_timeout");
    default:
      return t("httpStatus_network");
  }
}

export type StatusClass = "all" | "2xx" | "3xx" | "4xx" | "5xx" | "error";

function compare(a: string | number | boolean, b: string | number | boolean): number {
  if (typeof a === "string" && typeof b === "string") {
    return a.localeCompare(b, "fr", { sensitivity: "base" });
  }
  if (typeof a === "boolean" && typeof b === "boolean") {
    return Number(a) - Number(b);
  }
  return (a as number) - (b as number);
}

export function sortRows(rows: UrlRow[], key: keyof UrlRow, dir: "asc" | "desc"): UrlRow[] {
  const sign = dir === "asc" ? 1 : -1;
  return [...rows].sort((x, y) => {
    const a = x[key];
    const b = y[key];
    const aNull = a === null || a === undefined;
    const bNull = b === null || b === undefined;
    if (aNull && bNull) return 0;
    if (aNull) return 1;
    if (bNull) return -1;
    return sign * compare(a, b);
  });
}

function matchesStatus(status: number, cls: StatusClass): boolean {
  switch (cls) {
    case "all":
      return true;
    case "error":
      return status < 0 && status !== ERROR_SKIPPED;
    default:
      return status >= 0 && Math.floor(status / 100) === Number(cls[0]);
  }
}

export function filterRows(rows: UrlRow[], f: { query: string; statusClass: StatusClass }): UrlRow[] {
  const q = f.query.trim().toLowerCase();
  return rows.filter((r) => {
    if (!matchesStatus(r.status, f.statusClass)) return false;
    if (!q) return true;
    return [r.url, r.title, r.h1].some((v) => v?.toLowerCase().includes(q));
  });
}

export type StatusTone = "ok" | "redir" | "warn" | "err" | "dim";

export function statusTone(kind: CrawlStatus["kind"]): StatusTone {
  switch (kind) {
    case "completed":
      return "ok";
    case "running":
      return "redir";
    case "stopped":
    case "throttled":
      return "warn";
    case "failed":
      return "err";
  }
}
