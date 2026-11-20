import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  CrawlMeta,
  CrawlResult,
  CrawlSettings,
  Progress,
  SiteSummary,
  StartedCrawl,
} from "./types";

export function startCrawl(url: string, settings?: Partial<CrawlSettings>): Promise<StartedCrawl> {
  return invoke<StartedCrawl>("start_crawl", { url, settings: settings ?? null });
}

export function getSettings(): Promise<CrawlSettings> {
  return invoke<CrawlSettings>("get_settings");
}

/** Rejette avec la liste des messages de validation si un réglage est refusé. */
export function saveSettings(settings: CrawlSettings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

export function stopCrawl(): Promise<boolean> {
  return invoke<boolean>("stop_crawl");
}

export function listSites(): Promise<SiteSummary[]> {
  return invoke<SiteSummary[]>("list_sites");
}

export function loadCrawl(domain: string, id: string): Promise<CrawlResult> {
  return invoke<CrawlResult>("load_crawl", { domain, id });
}

/** Ouvre « Enregistrer sous » puis écrit le fichier. `false` si l'utilisateur annule. */
export async function exportCsv(defaultName: string, contents: string): Promise<boolean> {
  const path = await save({ defaultPath: defaultName, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (path === null) return false;
  await invoke<void>("export_file", { path, contents });
  return true;
}

export function onProgress(cb: (p: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("crawl-progress", (e) => cb(e.payload));
}

export function onFinished(cb: (m: CrawlMeta) => void): Promise<UnlistenFn> {
  return listen<CrawlMeta>("crawl-finished", (e) => cb(e.payload));
}
