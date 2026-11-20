import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlStatus, SiteSummary } from "../lib/types";

vi.mock("../lib/api", () => ({
  listSites: vi.fn(),
  startCrawl: vi.fn(),
}));

import * as api from "../lib/api";
import Home from "./Home";

function meta(
  id: string,
  startedAt: string,
  pages: number,
  status: CrawlStatus = { kind: "completed" },
): CrawlMeta {
  return {
    id,
    domain: "exemple.fr",
    startUrl: "https://exemple.fr/",
    startedAt,
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
    pages,
  };
}

afterEach(cleanup);

beforeEach(() => {
  vi.mocked(api.listSites).mockResolvedValue([]);
});

test("Accueil affiche le message d’erreur de lancement", async () => {
  vi.mocked(api.startCrawl).mockRejectedValue("Adresse invalide : x");
  render(<Home onOpen={() => {}} onSettings={() => {}} />);
  await userEvent.type(screen.getByRole("textbox"), "x");
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  expect(await screen.findByText("Adresse invalide : x")).toBeTruthy();
});

test("Accueil ouvre le crawl lancé", async () => {
  vi.mocked(api.startCrawl).mockResolvedValue({ domain: "exemple.fr", id: "abc" });
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} />);
  await userEvent.type(screen.getByRole("textbox"), "exemple.fr");
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith("exemple.fr", "abc", true));
});

test("Accueil liste les crawls du plus récent au plus ancien", async () => {
  const sites: SiteSummary[] = [
    {
      domain: "exemple.fr",
      crawls: [
        meta("vieux", "2026-01-01T10:00:00Z", 11),
        meta("recent", "2026-03-01T10:00:00Z", 22),
        meta("milieu", "2026-02-01T10:00:00Z", 33),
      ],
    },
  ];
  vi.mocked(api.listSites).mockResolvedValue(sites);
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} />);
  const items = await screen.findAllByRole("button", { name: /URL/ });
  expect(items.map((b) => /(\d+) URL/.exec(b.textContent ?? "")?.[1])).toEqual(["22", "33", "11"]);
  expect(items[0].textContent).toContain("terminé");
  await userEvent.click(items[0]);
  expect(onOpen).toHaveBeenCalledWith("exemple.fr", "recent", false);
});

test("Accueil rouvre un crawl en cours avec sa progression", async () => {
  const sites: SiteSummary[] = [
    {
      domain: "exemple.fr",
      crawls: [
        meta("fini", "2026-01-01T10:00:00Z", 11),
        meta("actif", "2026-03-01T10:00:00Z", 0, { kind: "running" }),
      ],
    },
  ];
  vi.mocked(api.listSites).mockResolvedValue(sites);
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} />);
  const items = await screen.findAllByRole("button", { name: /URL/ });
  expect(items[0].textContent).toContain("en cours");
  await userEvent.click(items[0]);
  expect(onOpen).toHaveBeenLastCalledWith("exemple.fr", "actif", true);
  await userEvent.click(items[1]);
  expect(onOpen).toHaveBeenLastCalledWith("exemple.fr", "fini", false);
});

test("chaque crawl de l’accueil est un bouton « ouvrir » accessible au clavier", async () => {
  vi.mocked(api.listSites).mockResolvedValue([
    {
      domain: "exemple.fr",
      crawls: [meta("a", "2026-01-01T10:00:00Z", 1), meta("b", "2026-02-01T10:00:00Z", 2), meta("c", "2026-03-01T10:00:00Z", 3)],
    },
  ]);
  render(<Home onOpen={() => {}} onSettings={() => {}} />);
  const rows = await screen.findAllByRole("button", { name: /ouvrir/ });
  expect(rows.length).toBe(3);
});

test("l’en-tête d’un site affiche le nombre de crawls", async () => {
  vi.mocked(api.listSites).mockResolvedValue([
    { domain: "un.fr", crawls: [meta("a", "2026-01-01T10:00:00Z", 1)] },
    {
      domain: "trois.fr",
      crawls: [meta("b", "2026-01-01T10:00:00Z", 1), meta("c", "2026-02-01T10:00:00Z", 2), meta("d", "2026-03-01T10:00:00Z", 3)],
    },
  ]);
  render(<Home onOpen={() => {}} onSettings={() => {}} />);
  expect(await screen.findByText("1 crawl")).toBeTruthy();
  expect(screen.getByText("3 crawls")).toBeTruthy();
});

test("Accueil : le lien réglages appelle onSettings", async () => {
  const onSettings = vi.fn();
  render(<Home onOpen={() => {}} onSettings={onSettings} />);
  await userEvent.click(screen.getByRole("button", { name: "réglages" }));
  expect(onSettings).toHaveBeenCalled();
});
