import { useEffect, useMemo, useRef, useState } from "react";
import { ProblemsPanel } from "../components/ProblemsPanel";
import { RobotsView } from "../components/RobotsView";
import { UrlDetail } from "../components/UrlDetail";
import { UrlTable } from "../components/UrlTable";
import { formatElapsed, t } from "../i18n/fr";
import { exportCsv, loadCrawl, onFinished, onProgress, peekCrawl, startCrawl, startListCrawl, stopCrawl } from "../lib/api";
import { findingTitle } from "../lib/catalog";
import { problemsCsv, urlsCsv } from "../lib/csv";
import { statusTone, type StatusClass } from "../lib/table";
import { normalizeUrl } from "../lib/urls";
import type { CrawlResult, CrawlStatus, FindingCode, LiveView, Progress, SkippedUrl, UrlRow } from "../lib/types";

/** Pause entre deux instantanés : 2 s, ou quatre fois la durée du dernier s'il a dépassé 500 ms. */
const PEEK_INTERVAL_MS = 2000;
const PEEK_SLOW_MS = 500;

function skippedMessage(skipped: SkippedUrl[]): string | null {
  if (skipped.length === 0) return null;
  const robots = skipped.filter((s) => s.reason === "robots").length;
  const pattern = skipped.length - robots;
  const reasons = [
    robots > 0 ? t("skippedRobots", { count: robots }) : null,
    pattern > 0 ? t("skippedPattern", { count: pattern }) : null,
  ].filter(Boolean);
  const head = skipped.length === 1 ? t("skippedOne") : t("skippedMany", { count: skipped.length });
  return `${head} : ${reasons.join(", ")}`;
}

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
  const [view, setView] = useState<LiveView | null>(null);
  const [liveNotice, setLiveNotice] = useState(false);
  const [ended, setEnded] = useState(false);
  const [selected, setSelected] = useState<UrlRow | null>(null);
  const [selectedCode, setSelectedCode] = useState<FindingCode | null>(null);
  const [statusClass, setStatusClass] = useState<StatusClass>("all");
  const [error, setError] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  const [robotsOpen, setRobotsOpen] = useState(false);
  const robotsButton = useRef<HTMLButtonElement>(null);
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
      if (!cancelled) setEnded(true);
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

  const polling = running && result === null && !ended;
  useEffect(() => {
    if (!polling) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // Un seul instantané à la fois : le suivant n'est programmé qu'une fois le précédent réglé.
    const peek = async () => {
      const began = Date.now();
      try {
        const v = await peekCrawl();
        if (!cancelled && !finished.current && v) setView(v);
      } catch {
        // Le dernier affichage reste.
      }
      if (cancelled) return;
      const took = Date.now() - began;
      timer = setTimeout(() => void peek(), took > PEEK_SLOW_MS ? took * 4 : PEEK_INTERVAL_MS);
    };
    void peek();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [polling]);

  const rows = result?.rows ?? view?.rows ?? null;
  const findings = result?.findings ?? view?.findings ?? null;

  const progressParts = t("progress", { elapsed: progress ? formatElapsed(progress.elapsedMs) : "" }).split("{done}");
  const message = result ? banner(result.meta.status) : null;
  const skipped = result ? skippedMessage(result.listSkipped) : null;

  const urlFilter = useMemo(() => {
    if (!rows || !findings || !selectedCode) return null;
    const known = new Set(rows.map((r) => normalizeUrl(r.url)));
    const urls = new Set<string>();
    for (const f of findings) {
      if (f.code !== selectedCode) continue;
      urls.add(normalizeUrl(f.url));
      for (const u of f.related) if (known.has(normalizeUrl(u))) urls.add(normalizeUrl(u));
    }
    return urls;
  }, [rows, findings, selectedCode]);

  function selectRow(row: UrlRow) {
    if (result) setSelected(row);
    else setLiveNotice(true);
  }

  async function restart() {
    if (!result) return;
    setRestarting(true);
    try {
      const { meta } = result;
      const started = await (meta.mode === "list" ? startListCrawl(meta.listUrls.join("\n")) : startCrawl(meta.startUrl));
      onOpen(started.domain, started.id, true);
    } catch (e) {
      setError(String(e));
      setRestarting(false);
    }
  }

  function closeRobots() {
    setRobotsOpen(false);
    robotsButton.current?.focus();
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
                <button
                  ref={robotsButton}
                  type="button"
                  className="btn"
                  aria-expanded={robotsOpen}
                  onClick={() => setRobotsOpen((o) => !o)}
                >
                  {t("robotsButton")}
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
                      {progress.external ? (
                        t("progressExternal", { done: progress.external.done, total: progress.external.total })
                      ) : (
                        <>
                          {progressParts[0]}
                          <em>{progress.done}</em>
                          {progressParts[1]}
                        </>
                      )}
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
      {rows && findings && (
        <>
          {result && message && (
            <p role="status" className={`crawl-banner tone-${statusTone(result.meta.status.kind)}`}>
              {message}
            </p>
          )}
          {result && result.externalUnchecked > 0 && (
            <p role="status" className="crawl-banner tone-warn">
              {result.externalUnchecked === 1
                ? t("externalUncheckedOne")
                : t("externalUncheckedMany", { count: result.externalUnchecked })}
            </p>
          )}
          {result && skipped && (
            <p role="status" className="crawl-banner tone-warn">
              {skipped}
            </p>
          )}
          {!result && (
            <p role="status" className="crawl-banner tone-warn">
              {t("provisional")}
            </p>
          )}
          <div className="crawl-stage">
          <div className="crawl-body" inert={robotsOpen}>
            <ProblemsPanel
              findings={findings}
              rows={rows}
              selectedCode={selectedCode}
              onSelect={setSelectedCode}
              statusClass={statusClass}
              onStatusClass={setStatusClass}
            />
            <div className="crawl-main">
              {selectedCode && (
                <p role="status" aria-label={t("problems_filter_label")} className="problems-filter">
                  <span>{t("problems_filter_text", { title: findingTitle(selectedCode) })}</span>
                  <button type="button" className="btn" onClick={() => setSelectedCode(null)}>
                    {t("problems_filter_clear")}
                  </button>
                </p>
              )}
              {!result && liveNotice && (
                <p role="status" className="problems-filter dim">
                  {t("detailAfterCrawl")}
                </p>
              )}
              <UrlTable
                rows={rows}
                selectedUrl={selected?.url ?? null}
                urlFilter={urlFilter}
                onSelect={selectRow}
                statusClass={statusClass}
                onStatusClass={setStatusClass}
              />
              {result && selected && <UrlDetail row={selected} result={result} />}
            </div>
          </div>
          {result && robotsOpen && (
            <RobotsView robots={result.robots} finished={result.meta.status.kind === "completed"} onClose={closeRobots} />
          )}
          </div>
        </>
      )}
    </main>
  );
}
