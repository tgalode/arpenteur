import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlStatus, SiteSummary } from "../lib/types";

vi.mock("../lib/api", () => ({
  listSites: vi.fn(),
  startCrawl: vi.fn(),
  startListCrawl: vi.fn(),
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
      checkExternalLinks: true,
      userAgent: null,
      includeRegex: [],
      ignoreRegex: [],
      limitThreshold: 5,
    },
    pages,
    mode: "site",
    listUrls: [],
  };
}

afterEach(cleanup);

beforeEach(() => {
  vi.mocked(api.listSites).mockResolvedValue([]);
});

test("Accueil affiche le message d’erreur de lancement", async () => {
  vi.mocked(api.startCrawl).mockRejectedValue("Adresse invalide : x");
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
  await userEvent.type(screen.getByRole("textbox"), "x");
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  expect(await screen.findByText("Adresse invalide : x")).toBeTruthy();
});

test("Accueil ouvre le crawl lancé", async () => {
  vi.mocked(api.startCrawl).mockResolvedValue({ domain: "exemple.fr", id: "abc" });
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} onAbout={() => {}} />);
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
  render(<Home onOpen={onOpen} onSettings={() => {}} onAbout={() => {}} />);
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
  render(<Home onOpen={onOpen} onSettings={() => {}} onAbout={() => {}} />);
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
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
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
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
  expect(await screen.findByText("1 crawl")).toBeTruthy();
  expect(screen.getByText("3 crawls")).toBeTruthy();
});

test("Accueil : le lien réglages appelle onSettings", async () => {
  const onSettings = vi.fn();
  render(<Home onOpen={() => {}} onSettings={onSettings} onAbout={() => {}} />);
  await userEvent.click(screen.getByRole("button", { name: "réglages" }));
  expect(onSettings).toHaveBeenCalled();
});

test("Accueil : le lien à propos appelle onAbout", async () => {
  const onAbout = vi.fn();
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={onAbout} />);
  await userEvent.click(screen.getByRole("button", { name: "à propos" }));
  expect(onAbout).toHaveBeenCalled();
});

async function chooseList() {
  await userEvent.click(screen.getByRole("radio", { name: "liste d’URL" }));
}

test("l’accueil propose « site » et « liste d’URL »", async () => {
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
  const group = screen.getByRole("radiogroup", { name: "mode de crawl" });
  const site = within(group).getByRole("radio", { name: "site" }) as HTMLInputElement;
  const list = within(group).getByRole("radio", { name: "liste d’URL" }) as HTMLInputElement;
  expect(site.checked).toBe(true);
  expect(list.checked).toBe(false);
  expect(screen.getByRole("textbox", { name: "adresse du site" }).tagName).toBe("INPUT");
  await chooseList();
  expect(list.checked).toBe(true);
  expect(screen.queryByRole("textbox", { name: "adresse du site" })).toBeNull();
  expect(screen.getByRole("textbox", { name: "adresses à crawler" }).tagName).toBe("TEXTAREA");
});

test("en mode liste, le compteur suit les lignes non vides et « lancer » appelle startListCrawl", async () => {
  vi.mocked(api.startListCrawl).mockResolvedValue({ domain: "exemple.fr", id: "liste-1" });
  vi.mocked(api.startCrawl).mockClear();
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} onAbout={() => {}} />);
  await chooseList();
  const box = screen.getByRole("textbox", { name: "adresses à crawler" });
  expect(screen.getByText("0 URL")).toBeTruthy();
  await userEvent.type(box, "https://exemple.fr/{Enter}{Enter}  {Enter}https://exemple.fr/tarifs{Enter}x");
  expect(screen.getByText("3 URL")).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  expect(api.startListCrawl).toHaveBeenCalledWith("https://exemple.fr/\n\n  \nhttps://exemple.fr/tarifs\nx");
  expect(api.startCrawl).not.toHaveBeenCalled();
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith("exemple.fr", "liste-1", true));
});

test("les lignes fautives renvoyées par le cœur s’affichent, une par ligne", async () => {
  vi.mocked(api.startListCrawl).mockRejectedValue(
    "ligne 2 : « x » n'est pas une adresse http ou https\nligne 4 : « https://autre.fr/ » n'est pas sur le site exemple.fr",
  );
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
  await chooseList();
  const box = screen.getByRole("textbox", { name: "adresses à crawler" }) as HTMLTextAreaElement;
  await userEvent.type(box, "a");
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  const alert = await screen.findByRole("alert");
  const lines = within(alert).getAllByText(/^ligne/);
  expect(lines.map((l) => l.textContent)).toEqual([
    "ligne 2 : « x » n'est pas une adresse http ou https",
    "ligne 4 : « https://autre.fr/ » n'est pas sur le site exemple.fr",
  ]);
  expect(box.value).toBe("a");
});

test("une liste plus longue que la limite de requêtes est refusée avec son message", async () => {
  const message = "la liste compte 20000 adresses, la limite de requêtes est 10000";
  vi.mocked(api.startListCrawl).mockRejectedValue(message);
  const onOpen = vi.fn();
  render(<Home onOpen={onOpen} onSettings={() => {}} onAbout={() => {}} />);
  await chooseList();
  await userEvent.type(screen.getByRole("textbox", { name: "adresses à crawler" }), "https://exemple.fr/");
  await userEvent.click(screen.getByRole("button", { name: "lancer" }));
  expect((await screen.findByRole("alert")).textContent).toBe(message);
  expect(onOpen).not.toHaveBeenCalled();
});

test("la liste des crawls affiche « liste, 3 URL »", async () => {
  const list = { ...meta("l", "2026-03-01T10:00:00Z", 5), mode: "list" as const, listUrls: ["a", "b", "c"].map((p) => `https://exemple.fr/${p}`) };
  vi.mocked(api.listSites).mockResolvedValue([
    { domain: "exemple.fr", crawls: [list, meta("s", "2026-01-01T10:00:00Z", 7)] },
  ]);
  render(<Home onOpen={() => {}} onSettings={() => {}} onAbout={() => {}} />);
  const rows = await screen.findAllByRole("button", { name: /ouvrir/ });
  expect(rows[0].textContent).toContain("liste, 3 URL");
  expect(rows[0].textContent).not.toContain("5 URL");
  expect(rows[1].textContent).toContain("7 URL");
  expect(rows[1].textContent).not.toContain("liste");
});
