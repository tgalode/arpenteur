import { t } from "../i18n/fr";
import { CATALOG, findingFix, findingTitle, findingWhy, LEVELS } from "../lib/catalog";
import { normalizeUrl } from "../lib/urls";
import type { CrawlResult, ExternalCheck, Finding, FindingCode, UrlRow } from "../lib/types";
import { levelLabel } from "./ProblemsPanel";

// Codes dont les URL liées sont les pages qui portent le problème (le constat est sur la cible).
// Pour tous les autres, les URL liées sont l'autre bout du constat : elles n'en héritent pas.
const CARRIER_CODES: Partial<Record<FindingCode, "detail_problem_link_to" | "detail_problem_image">> = {
  brokenInternalLink: "detail_problem_link_to",
  imageTooHeavy: "detail_problem_image",
  brokenExternalLink: "detail_problem_link_to",
};

interface Problem {
  finding: Finding;
  /** Adresse à montrer quand le constat est porté par une autre URL que la ligne. */
  target: string | null;
}

function problemsOf(url: string, findings: Finding[]): Problem[] {
  const key = normalizeUrl(url);
  const seen = new Set<string>();
  const out: Problem[] = [];
  for (const finding of findings) {
    const own = normalizeUrl(finding.url) === key;
    const carried = !own && finding.code in CARRIER_CODES && finding.related.some((u) => normalizeUrl(u) === key);
    if (!own && !carried) continue;
    const id = `${finding.code}|${normalizeUrl(finding.url)}`;
    if (seen.has(id)) continue;
    seen.add(id);
    out.push({ finding, target: carried ? finding.url : null });
  }
  return out.sort((a, b) => LEVELS.indexOf(CATALOG[a.finding.code].level) - LEVELS.indexOf(CATALOG[b.finding.code].level));
}

function externalStatus(check: ExternalCheck | undefined): string | null {
  if (!check) return null;
  if (check.status === null) return check.outcome === "broken" ? t("detail_externalStatus_unreachable") : null;
  return String(check.status);
}

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

  const externals = new Map(result.externalLinks.map((c) => [c.url, c]));

  const problems = problemsOf(row.url, result.findings);

  return (
    <section aria-label={t("detail_label", { url: row.url })} tabIndex={0} className="detail">
      <h2 className="strong">{row.url}</h2>
      {row.offList && <p className="dim">{t("detail_offList")}</p>}
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
          {problems.map(({ finding: f, target }, i) => {
            const level = CATALOG[f.code].level;
            const fix = findingFix(f.code);
            return (
              <li key={i} className={`lv-${level}`}>
                <p>
                  <span className="problems-dot" aria-hidden="true" />
                  <span className="strong">{findingTitle(f.code)}</span> <span className="dim">{levelLabel(level)}</span>
                </p>
                {target && (
                  <p className="detail-problem-detail">{t(CARRIER_CODES[f.code]!, { url: target })}</p>
                )}
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
        {outlinks.map((l, i) => {
          const status = externalStatus(externals.get(normalizeUrl(l.href)));
          return (
            <li key={i}>
              <span>{l.href}</span> <span>{l.anchor || t("none")}</span>
              {status && <span className="dim"> {status}</span>}
            </li>
          );
        })}
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
