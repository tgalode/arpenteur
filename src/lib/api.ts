import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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

export function stopCrawl(): Promise<boolean> {
  return invoke<boolean>("stop_crawl");
}

export function listSites(): Promise<SiteSummary[]> {
  return invoke<SiteSummary[]>("list_sites");
}

export function loadCrawl(domain: string, id: string): Promise<CrawlResult> {
  return invoke<CrawlResult>("load_crawl", { domain, id });
}

export function onProgress(cb: (p: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("crawl-progress", (e) => cb(e.payload));
}

export function onFinished(cb: (m: CrawlMeta) => void): Promise<UnlistenFn> {
  return listen<CrawlMeta>("crawl-finished", (e) => cb(e.payload));
}
