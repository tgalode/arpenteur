import { useEffect, useRef } from "react";
import { t } from "../i18n/fr";
import type { RobotsReport } from "../lib/types";

export function RobotsView({ robots, onClose }: { robots: RobotsReport; onClose: () => void }) {
  const rootRef = useRef<HTMLElement>(null);
  useEffect(() => rootRef.current?.focus(), []);

  const unknown = robots.respected && !robots.blockedKnown;
  const none = robots.respected && robots.blockedKnown && robots.blocked.length === 0;

  return (
    <section
      ref={rootRef}
      aria-label={t("robotsButton")}
      tabIndex={-1}
      className="robots"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <header className="robots-head">
        <h2 className="strong">{t("robotsButton")}</h2>
        <button type="button" className="btn" onClick={onClose}>
          {t("robotsClose")}
        </button>
      </header>

      <h3>
        {t("robotsFileHeading")}
        {robots.status !== null && <span className="dim"> · {t("robotsStatus", { status: robots.status })}</span>}
      </h3>
      {robots.content === null ? <p className="dim">{t("robotsMissing")}</p> : <pre className="robots-file">{robots.content}</pre>}

      <h3>{t("robotsSitemapsHeading")}</h3>
      {robots.sitemaps.length === 0 ? (
        <p className="dim">{t("robotsNoSitemaps")}</p>
      ) : (
        <ul aria-label={t("robotsSitemapsHeading")}>
          {robots.sitemaps.map((url, i) => (
            <li key={`${i}|${url}`}>{url}</li>
          ))}
        </ul>
      )}

      <h3>{t("robotsBlockedHeading")}</h3>
      {!robots.respected && <p className="dim">{t("robotsNotRespected")}</p>}
      {unknown && <p className="dim">{t("robotsBlockedUnknown")}</p>}
      {none && <p className="dim">{t("robotsNoneBlocked")}</p>}
      {robots.respected && robots.blocked.length > 0 && (
        <ul aria-label={t("robotsBlockedHeading")}>
          {robots.blocked.map((b) => (
            <li key={b.url}>
              <span>{b.url}</span>
              <span className="dim">{b.foundOn ? t("robotsFoundOn", { url: b.foundOn }) : t("robotsNoSource")}</span>
            </li>
          ))}
        </ul>
      )}
      <p className="dim">{t("robotsFilesNote")}</p>
    </section>
  );
}
