import { Fragment, useState } from "react";
import { t, type FrKey } from "../i18n/fr";
import { CATALOG, findingTitle, kindLabel, LEVELS, themeLabel, THEMES, type Level } from "../lib/catalog";
import { crawlSummary, loadGrouping, RESPONSE_CLASSES, saveGrouping, type Grouping } from "../lib/overview";
import type { StatusClass } from "../lib/table";
import type { Finding, FindingCode, UrlRow } from "../lib/types";

export function levelLabel(level: Level): string {
  return t(`level_${level}` as FrKey);
}

function responseLabel(c: Exclude<StatusClass, "all">): string {
  return c === "error" ? t("statusError") : c;
}

type Group = { key: string; className: string; label: string; codes: FindingCode[]; total: number };

export function ProblemsPanel({
  findings,
  rows,
  selectedCode,
  onSelect,
  statusClass,
  onStatusClass,
}: {
  findings: Finding[];
  rows: UrlRow[];
  selectedCode: FindingCode | null;
  onSelect: (code: FindingCode | null) => void;
  statusClass: StatusClass;
  onStatusClass: (c: StatusClass) => void;
}) {
  const [grouping, setGrouping] = useState<Grouping>(loadGrouping);

  function chooseGrouping(g: Grouping) {
    setGrouping(g);
    saveGrouping(g);
  }

  const summary = crawlSummary(rows);

  const counts = new Map<FindingCode, number>();
  for (const f of findings) counts.set(f.code, (counts.get(f.code) ?? 0) + 1);
  const count = (c: FindingCode) => counts.get(c) ?? 0;
  const present = [...counts.keys()];
  const sum = (codes: FindingCode[]) => codes.reduce((n, c) => n + count(c), 0);
  const levelRank = (c: FindingCode) => LEVELS.indexOf(CATALOG[c].level);

  const info = present.filter((c) => CATALOG[c].level === "info").sort((a, b) => count(b) - count(a));
  const counted = present.filter((c) => CATALOG[c].level !== "info");

  const groups: Group[] =
    grouping === "priorite"
      ? LEVELS.filter((level) => level !== "info")
          .map((level) => {
            const codes = counted.filter((c) => CATALOG[c].level === level).sort((a, b) => count(b) - count(a));
            return { key: level, className: `lv-${level}`, label: levelLabel(level), codes, total: sum(codes) };
          })
          .filter((g) => g.codes.length > 0)
      : THEMES.map((theme) => {
          const codes = counted
            .filter((c) => CATALOG[c].theme === theme)
            .sort((a, b) => levelRank(a) - levelRank(b) || count(b) - count(a));
          return { key: theme, className: "problems-theme", label: themeLabel(theme), codes, total: sum(codes) };
        }).filter((g) => g.codes.length > 0);

  if (info.length > 0) {
    groups.push({ key: "info", className: "lv-info", label: levelLabel("info"), codes: info, total: sum(info) });
  }

  const problems = sum(counted);

  function codeButton(code: FindingCode) {
    const n = count(code);
    const title = findingTitle(code);
    const kind = kindLabel(CATALOG[code].kind);
    return (
      <li key={code}>
        <button
          type="button"
          className="problems-code"
          aria-pressed={code === selectedCode}
          aria-label={t("problems_code_label", { title, kind, count: n })}
          onClick={() => onSelect(code === selectedCode ? null : code)}
        >
          <span>
            {title} <span className="problems-kind dim">{kind}</span>
          </span>
          <span className="n">{n}</span>
        </button>
      </li>
    );
  }

  return (
    <aside className="problems" aria-label={t("problems_label")}>
      <h2 className="problems-title">{t("overview_title")}</h2>
      <ul className="overview">
        <li className="overview-line">
          <span>{t("overview_urls")}</span>
          <span className="n">{summary.urls}</span>
        </li>
        {RESPONSE_CLASSES.filter((c) => c !== "error" || summary.byStatus.error > 0 || statusClass === "error").map((c) => (
          <li key={c}>
            <button
              type="button"
              className="problems-code overview-line"
              aria-pressed={statusClass === c}
              aria-label={t("overview_status_label", { label: responseLabel(c), count: summary.byStatus[c] })}
              onClick={() => onStatusClass(statusClass === c ? "all" : c)}
            >
              <span>{responseLabel(c)}</span>
              <span className="n">{summary.byStatus[c]}</span>
            </button>
          </li>
        ))}
        <li className="overview-line">
          <span>{t("overview_indexable")}</span>
          <span className="n">{summary.indexable}</span>
        </li>
      </ul>
      <h2 className="problems-title">
        {t("problems_title")} <em>{problems}</em>
      </h2>
      <div role="group" aria-label={t("grouping_label")} className="problems-grouping">
        {(["priorite", "theme"] as const).map((g) => (
          <button
            key={g}
            type="button"
            className="btn"
            aria-pressed={grouping === g}
            onClick={() => chooseGrouping(g)}
          >
            {t(`grouping_${g}` as FrKey)}
          </button>
        ))}
      </div>
      {groups.length === 0 && <p className="dim">{t("problems_none")}</p>}
      {groups.map((g) => (
        <Fragment key={g.key}>
          <h3 className={`problems-level ${g.className}`}>
            {g.className !== "problems-theme" && <span className="problems-dot" aria-hidden="true" />}
            {g.label} <em>{g.total}</em>
          </h3>
          <ul className="problems-list">{g.codes.map(codeButton)}</ul>
        </Fragment>
      ))}
    </aside>
  );
}
