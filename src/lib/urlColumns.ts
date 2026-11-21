import { t } from "../i18n/fr";
import type { ColumnKey } from "./columns";
import { formatMs, formatSizeKo, httpTone, indexableText, indexableTone, responseMs, statusText } from "./table";
import type { UrlRow } from "./types";

export type UrlColumn = {
  key: ColumnKey;
  label: string;
  csvLabel: string;
  width: string;
  render: (r: UrlRow) => string;
  csv: (r: UrlRow) => string | number | null;
  tone?: (r: UrlRow) => string;
  numeric?: boolean;
  tip?: boolean;
};

type Cell = string | number | null | undefined;

const text = (v: Cell): string => (v === null || v === undefined || v === "" ? t("none") : String(v));
const isEmpty = (v: Cell): boolean => v === null || v === undefined || v === "";
const dimIfEmpty = (v: Cell) => (isEmpty(v) ? "dim" : "");

const FIELD_COLUMNS: Record<ColumnKey, Omit<UrlColumn, "key">> = {
  url: { label: t("col_url"), csvLabel: t("csvcol_url"), width: "3fr", render: (r) => r.url, csv: (r) => r.url, tip: true },
  status: {
    label: t("col_status"),
    csvLabel: t("csvcol_status"),
    width: "calc(20ch + 18px)",
    render: (r) => statusText(r.status),
    csv: (r) => statusText(r.status),
    tip: true,
    tone: (r) => `tone-${httpTone(r.status)}`,
  },
  contentType: {
    label: t("col_type"),
    csvLabel: t("csvcol_contentType"),
    width: "1fr",
    render: (r) => text(r.contentType),
    csv: (r) => r.contentType,
    tone: () => "dim",
  },
  title: {
    label: t("col_title"),
    csvLabel: t("csvcol_title"),
    width: "2fr",
    render: (r) => text(r.title),
    csv: (r) => r.title,
    tone: (r) => dimIfEmpty(r.title),
    tip: true,
  },
  titleLength: {
    label: t("col_titleLength"),
    csvLabel: t("csvcol_titleLength"),
    width: "calc(13ch + 18px)",
    render: (r) => text(r.titleLength),
    csv: (r) => r.titleLength,
    numeric: true,
    tone: (r) => dimIfEmpty(r.titleLength),
  },
  metaDescription: {
    label: t("col_metaDescription"),
    csvLabel: t("csvcol_metaDescription"),
    width: "2fr",
    render: (r) => text(r.metaDescription),
    csv: (r) => r.metaDescription,
    tone: (r) => dimIfEmpty(r.metaDescription),
    tip: true,
  },
  metaDescriptionLength: {
    label: t("col_metaDescriptionLength"),
    csvLabel: t("csvcol_metaDescriptionLength"),
    width: "calc(12ch + 18px)",
    render: (r) => text(r.metaDescriptionLength),
    csv: (r) => r.metaDescriptionLength,
    numeric: true,
    tone: (r) => dimIfEmpty(r.metaDescriptionLength),
  },
  h1: {
    label: t("col_h1"),
    csvLabel: t("csvcol_h1"),
    width: "2fr",
    render: (r) => text(r.h1),
    csv: (r) => r.h1,
    tone: (r) => dimIfEmpty(r.h1),
    tip: true,
  },
  h1Count: {
    label: t("col_h1Count"),
    csvLabel: t("csvcol_h1Count"),
    width: "calc(8ch + 18px)",
    render: (r) => text(r.h1Count),
    csv: (r) => r.h1Count,
    numeric: true,
  },
  canonical: {
    label: t("col_canonical"),
    csvLabel: t("csvcol_canonical"),
    width: "2fr",
    render: (r) => text(r.canonical),
    csv: (r) => r.canonical,
    tone: (r) => dimIfEmpty(r.canonical),
    tip: true,
  },
  indexable: {
    label: t("col_indexable"),
    csvLabel: t("csvcol_indexable"),
    width: "calc(12ch + 18px)",
    render: (r) => indexableText(r.indexable),
    csv: (r) => indexableText(r.indexable),
    tone: (r) => indexableTone(r.indexable),
  },
  depth: {
    label: t("col_depth"),
    csvLabel: t("csvcol_depth"),
    width: "calc(9ch + 18px)",
    render: (r) => text(r.depth),
    csv: (r) => r.depth,
    numeric: true,
    tone: (r) => dimIfEmpty(r.depth),
  },
  inlinks: {
    label: t("col_inlinks"),
    csvLabel: t("csvcol_inlinks"),
    width: "calc(14ch + 18px)",
    render: (r) => text(r.inlinks),
    csv: (r) => r.inlinks,
    numeric: true,
    tone: (r) => dimIfEmpty(r.inlinks),
  },
  wordCount: {
    label: t("col_words"),
    csvLabel: t("csvcol_words"),
    width: "calc(8ch + 18px)",
    render: (r) => text(r.wordCount),
    csv: (r) => r.wordCount,
    numeric: true,
    tone: (r) => dimIfEmpty(r.wordCount),
  },
  size: {
    label: t("col_size"),
    csvLabel: t("csvcol_size"),
    width: "calc(9ch + 18px)",
    render: (r) => (r.size === null ? t("none") : formatSizeKo(r.size)),
    csv: (r) => r.size,
    numeric: true,
    tone: (r) => dimIfEmpty(r.size),
  },
  elapsedMs: {
    label: t("col_elapsedMs"),
    csvLabel: t("csvcol_elapsedMs"),
    width: "calc(9ch + 18px)",
    render: (r) => (responseMs(r) === null ? t("none") : formatMs(r.elapsedMs)),
    csv: responseMs,
    numeric: true,
    tone: (r) => dimIfEmpty(responseMs(r)),
  },
};

export function urlColumn(key: ColumnKey): UrlColumn {
  return { key, ...FIELD_COLUMNS[key] };
}
