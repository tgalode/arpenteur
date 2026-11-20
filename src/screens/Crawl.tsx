import { useEffect, useMemo, useRef, useState } from "react";
import { ProblemsPanel } from "../components/ProblemsPanel";
import { UrlDetail } from "../components/UrlDetail";
import { UrlTable } from "../components/UrlTable";
import { formatElapsed, t } from "../i18n/fr";
import { exportCsv, loadCrawl, onFinished, onProgress, startCrawl, stopCrawl } from "../lib/api";
import { findingTitle } from "../lib/catalog";
import { problemsCsv, urlsCsv } from "../lib/csv";
import { statusTone } from "../lib/table";
import { normalizeUrl } from "../lib/urls";
import type { CrawlResult, CrawlStatus, FindingCode, Progress, UrlRow } from "../lib/types";

function banner(status: CrawlStatus): string | null {
  switch (status.kind) {
    case "stopped":
      return t("banner_stopped");
    case "throttled":
      return t("banner_throttled");
    case "failed":
      return t("banner_failed", { message: status.message });
    default:
      return null;
  }
}

export function Crawl({
  domain,
  id,
  running,
  onBack,
  onOpen,
}: {
  domain: string;
  id: string;
  running: boolean;
  onBack: () => void;
  onOpen: (domain: string, id: string, running: boolean) => void;
}) {
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<CrawlResult | null>(null);
  const [selected, setSelected] = useState<UrlRow | null>(null);
  const [selectedCode, setSelectedCode] = useState<FindingCode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  const finished = useRef(false);

  useEffect(() => {
    let cancelled = false;
    const unlisten: Array<() => void> = [];

    const show = async () => {
      try {
        const r = await loadCrawl(domain, id);
        if (!cancelled) setResult(r);
        return r;
      } catch (e) {
        if (!cancelled) setError(String(e));
        return null;
      }
    };

    const finish = async () => {
      if (finished.current) return;
      finished.current = true;
      await show();
    };

    if (!running) {
      finished.current = true;
      void show();
    } else {
      void (async () => {
        const [offProgress, offFinished] = await Promise.all([
          onProgress((p) => {
            if (!finished.current) setProgress(p);
          }),
          onFinished((m) => {
            if (m.id === id) void finish();
          }),
        ]);
        if (cancelled) {
          offProgress();
          offFinished();
          return;
        }
        unlisten.push(offProgress, offFinished);
        try {
          const r = await loadCrawl(domain, id);
          if (cancelled || finished.current) return;
          if (r.meta.status.kind !== "running") {
            finished.current = true;
            setResult(r);
          }
        } catch (e) {
          if (!cancelled) setError(String(e));
        }
      })();
    }

    return () => {
      cancelled = true;
      unlisten.forEach((f) => f());
    };
  }, [domain, id, running]);

  const progressParts = t("progress", { elapsed: progress ? formatElapsed(progress.elapsedMs) : "" }).split("{done}");
  const message = result ? banner(result.meta.status) : null;

  const urlFilter = useMemo(() => {
    if (!result || !selectedCode) return null;
    const known = new Set(result.rows.map((r) => normalizeUrl(r.url)));
    const urls = new Set<string>();
    for (const f of result.findings) {
      if (f.code !== selectedCode) continue;
      urls.add(normalizeUrl(f.url));
      for (const u of f.related) if (known.has(normalizeUrl(u))) urls.add(normalizeUrl(u));
    }
    return urls;
  }, [result, selectedCode]);

  async function restart() {
    if (!result) return;
    setRestarting(true);
    try {
      const started = await startCrawl(result.meta.startUrl, result.meta.settings);
      onOpen(started.domain, started.id, true);
    } catch (e) {
      setError(String(e));
      setRestarting(false);
    }
  }

  async function exportFile(suffix: "urls" | "problemes", contents: string) {
    setError(null);
    try {
      await exportCsv(`${domain}-${id}-${suffix}.csv`, contents);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main className="crawl">
      <header className="topbar">
        <button type="button" className="btn crawl-back" onClick={onBack}>
          {t("back")}
        </button>
        <span className="logo">{t("appName")}</span>
        <span className="field crawl-url" title={result?.meta.startUrl ?? domain}>
          {result?.meta.startUrl ?? domain}
        </span>
        {result ? (
          <>
            {result.meta.status.kind !== "running" && (
              <>
                <button type="button" className="btn" onClick={() => void exportFile("urls", urlsCsv(result.rows))}>
                  {t("export_urls")}
                </button>
                <button
                  type="button"
                  className="btn"
                  onClick={() => void exportFile("problemes", problemsCsv(result.findings))}
                >
                  {t("export_problems")}
                </button>
              </>
            )}
            <button type="button" className="btn btn-primary" disabled={restarting} onClick={() => void restart()}>
              {t("restart")}
            </button>
          </>
        ) : (
          running && (
            <>
              <button type="button" className="btn" onClick={() => void stopCrawl()}>
                {t("stop")}
              </button>
              <span className="crawl-progress">
                {progress ? (
                  <>
                    <span className="crawl-pulse" aria-hidden="true" />
                    <span data-testid="progress">
                      {progressParts[0]}
                      <em>{progress.done}</em>
                      {progressParts[1]}
                    </span>
                  </>
                ) : (
                  t("loading")
                )}
              </span>
            </>
          )
        )}
      </header>
      {error && (
        <p role="alert" className="crawl-error">
          {error}
        </p>
      )}
      {!result && !running && !error && <p className="crawl-loading dim">{t("loading")}</p>}
      {result && (
        <>
          {message && (
            <p role="status" className={`crawl-banner tone-${statusTone(result.meta.status.kind)}`}>
              {message}
            </p>
          )}
          <div className="crawl-body">
            <ProblemsPanel findings={result.findings} selectedCode={selectedCode} onSelect={setSelectedCode} />
            <div className="crawl-main">
              {selectedCode && (
                <p role="status" aria-label={t("problems_filter_label")} className="problems-filter">
                  <span>{t("problems_filter_text", { title: findingTitle(selectedCode) })}</span>
                  <button type="button" className="btn" onClick={() => setSelectedCode(null)}>
                    {t("problems_filter_clear")}
                  </button>
                </p>
              )}
              <UrlTable
                rows={result.rows}
                selectedUrl={selected?.url ?? null}
                urlFilter={urlFilter}
                onSelect={setSelected}
              />
              {selected && <UrlDetail row={selected} result={result} />}
            </div>
          </div>
        </>
      )}
    </main>
  );
}
