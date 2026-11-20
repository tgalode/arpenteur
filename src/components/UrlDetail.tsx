import { t } from "../i18n/fr";
import { CATALOG, findingFix, findingTitle, findingWhy } from "../lib/catalog";
import { normalizeUrl } from "../lib/urls";
import type { CrawlResult, UrlRow } from "../lib/types";
import { LEVELS, levelLabel } from "./ProblemsPanel";

export function UrlDetail({ row, result }: { row: UrlRow; result: CrawlResult }) {
  const page = result.pages.find((p) => p.url === row.url);
  const facts = page?.facts ?? null;
  const inlinks = result.links.inlinks[normalizeUrl(row.url)] ?? [];
  const outlinks = facts?.links ?? [];
  const h1s = facts?.h1 ?? (row.h1 ? [row.h1] : []);
  const jsonLdTypes = (facts?.jsonLd ?? []).flatMap((j) => j.types);
  const headers = Object.entries(page?.headers ?? {});
  const canonical = facts?.canonical ?? row.canonical;
  const metaDescription = facts?.metaDescription ?? row.metaDescription;

  const key = normalizeUrl(row.url);
  const problems = result.findings
    .filter((f) => normalizeUrl(f.url) === key || f.related.some((u) => normalizeUrl(u) === key))
    .sort((a, b) => LEVELS.indexOf(CATALOG[a.code].level) - LEVELS.indexOf(CATALOG[b.code].level));

  return (
    <section aria-label={t("detail_label", { url: row.url })} tabIndex={0} className="detail">
      <h2 className="strong">{row.url}</h2>
      <dl>
        <dt>{t("detail_canonical")}</dt>
        <dd>{canonical ?? t("none")}</dd>
        <dt>{t("detail_metaDescription")}</dt>
        <dd>{metaDescription ?? t("none")}</dd>
        <dt>{t("detail_h1")}</dt>
        <dd>
          {h1s.length === 0 ? (
            t("none")
          ) : (
            <ul>
              {h1s.map((h, i) => (
                <li key={i}>{h}</li>
              ))}
            </ul>
          )}
        </dd>
        <dt>{t("detail_jsonLd")}</dt>
        <dd>
          {jsonLdTypes.length === 0 ? (
            t("none")
          ) : (
            <ul>
              {jsonLdTypes.map((ty, i) => (
                <li key={i}>{ty}</li>
              ))}
            </ul>
          )}
        </dd>
      </dl>
      <h3>
        {t("detail_problems")} ({problems.length})
      </h3>
      {problems.length === 0 ? (
        <p>{t("detail_problems_none")}</p>
      ) : (
        <ul className="detail-problems">
          {problems.map((f, i) => {
            const level = CATALOG[f.code].level;
            const fix = findingFix(f.code);
            return (
              <li key={i} className={`lv-${level}`}>
                <p>
                  <span className="problems-dot" aria-hidden="true" />
                  <span className="strong">{findingTitle(f.code)}</span> <span className="dim">{levelLabel(level)}</span>
                </p>
                {f.detail && <p className="detail-problem-detail">{f.detail}</p>}
                <p>
                  <span className="dim">{t("detail_why")} : </span>
                  {findingWhy(f.code)}
                </p>
                {fix && (
                  <p>
                    <span className="dim">{t("detail_fix")} : </span>
                    {fix}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <h3>
        {t("detail_inlinks")} ({inlinks.length})
      </h3>
      <ul>
        {inlinks.map((l, i) => (
          <li key={i}>
            <span>{l.from}</span> <span>{l.anchor || t("none")}</span>
          </li>
        ))}
      </ul>
      <h3>
        {t("detail_outlinks")} ({outlinks.length})
      </h3>
      <ul>
        {outlinks.map((l, i) => (
          <li key={i}>
            <span>{l.href}</span> <span>{l.anchor || t("none")}</span>
          </li>
        ))}
      </ul>
      <h3>{t("detail_headers")}</h3>
      <ul>
        {headers.map(([k, v]) => (
          <li key={k}>
            {k} : {v}
          </li>
        ))}
      </ul>
    </section>
  );
}
