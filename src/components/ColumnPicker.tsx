import { useEffect, useRef, useState } from "react";
import { t } from "../i18n/fr";
import { ALL_COLUMN_KEYS, normalizeColumnKeys, type ColumnKey } from "../lib/columns";
import { urlColumn } from "../lib/urlColumns";

export function ColumnPicker({
  selected,
  onChange,
}: {
  selected: ColumnKey[];
  onChange: (keys: ColumnKey[]) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    function onPointerDown(e: MouseEvent) {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open]);

  function toggle(key: ColumnKey) {
    if (key === "url") return;
    onChange(normalizeColumnKeys(selected.includes(key) ? selected.filter((k) => k !== key) : [...selected, key]));
  }

  return (
    <div
      ref={rootRef}
      className="columns-picker"
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) {
          e.stopPropagation();
          setOpen(false);
          buttonRef.current?.focus();
        }
      }}
    >
      <button
        ref={buttonRef}
        type="button"
        className="btn"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        {t("columnsButton")}
      </button>
      {open && (
        <div role="group" aria-label={t("columnsListLabel")} className="columns-list">
          {ALL_COLUMN_KEYS.map((key) => (
            <label key={key}>
              <input
                type="checkbox"
                checked={selected.includes(key)}
                disabled={key === "url"}
                onChange={() => toggle(key)}
              />
              {urlColumn(key).label}
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
