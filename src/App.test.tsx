import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlResult, CrawlStatus, SiteSummary } from "./lib/types";

vi.mock("./lib/api", () => ({
  listSites: vi.fn(),
  startCrawl: vi.fn(),
  stopCrawl: vi.fn(),
  loadCrawl: vi.fn(),
  onProgress: vi.fn(),
  onFinished: vi.fn(),
}));

import * as api from "./lib/api";
import App from "./App";

function meta(id: string, status: CrawlStatus): CrawlMeta {
  return {
    id,
    domain: "exemple.fr",
    startUrl: "https://exemple.fr/",
    startedAt: "2026-01-01T10:00:00Z",
    finishedAt: null,
    status,
    settings: {
      workers: 1,
      maxReqsPerSec: 2,
      maxRequests: 10000,
      respectRobots: true,
      checkExternalLinks: true,
      userAgent: null,
      includeRegex: [],
      ignoreRegex: [],
      limitThreshold: 5,
    },
    pages: 0,
    mode: "site",
    listUrls: [],
  };
}

function result(id: string, status: CrawlStatus): CrawlResult {
  return {
    meta: meta(id, status),
    rows: [],
    findings: [],
    pages: [],
    links: { inlinks: {}, depth: {}, redirectChains: {} },
    sitemap: { found: false, sources: [], urls: [], truncated: false, errors: [], robotsTxt: null, robotsStatus: null },
    externalLinks: [],
    externalUnchecked: 0,
    listSkipped: [],
  };
}

const originalHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");
const originalWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, value: 600 });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, value: 1000 });
});

afterAll(() => {
  for (const [key, d] of [
    ["clientHeight", originalHeight],
    ["clientWidth", originalWidth],
  ] as const) {
    if (d) Object.defineProperty(HTMLElement.prototype, key, d);
    else delete (HTMLElement.prototype as unknown as Record<string, unknown>)[key];
  }
});

afterEach(cleanup);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.onProgress).mockResolvedValue(() => {});
  vi.mocked(api.onFinished).mockResolvedValue(() => {});
});

test("relancer remonte l’écran : le nouveau crawl affiche sa progression", async () => {
  const sites: SiteSummary[] = [{ domain: "exemple.fr", crawls: [meta("ancien", { kind: "completed" })] }];
  vi.mocked(api.listSites).mockResolvedValue(sites);
  vi.mocked(api.loadCrawl).mockImplementation(async (_d, id) =>
    id === "nouvel-id" ? result(id, { kind: "running" }) : result(id, { kind: "completed" }),
  );
  vi.mocked(api.startCrawl).mockResolvedValue({ domain: "exemple.fr", id: "nouvel-id" });

  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /ouvrir/ }));
  await userEvent.click(await screen.findByRole("button", { name: "relancer" }));

  expect(await screen.findByRole("button", { name: "arrêter" })).toBeTruthy();
  await waitFor(() => expect(screen.queryByRole("button", { name: "relancer" })).toBeNull());
});

test("à propos s’ouvre depuis l’accueil et on en revient", async () => {
  vi.mocked(api.listSites).mockResolvedValue([]);
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "à propos" }));
  expect(screen.getByText(/Permission is hereby granted/)).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "retour" }));
  expect(await screen.findByRole("button", { name: "à propos" })).toBeTruthy();
});
