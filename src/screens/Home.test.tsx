import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, SiteSummary } from "../lib/types";

vi.mock("../lib/api", () => ({
  listSites: vi.fn(),
  startCrawl: vi.fn(),
}));

import * as api from "../lib/api";
import Home from "./Home";

function meta(id: string, startedAt: string, pages: number): CrawlMeta {
  return {
    id,
    domain: "exemple.fr",
    startUrl: "https://exemple.fr/",
    startedAt,
    finishedAt: null,
    status: { kind: "completed" },
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
  render(<Home onOpen={() => {}} />);
  await userEvent.type(screen.getByRole("textbox"), "x");
  await userEvent.click(screen.getByRole("button", { name: "Lancer" }));
  expect(await screen.findByText("Adresse invalide : x")).toBeTruthy();
});

test("Accueil ouvre le crawl lancé", async () => {
  vi.mocked(api.startCrawl).mockResolvedValue({ domain: "exemple.fr", id: "abc" });
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} />);
  await userEvent.type(screen.getByRole("textbox"), "exemple.fr");
  await userEvent.click(screen.getByRole("button", { name: "Lancer" }));
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
  render(<Home onOpen={onOpen} />);
  const items = await screen.findAllByRole("button", { name: /pages/ });
  expect(items.map((b) => /(\d+) pages/.exec(b.textContent ?? "")?.[1])).toEqual(["22", "33", "11"]);
  expect(items[0].textContent).toContain("Terminé");
  await userEvent.click(items[0]);
  expect(onOpen).toHaveBeenCalledWith("exemple.fr", "recent", false);
});
