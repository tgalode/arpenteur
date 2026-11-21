import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, useState } from "react";
import { t } from "../i18n/fr";
import { loadColumnKeys, saveColumnKeys, type ColumnKey } from "../lib/columns";
import { filterRows, sortRows, type StatusClass } from "../lib/table";
import { urlColumn } from "../lib/urlColumns";
import { normalizeUrl } from "../lib/urls";
import { ColumnPicker } from "./ColumnPicker";
import type { UrlRow } from "../lib/types";

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
  const [columnKeys, setColumnKeys] = useState<ColumnKey[]>(loadColumnKeys);
  const scrollRef = useRef<HTMLDivElement>(null);
  const columns = useMemo(() => columnKeys.map(urlColumn), [columnKeys]);
  const grid = useMemo(() => columns.map((c) => c.width).join(" "), [columns]);

  function chooseColumns(keys: ColumnKey[]) {
    setColumnKeys(keys);
    saveColumnKeys(keys);
    setSort((s) => (s && !keys.includes(s.key as ColumnKey) ? null : s));
  }

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
        <ColumnPicker selected={columnKeys} onChange={chooseColumns} />
      </div>
      <div role="table" aria-rowcount={visible.length + 1} className="urltable-table">
        <div ref={scrollRef} className="urltable-scroll">
          <div role="row" className="urltable-head" style={{ gridTemplateColumns: grid }}>
            {columns.map((c) => {
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
                    gridTemplateColumns: grid,
                  }}
                >
                  {columns.map((c) => {
                    const value = c.render(r);
                    const tone = c.tone?.(r) ?? "";
                    const cls = [c.numeric ? "n" : "", tone.startsWith("tone-") ? tone : tone ? "dim" : ""]
                      .filter(Boolean)
                      .join(" ");
                    return (
                      <div key={c.key} role="cell" className={cls || undefined} title={c.tip ? value : undefined}>
                        {value}
                        {c.key === "url" && r.offList && <span className="tag-off-list">{t("offList")}</span>}
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
