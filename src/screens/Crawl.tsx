import { useEffect, useRef, useState } from "react";
import { UrlDetail } from "../components/UrlDetail";
import { UrlTable } from "../components/UrlTable";
import { formatElapsed, t } from "../i18n/fr";
import { loadCrawl, onFinished, onProgress, startCrawl, stopCrawl } from "../lib/api";
import { statusTone } from "../lib/table";
import type { CrawlResult, CrawlStatus, Progress, UrlRow } from "../lib/types";

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

  const message = result ? banner(result.meta.status) : null;

  async function restart() {
    if (!result) return;
    setRestarting(true);
    try {
      const started = await startCrawl(result.meta.startUrl);
      onOpen(started.domain, started.id, true);
    } catch (e) {
      setError(String(e));
      setRestarting(false);
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
          <button type="button" className="btn btn-primary" disabled={restarting} onClick={() => void restart()}>
            {t("restart")}
          </button>
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
                      <em>{progress.done}</em>
                      {t("progress", { done: "", elapsed: formatElapsed(progress.elapsedMs) })}
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
          <UrlTable rows={result.rows} selectedUrl={selected?.url ?? null} onSelect={setSelected} />
          {selected && <UrlDetail row={selected} result={result} />}
        </>
      )}
    </main>
  );
}
