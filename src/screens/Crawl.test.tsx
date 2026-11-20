import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterEach, beforeAll, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlResult, CrawlStatus, Progress, UrlRow } from "../lib/types";

vi.mock("../lib/api", () => ({
  stopCrawl: vi.fn(),
  loadCrawl: vi.fn(),
  onProgress: vi.fn(),
  onFinished: vi.fn(),
}));

import * as api from "../lib/api";
import { Crawl } from "./Crawl";

let progressCb: (p: Progress) => void = () => {};
let finishedCb: (m: CrawlMeta) => void = () => {};

function meta(status: CrawlStatus, id = "abc"): CrawlMeta {
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
      userAgent: null,
      includeRegex: [],
      ignoreRegex: [],
      limitThreshold: 5,
    },
    pages: 2,
  };
}

function row(url: string, over: Partial<UrlRow> = {}): UrlRow {
  return {
    url,
    status: 200,
    contentType: "text/html",
    title: null,
    metaDescription: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable: true,
    depth: 0,
    inlinks: 0,
    wordCount: 10,
    size: null,
    ...over,
  };
}

function result(status: CrawlStatus): CrawlResult {
  return {
    meta: meta(status),
    rows: [
      row("https://exemple.fr/", { title: "Accueil", h1: "Bienvenue" }),
      row("https://exemple.fr/tarifs", { title: "Tarifs", h1: "Nos tarifs", depth: 1, inlinks: 1 }),
    ],
    findings: [],
    pages: [
      {
        url: "https://exemple.fr/tarifs",
        status: 200,
        contentType: "text/html",
        size: 10,
        elapsedMs: 5,
        headers: { server: "nginx" },
        facts: {
          title: "Tarifs",
          metaDescription: "Les prix",
          h1: ["Nos tarifs"],
          canonical: null,
          metaRobots: null,
          links: [],
          images: [],
          jsonLd: [{ valid: true, types: ["Product"] }],
          wordCount: 10,
          textHash: "x",
        },
      },
    ],
    links: {
      inlinks: {
        "https://exemple.fr/tarifs": [{ from: "https://exemple.fr/", anchor: "Voir les tarifs", nofollow: false }],
      },
      depth: {},
      redirectChains: {},
    },
    sitemap: { found: false, sources: [], urls: [], truncated: false, errors: [], robotsTxt: null },
  };
}

function firstDataRowUrl(): string {
  const rows = screen.getAllByRole("row");
  return within(rows[1]).getAllByRole("cell")[0].textContent ?? "";
}

// jsdom ne mesure aucune mise en page : le virtualiseur lit clientHeight/clientWidth.
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, value: 600 });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, value: 1000 });
});

afterEach(cleanup);

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.onProgress).mockImplementation(async (cb) => {
    progressCb = cb;
    return () => {};
  });
  vi.mocked(api.onFinished).mockImplementation(async (cb) => {
    finishedCb = cb;
    return () => {};
  });
  vi.mocked(api.stopCrawl).mockResolvedValue(true);
});

test("la progression met à jour le compteur", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalled());
  act(() => progressCb({ done: 1, lastUrl: "u", lastStatus: 200, elapsedMs: 1000 }));
  act(() => progressCb({ done: 2, lastUrl: "u", lastStatus: 200, elapsedMs: 4000 }));
  expect(await screen.findByText("2 URL traitées · 4 s")).toBeTruthy();
});

test("arrêter appelle stopCrawl", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "arrêter" }));
  expect(api.stopCrawl).toHaveBeenCalled();
});

test("à la fin, le résultat est chargé et affiché", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValueOnce(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalledTimes(1));
  vi.mocked(api.loadCrawl).mockResolvedValueOnce(result({ kind: "completed" }));
  await act(async () => finishedCb(meta({ kind: "completed" })));
  expect(await screen.findByText("https://exemple.fr/tarifs")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "arrêter" })).toBeNull();
});

test("un crawl terminé avant les écouteurs est affiché", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} />);
  expect(await screen.findByText("https://exemple.fr/tarifs")).toBeTruthy();
});

test("un événement de fin d’un autre crawl est ignoré", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalledTimes(1));
  await act(async () => finishedCb(meta({ kind: "completed" }, "autre")));
  expect(api.loadCrawl).toHaveBeenCalledTimes(1);
});

test("un clic sur un en-tête trie le tableau", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(firstDataRowUrl()).toBe("https://exemple.fr/");
  await userEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  await userEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs");
});

test("un clic sur une ligne ouvre le détail avec les liens entrants", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} />);
  await userEvent.click(await screen.findByText("https://exemple.fr/tarifs"));
  expect(await screen.findByText("Voir les tarifs")).toBeTruthy();
  expect(screen.getByText("Les prix")).toBeTruthy();
  expect(screen.getByText("Product")).toBeTruthy();
});

test("un crawl arrêté affiche son bandeau", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "stopped" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} />);
  expect(await screen.findByText("crawl arrêté avant la fin.")).toBeTruthy();
});

test("un crawl interrompu conseille de baisser le débit", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "throttled" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} />);
  expect(await screen.findByText(/baissez le débit/)).toBeTruthy();
});
