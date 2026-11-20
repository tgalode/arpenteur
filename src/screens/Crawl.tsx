import { useEffect, useRef, useState } from "react";
import { UrlDetail } from "../components/UrlDetail";
import { UrlTable } from "../components/UrlTable";
import { formatElapsed, t } from "../i18n/fr";
import { loadCrawl, onFinished, onProgress, stopCrawl } from "../lib/api";
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
}: {
  domain: string;
  id: string;
  running: boolean;
  onBack: () => void;
}) {
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<CrawlResult | null>(null);
  const [selected, setSelected] = useState<UrlRow | null>(null);
  const [error, setError] = useState<string | null>(null);
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

  return (
    <main>
      <div style={{ padding: "0.5rem 1rem", display: "flex", gap: "1rem", alignItems: "center" }}>
        <button type="button" onClick={onBack}>
          {t("back")}
        </button>
        <strong>{domain}</strong>
      </div>
      {error && <p role="alert">{error}</p>}
      {!result && running && (
        <div style={{ padding: "0 1rem", display: "flex", gap: "1rem", alignItems: "center" }}>
          <p>
            {progress
              ? t("progress", { done: progress.done, elapsed: formatElapsed(progress.elapsedMs) })
              : t("loading")}
          </p>
          <button type="button" onClick={() => void stopCrawl()}>
            {t("stop")}
          </button>
        </div>
      )}
      {!result && !running && !error && <p style={{ padding: "0 1rem" }}>{t("loading")}</p>}
      {result && (
        <>
          {message && (
            <p role="status" style={{ padding: "0.5rem 1rem", background: "rgba(255,170,0,0.2)" }}>
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
