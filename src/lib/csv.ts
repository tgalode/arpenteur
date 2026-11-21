import { t } from "../i18n/fr";
import { CATALOG, findingTitle, LEVELS } from "./catalog";
import codes from "./finding-codes.json";
import { indexableText, statusText } from "./table";
import type { Finding, UrlRow } from "./types";

// Excel en français : séparateur « ; », UTF-8 avec BOM, fins de ligne CRLF.
const BOM = "﻿";
const SEPARATOR = ";";
const EOL = "\r\n";

const PLAIN_NUMBER = /^-?\d+([.,]\d+)?$/;

function neutralizeFormula(text: string): string {
  const risky = /^[=+@\t\r]/.test(text) || (text.startsWith("-") && !PLAIN_NUMBER.test(text));
  return risky ? `'${text}` : text;
}

/** Une cellule CSV : neutralise les formules (texte venu de pages tierces), puis échappe. */
export function csvCell(value: string | number | null | undefined): string {
  const text = neutralizeFormula(value === null || value === undefined ? "" : String(value));
  return /[;"\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function toCsv(header: string[], lines: Array<Array<string | number | null | undefined>>): string {
  return BOM + [header, ...lines].map((cells) => cells.map(csvCell).join(SEPARATOR)).join(EOL);
}

export function urlsCsv(rows: UrlRow[]): string {
  return toCsv(
    [
      t("col_url"),
      t("col_status"),
      t("col_type"),
      t("col_title"),
      t("col_h1"),
      t("col_indexable"),
      t("col_depth"),
      t("col_inlinks"),
      t("col_words"),
    ],
    rows.map((r) => [
      r.url,
      statusText(r.status),
      r.contentType,
      r.title,
      r.h1,
      indexableText(r.indexable),
      r.depth,
      r.inlinks,
      r.wordCount,
    ]),
  );
}

const CODE_ORDER = new Map((codes as string[]).map((code, i) => [code, i]));

/** Niveau (haute en premier), puis ordre du catalogue, puis URL : un tri Excel sur « niveau » reste lisible. */
function byLevelCodeUrl(a: Finding, b: Finding): number {
  return (
    LEVELS.indexOf(CATALOG[a.code].level) - LEVELS.indexOf(CATALOG[b.code].level) ||
    (CODE_ORDER.get(a.code) ?? 0) - (CODE_ORDER.get(b.code) ?? 0) ||
    a.url.localeCompare(b.url)
  );
}

export function problemsCsv(findings: Finding[]): string {
  return toCsv(
    [t("csv_level"), t("csv_code"), t("csv_problem"), t("csv_url"), t("csv_detail"), t("csv_related")],
    [...findings].sort(byLevelCodeUrl).map((f) => [
      t(`level_${CATALOG[f.code].level}`),
      f.code,
      findingTitle(f.code),
      f.url,
      f.detail,
      f.related.join(" "),
    ]),
  );
}
