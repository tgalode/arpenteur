import { useEffect, useState, type FormEvent } from "react";
import { t, type fr } from "../i18n/fr";
import { version } from "../../package.json";
import { listSites, startCrawl } from "../lib/api";
import { statusTone } from "../lib/table";
import type { CrawlMeta, SiteSummary } from "../lib/types";

const dateFormat = new Intl.DateTimeFormat("fr-FR", { dateStyle: "short", timeStyle: "short" });

function statusLabel(meta: CrawlMeta): string {
  return t(`status_${meta.status.kind}` as keyof typeof fr);
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateFormat.format(d);
}

export default function Home({
  onOpen,
  onSettings,
}: {
  onOpen: (domain: string, id: string, running: boolean) => void;
  onSettings: () => void;
}) {
  const links = [{ label: t("settings"), onClick: onSettings }];
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
      <header className="topbar">
        <h1 className="logo">{t("appName")}</h1>
        <span className="dim">{t("version", { version })}</span>
        <nav className="topbar-links">
          {links.map((l) => (
            <button key={l.label} type="button" className="btn" onClick={l.onClick}>
              {l.label}
            </button>
          ))}
        </nav>
      </header>
      <div className="home-body">
        <form className="home-new" onSubmit={submit}>
          <input
            type="text"
            className="field"
            aria-label={t("urlLabel")}
            placeholder={t("urlPlaceholder")}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {t("launch")}
          </button>
        </form>
        {error && (
          <p role="alert" className="home-error">
            {error}
          </p>
        )}
        <h2 className="home-h dim">{t("sites")}</h2>
        {sites.length === 0 && <p className="dim">{t("noSites")}</p>}
        {sites.map((site) => (
          <section key={site.domain} className="site">
            <header className="site-head">
              <h3 className="strong">{site.domain}</h3>
              <span className="dim">
                {site.crawls.length === 1 ? t("crawlsOne") : t("crawlsMany", { count: site.crawls.length })}
              </span>
            </header>
            <ul>
              {[...site.crawls]
                .sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt))
                .map((c) => (
                  <li key={c.id}>
                    <button
                      type="button"
                      className="crawl-row"
                      onClick={() => onOpen(site.domain, c.id, c.status.kind === "running")}
                    >
                      <span className="crawl-date">{formatDate(c.startedAt)}</span>
                      <span className={`crawl-status tone-${statusTone(c.status.kind)}`} title={statusLabel(c)}>{statusLabel(c)}</span>
                      <span className="dim">{t("pages", { count: c.pages })}</span>
                      <span className="crawl-open dim">{t("open")}</span>
                    </button>
                  </li>
                ))}
            </ul>
          </section>
        ))}
      </div>
    </main>
  );
}
