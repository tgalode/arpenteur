import { Fragment } from "react";
import { t, type FrKey } from "../i18n/fr";
import { CATALOG, findingTitle, kindLabel, LEVELS, type Level } from "../lib/catalog";
import type { Finding, FindingCode } from "../lib/types";

export function levelLabel(level: Level): string {
  return t(`level_${level}` as FrKey);
}

export function ProblemsPanel({
  findings,
  selectedCode,
  onSelect,
}: {
  findings: Finding[];
  selectedCode: FindingCode | null;
  onSelect: (code: FindingCode | null) => void;
}) {
  const counts = new Map<FindingCode, number>();
  for (const f of findings) counts.set(f.code, (counts.get(f.code) ?? 0) + 1);

  const groups = LEVELS.map((level) => {
    const codes = [...counts.keys()]
      .filter((c) => CATALOG[c].level === level)
      .sort((a, b) => (counts.get(b) ?? 0) - (counts.get(a) ?? 0));
    const total = codes.reduce((sum, c) => sum + (counts.get(c) ?? 0), 0);
    return { level, codes, total };
  }).filter((g) => g.codes.length > 0);

  const problems = groups.filter((g) => g.level !== "info").reduce((sum, g) => sum + g.total, 0);

  return (
    <aside className="problems" aria-label={t("problems_label")}>
      <h2 className="problems-title">
        {t("problems_title")} <em>{problems}</em>
      </h2>
      {groups.length === 0 && <p className="dim">{t("problems_none")}</p>}
      {groups.map((g) => (
        <Fragment key={g.level}>
          <h3 className={`problems-level lv-${g.level}`}>
            <span className="problems-dot" aria-hidden="true" />
            {levelLabel(g.level)} <em>{g.total}</em>
          </h3>
          <ul className="problems-list">
            {g.codes.map((code) => {
              const count = counts.get(code) ?? 0;
              const title = findingTitle(code);
              const kind = kindLabel(CATALOG[code].kind);
              return (
                <li key={code}>
                  <button
                    type="button"
                    className="problems-code"
                    aria-pressed={code === selectedCode}
                    aria-label={t("problems_code_label", { title, kind, count })}
                    onClick={() => onSelect(code === selectedCode ? null : code)}
                  >
                    <span>
                      {title} <span className="problems-kind dim">{kind}</span>
                    </span>
                    <span className="n">{count}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </Fragment>
      ))}
    </aside>
  );
}
