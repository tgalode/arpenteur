import { t } from "../i18n/fr";
import { normalizeUrl } from "../lib/urls";
import type { CrawlResult, UrlRow } from "../lib/types";

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

  return (
    <section aria-label={row.url} className="detail">
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
