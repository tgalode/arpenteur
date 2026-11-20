import { useEffect, useState, type FormEvent } from "react";
import { t, type fr } from "../i18n/fr";
import { listSites, startCrawl } from "../lib/api";
import type { CrawlMeta, SiteSummary } from "../lib/types";

const dateFormat = new Intl.DateTimeFormat("fr-FR", { dateStyle: "short", timeStyle: "short" });

function statusLabel(meta: CrawlMeta): string {
  return t(`status_${meta.status.kind}` as keyof typeof fr);
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateFormat.format(d);
}

export default function Home({ onOpen }: { onOpen: (domain: string, id: string, running: boolean) => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sites, setSites] = useState<SiteSummary[]>([]);

  useEffect(() => {
    let alive = true;
    listSites()
      .then((s) => alive && setSites(s))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, []);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const { domain, id } = await startCrawl(url);
      onOpen(domain, id, true);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="home">
      <h1>{t("appName")}</h1>
      <form onSubmit={submit}>
        <input
          type="text"
          aria-label={t("urlLabel")}
          placeholder={t("urlPlaceholder")}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
        <button type="submit" disabled={busy}>
          {t("launch")}
        </button>
      </form>
      {error && <p role="alert">{error}</p>}
      <h2>{t("sites")}</h2>
      {sites.length === 0 && <p>{t("noSites")}</p>}
      {sites.map((site) => (
        <section key={site.domain}>
          <h3>{site.domain}</h3>
          <ul>
            {[...site.crawls]
              .sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt))
              .map((c) => (
                <li key={c.id}>
                  <button type="button" onClick={() => onOpen(site.domain, c.id, false)}>
                    {formatDate(c.startedAt)} · {statusLabel(c)} · {t("pages", { count: c.pages })}
                  </button>
                </li>
              ))}
          </ul>
        </section>
      ))}
    </main>
  );
}
