import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, useState } from "react";
import { t } from "../i18n/fr";
import { filterRows, httpTone, indexableText, indexableTone, sortRows, statusText, type StatusClass } from "../lib/table";
import { normalizeUrl } from "../lib/urls";
import type { UrlRow } from "../lib/types";

type Column = {
  key: keyof UrlRow;
  label: string;
  width: string;
  render: (r: UrlRow) => string;
  tone?: (r: UrlRow) => string;
  numeric?: boolean;
  tip?: boolean;
};

const text = (v: string | number | null | undefined): string =>
  v === null || v === undefined || v === "" ? t("none") : String(v);

const isEmpty = (v: string | number | null | undefined): boolean => v === null || v === undefined || v === "";

const COLUMNS: Column[] = [
  { key: "url", label: t("col_url"), width: "3fr", render: (r) => r.url, tip: true },
  {
    key: "status",
    label: t("col_status"),
    width: "calc(20ch + 18px)",
    render: (r) => statusText(r.status),
    tip: true,
    tone: (r) => `tone-${httpTone(r.status)}`,
  },
  {
    key: "contentType",
    label: t("col_type"),
    width: "1fr",
    render: (r) => text(r.contentType),
    tone: () => "dim",
  },
  {
    key: "title",
    label: t("col_title"),
    width: "2fr",
    render: (r) => text(r.title),
    tone: (r) => (isEmpty(r.title) ? "dim" : ""),
    tip: true,
  },
  {
    key: "h1",
    label: t("col_h1"),
    width: "2fr",
    render: (r) => text(r.h1),
    tone: (r) => (isEmpty(r.h1) ? "dim" : ""),
    tip: true,
  },
  {
    key: "indexable",
    label: t("col_indexable"),
    width: "calc(12ch + 18px)",
    render: (r) => indexableText(r.indexable),
    tone: (r) => indexableTone(r.indexable),
  },
  { key: "depth", label: t("col_depth"), width: "calc(9ch + 18px)", render: (r) => text(r.depth), numeric: true, tone: (r) => (isEmpty(r.depth) ? "dim" : "") },
  { key: "inlinks", label: t("col_inlinks"), width: "calc(14ch + 18px)", render: (r) => text(r.inlinks), numeric: true, tone: (r) => (isEmpty(r.inlinks) ? "dim" : "") },
  { key: "wordCount", label: t("col_words"), width: "calc(8ch + 18px)", render: (r) => text(r.wordCount), numeric: true, tone: (r) => (isEmpty(r.wordCount) ? "dim" : "") },
];

const GRID = COLUMNS.map((c) => c.width).join(" ");
const ROW_HEIGHT = 26;
const STATUS_CLASSES: StatusClass[] = ["all", "2xx", "3xx", "4xx", "5xx", "error"];

function statusLabel(c: StatusClass): string {
  if (c === "all") return t("statusAll");
  if (c === "error") return t("statusError");
  return c;
}

export function UrlTable({
  rows,
  selectedUrl,
  urlFilter,
  onSelect,
}: {
  rows: UrlRow[];
  selectedUrl: string | null;
  urlFilter: Set<string> | null;
  onSelect: (row: UrlRow) => void;
}) {
  const [query, setQuery] = useState("");
  const [statusClass, setStatusClass] = useState<StatusClass>("all");
  const [sort, setSort] = useState<{ key: keyof UrlRow; dir: "asc" | "desc" } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => {
    const scoped = urlFilter ? rows.filter((r) => urlFilter.has(normalizeUrl(r.url))) : rows;
    const filtered = filterRows(scoped, { query, statusClass });
    return sort ? sortRows(filtered, sort.key, sort.dir) : filtered;
  }, [rows, urlFilter, query, statusClass, sort]);

  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
    initialRect: { width: 1000, height: 600 },
  });

  function toggleSort(key: keyof UrlRow) {
    setSort((s) => (s && s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: "asc" }));
  }

  return (
    <div className="urltable">
      <div className="urltable-tools">
        <input
          type="search"
          className="field"
          aria-label={t("filterLabel")}
          placeholder={t("filterPlaceholder")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <select
          className="field"
          aria-label={t("statusFilterLabel")}
          value={statusClass}
          onChange={(e) => setStatusClass(e.target.value as StatusClass)}
        >
          {STATUS_CLASSES.map((c) => (
            <option key={c} value={c}>
              {statusLabel(c)}
            </option>
          ))}
        </select>
      </div>
      <div role="table" aria-rowcount={visible.length + 1} className="urltable-table">
        <div ref={scrollRef} className="urltable-scroll">
          <div role="row" className="urltable-head" style={{ gridTemplateColumns: GRID }}>
            {COLUMNS.map((c) => {
              const active = sort?.key === c.key;
              return (
                <div
                  key={c.key}
                  role="columnheader"
                  className={c.numeric ? "n" : undefined}
                  aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}
                  tabIndex={0}
                  onClick={() => toggleSort(c.key)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      toggleSort(c.key);
                    }
                  }}
                >
                  <span>{c.label}</span>
                </div>
              );
            })}
          </div>
          <div role="rowgroup" style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((v) => {
              const r = visible[v.index];
              return (
                <div
                  key={r.url}
                  role="row"
                  className="urltable-row"
                  tabIndex={0}
                  aria-selected={r.url === selectedUrl}
                  onClick={() => onSelect(r)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelect(r);
                    }
                  }}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    right: 0,
                    height: v.size,
                    transform: `translateY(${v.start}px)`,
                    gridTemplateColumns: GRID,
                  }}
                >
                  {COLUMNS.map((c) => {
                    const value = c.render(r);
                    const tone = c.tone?.(r) ?? "";
                    const cls = [c.numeric ? "n" : "", tone.startsWith("tone-") ? tone : tone ? "dim" : ""]
                      .filter(Boolean)
                      .join(" ");
                    return (
                      <div key={c.key} role="cell" className={cls || undefined} title={c.tip ? value : undefined}>
                        {value}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
          {visible.length === 0 && <p className="dim urltable-empty">{urlFilter ? t("noRowsProblem") : t("noRows")}</p>}
        </div>
      </div>
    </div>
  );
}
