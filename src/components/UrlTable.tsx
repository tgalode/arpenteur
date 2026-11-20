import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, useState } from "react";
import { t } from "../i18n/fr";
import { filterRows, sortRows, type StatusClass } from "../lib/table";
import type { UrlRow } from "../lib/types";

type Column = {
  key: keyof UrlRow;
  label: string;
  width: string;
  render: (r: UrlRow) => string;
};

const text = (v: string | number | null | undefined): string =>
  v === null || v === undefined || v === "" ? t("none") : String(v);

const COLUMNS: Column[] = [
  { key: "url", label: t("col_url"), width: "3fr", render: (r) => r.url },
  { key: "status", label: t("col_status"), width: "70px", render: (r) => (r.status < 0 ? t("none") : String(r.status)) },
  { key: "contentType", label: t("col_type"), width: "1fr", render: (r) => text(r.contentType) },
  { key: "title", label: t("col_title"), width: "2fr", render: (r) => text(r.title) },
  { key: "h1", label: t("col_h1"), width: "2fr", render: (r) => text(r.h1) },
  { key: "indexable", label: t("col_indexable"), width: "90px", render: (r) => (r.indexable ? t("yes") : t("no")) },
  { key: "depth", label: t("col_depth"), width: "100px", render: (r) => text(r.depth) },
  { key: "inlinks", label: t("col_inlinks"), width: "120px", render: (r) => text(r.inlinks) },
  { key: "wordCount", label: t("col_words"), width: "70px", render: (r) => text(r.wordCount) },
];

const GRID = COLUMNS.map((c) => c.width).join(" ");
const ROW_HEIGHT = 28;
const STATUS_CLASSES: StatusClass[] = ["all", "2xx", "3xx", "4xx", "5xx", "error"];

function statusLabel(c: StatusClass): string {
  if (c === "all") return t("statusAll");
  if (c === "error") return t("statusError");
  return c;
}

export function UrlTable({
  rows,
  selectedUrl,
  onSelect,
}: {
  rows: UrlRow[];
  selectedUrl: string | null;
  onSelect: (row: UrlRow) => void;
}) {
  const [query, setQuery] = useState("");
  const [statusClass, setStatusClass] = useState<StatusClass>("all");
  const [sort, setSort] = useState<{ key: keyof UrlRow; dir: "asc" | "desc" } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => {
    const filtered = filterRows(rows, { query, statusClass });
    return sort ? sortRows(filtered, sort.key, sort.dir) : filtered;
  }, [rows, query, statusClass, sort]);

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
    <div>
      <div style={{ display: "flex", gap: "0.5rem", padding: "0.5rem 1rem" }}>
        <input
          type="search"
          aria-label={t("filterLabel")}
          placeholder={t("filterPlaceholder")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <select
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
      <div role="table" aria-rowcount={visible.length + 1} style={{ fontSize: "0.85rem" }}>
        <div role="row" style={{ display: "grid", gridTemplateColumns: GRID, fontWeight: 600, padding: "0 1rem" }}>
          {COLUMNS.map((c) => {
            const active = sort?.key === c.key;
            return (
              <div
                key={c.key}
                role="columnheader"
                aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}
                tabIndex={0}
                style={{ cursor: "pointer", userSelect: "none" }}
                onClick={() => toggleSort(c.key)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleSort(c.key);
                  }
                }}
              >
                {c.label}
                {active ? (sort.dir === "asc" ? " ▲" : " ▼") : ""}
              </div>
            );
          })}
        </div>
        <div ref={scrollRef} style={{ height: 360, overflow: "auto" }}>
          <div role="rowgroup" style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((v) => {
              const r = visible[v.index];
              return (
                <div
                  key={r.url}
                  role="row"
                  aria-selected={r.url === selectedUrl}
                  onClick={() => onSelect(r)}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    right: 0,
                    height: v.size,
                    transform: `translateY(${v.start}px)`,
                    display: "grid",
                    gridTemplateColumns: GRID,
                    alignItems: "center",
                    padding: "0 1rem",
                    cursor: "pointer",
                    background: r.url === selectedUrl ? "rgba(0,120,255,0.12)" : undefined,
                  }}
                >
                  {COLUMNS.map((c) => (
                    <div
                      key={c.key}
                      role="cell"
                      style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                    >
                      {c.render(r)}
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
          {visible.length === 0 && <p style={{ padding: "0 1rem" }}>{t("noRows")}</p>}
        </div>
      </div>
    </div>
  );
}
