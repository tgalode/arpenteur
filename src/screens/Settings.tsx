import { useEffect, useState, type FormEvent } from "react";
import { t } from "../i18n/fr";
import { getSettings, saveSettings } from "../lib/api";
import type { CrawlSettings } from "../lib/types";

interface Form {
  workers: string;
  rate: string;
  maxRequests: string;
  limitThreshold: string;
  respectRobots: boolean;
  userAgent: string;
  include: string;
  ignore: string;
}

function toForm(s: CrawlSettings): Form {
  return {
    workers: String(s.workers),
    rate: String(s.maxReqsPerSec).replace(".", ","),
    maxRequests: String(s.maxRequests),
    limitThreshold: String(s.limitThreshold),
    respectRobots: s.respectRobots,
    userAgent: s.userAgent ?? "",
    include: s.includeRegex.join("\n"),
    ignore: s.ignoreRegex.join("\n"),
  };
}

const U32_MAX = 4_294_967_295;

/** Entier écrit en chiffres seulement ; un dépassement de u32 est ramené au maximum pour que Rust le refuse avec son message. */
function parseInteger(text: string): number {
  const clean = text.trim();
  return /^\d+$/.test(clean) ? Math.min(Number(clean), U32_MAX) : Number.NaN;
}

function parseRate(text: string): number {
  const clean = text.trim().replace(",", ".");
  return /^\d+(\.\d+)?$/.test(clean) ? Math.min(Number(clean), Number.MAX_VALUE) : Number.NaN;
}

function lines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l !== "");
}

function toSettings(form: Form): { settings?: CrawlSettings; errors: string[] } {
  const workers = parseInteger(form.workers);
  const maxReqsPerSec = parseRate(form.rate);
  const maxRequests = parseInteger(form.maxRequests);
  const limitThreshold = parseInteger(form.limitThreshold);
  const errors = [
    Number.isNaN(workers) && t("settings_notInteger", { field: t("settings_workers") }),
    Number.isNaN(maxReqsPerSec) && t("settings_notDecimal", { field: t("settings_rate") }),
    Number.isNaN(maxRequests) && t("settings_notInteger", { field: t("settings_maxRequests") }),
    Number.isNaN(limitThreshold) && t("settings_notInteger", { field: t("settings_limitThreshold") }),
  ].filter((m): m is string => m !== false);
  if (errors.length > 0) return { errors };
  const userAgent = form.userAgent.trim();
  return {
    errors,
    settings: {
      workers,
      maxReqsPerSec,
      maxRequests,
      limitThreshold,
      respectRobots: form.respectRobots,
      userAgent: userAgent === "" ? null : userAgent,
      includeRegex: lines(form.include),
      ignoreRegex: lines(form.ignore),
    },
  };
}

export default function Settings({ onBack }: { onBack: () => void }) {
  const [form, setForm] = useState<Form | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((s) => alive && setForm(toForm(s)))
      .catch((e) => alive && setErrors([String(e)]));
    return () => {
      alive = false;
    };
  }, []);

  function edit(patch: Partial<Form>) {
    setSaved(false);
    setForm((f) => (f ? { ...f, ...patch } : f));
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!form) return;
    setSaved(false);
    const parsed = toSettings(form);
    if (!parsed.settings) {
      setErrors(parsed.errors);
      return;
    }
    setBusy(true);
    try {
      await saveSettings(parsed.settings);
      setErrors([]);
      setSaved(true);
    } catch (err) {
      setErrors(Array.isArray(err) ? err.map(String) : [String(err)]);
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="settings">
      <header className="topbar">
        <button type="button" className="btn" onClick={onBack}>
          {t("back")}
        </button>
        <span className="logo">{t("appName")}</span>
        <span className="dim">{t("settings")}</span>
      </header>
      <div className="settings-body">
        <h2 className="home-h dim">{t("settings_title")}</h2>
        <p className="dim settings-intro">{t("settings_intro")}</p>
        {errors.length > 0 && (
          <div role="alert" className="home-error">
            <p>{t("settings_errors")}</p>
            <ul>
              {errors.map((m, i) => (
                <li key={i}>{m}</li>
              ))}
            </ul>
          </div>
        )}
        {form && (
          <form className="settings-form" onSubmit={submit}>
            <label className="settings-row">
              <span>{t("settings_workers")}</span>
              <input
                type="text"
                inputMode="numeric"
                className="field"
                value={form.workers}
                onChange={(e) => edit({ workers: e.target.value })}
              />
            </label>
            <label className="settings-row">
              <span>{t("settings_rate")}</span>
              <input
                type="text"
                inputMode="decimal"
                className="field"
                value={form.rate}
                onChange={(e) => edit({ rate: e.target.value })}
              />
            </label>
            <label className="settings-row">
              <span>{t("settings_maxRequests")}</span>
              <input
                type="text"
                inputMode="numeric"
                className="field"
                value={form.maxRequests}
                onChange={(e) => edit({ maxRequests: e.target.value })}
              />
            </label>
            <label className="settings-row">
              <span>{t("settings_limitThreshold")}</span>
              <input
                type="text"
                inputMode="numeric"
                className="field"
                value={form.limitThreshold}
                onChange={(e) => edit({ limitThreshold: e.target.value })}
              />
            </label>
            <label className="settings-row settings-check">
              <input
                type="checkbox"
                checked={form.respectRobots}
                onChange={(e) => edit({ respectRobots: e.target.checked })}
              />
              <span>{t("settings_respectRobots")}</span>
            </label>
            <label className="settings-row">
              <span>{t("settings_userAgent")}</span>
              <input
                type="text"
                className="field"
                value={form.userAgent}
                onChange={(e) => edit({ userAgent: e.target.value })}
              />
            </label>
            <label className="settings-row">
              <span>{t("settings_include")}</span>
              <textarea
                className="field"
                rows={4}
                value={form.include}
                onChange={(e) => edit({ include: e.target.value })}
              />
            </label>
            <label className="settings-row">
              <span>{t("settings_ignore")}</span>
              <textarea
                className="field"
                rows={4}
                value={form.ignore}
                onChange={(e) => edit({ ignore: e.target.value })}
              />
            </label>
            <div className="settings-actions">
              <button type="submit" className="btn btn-primary" disabled={busy}>
                {t("settings_save")}
              </button>
              {saved && (
                <p role="status" className="tone-ok">
                  {t("settings_saved")}
                </p>
              )}
            </div>
          </form>
        )}
      </div>
    </main>
  );
}
