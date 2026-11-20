import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterAll, afterEach, beforeAll, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlResult, CrawlStatus, Finding, Progress, UrlRow } from "../lib/types";

vi.mock("../lib/api", () => ({
  startCrawl: vi.fn(),
  stopCrawl: vi.fn(),
  loadCrawl: vi.fn(),
  onProgress: vi.fn(),
  onFinished: vi.fn(),
  exportCsv: vi.fn(),
}));

import * as api from "../lib/api";
import { problemsCsv, urlsCsv } from "../lib/csv";
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
          lang: null,
          headings: [],
          unlinkedPhones: [],
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
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalled());
  act(() => progressCb({ done: 1, lastUrl: "u", lastStatus: 200, elapsedMs: 1000 }));
  act(() => progressCb({ done: 2, lastUrl: "u", lastStatus: 200, elapsedMs: 4000 }));
  expect((await screen.findByTestId("progress")).textContent).toBe("2 URL traitées · 4 s");
});

test("arrêter appelle stopCrawl", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "arrêter" }));
  expect(api.stopCrawl).toHaveBeenCalled();
});

test("à la fin, le résultat est chargé et affiché", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValueOnce(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalledTimes(1));
  vi.mocked(api.loadCrawl).mockResolvedValueOnce(result({ kind: "completed" }));
  await act(async () => finishedCb(meta({ kind: "completed" })));
  expect(await screen.findByText("https://exemple.fr/tarifs")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "arrêter" })).toBeNull();
});

test("un crawl terminé avant les écouteurs est affiché", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText("https://exemple.fr/tarifs")).toBeTruthy();
});

test("un événement de fin d’un autre crawl est ignoré", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalledTimes(1));
  await act(async () => finishedCb(meta({ kind: "completed" }, "autre")));
  expect(api.loadCrawl).toHaveBeenCalledTimes(1);
});

test("un clic sur un en-tête trie le tableau", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(firstDataRowUrl()).toBe("https://exemple.fr/");
  await userEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  await userEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs");
});

test("Entrée et Espace sur une ligne la sélectionnent", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  const rows = screen.getAllByRole("row");
  rows[2].focus();
  await userEvent.keyboard("{Enter}");
  expect(await screen.findByText("Voir les tarifs")).toBeTruthy();
  rows[1].focus();
  await userEvent.keyboard(" ");
  await waitFor(() => expect(screen.queryByText("Voir les tarifs")).toBeNull());
  expect(rows[1].getAttribute("aria-selected")).toBe("true");
});

test("Entrée sur un en-tête trie le tableau", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  screen.getByRole("columnheader", { name: /prof\./ }).focus();
  await userEvent.keyboard("{Enter}{Enter}");
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs");
});

test("un clic sur une ligne ouvre le détail avec les liens entrants", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByText("https://exemple.fr/tarifs"));
  expect(await screen.findByText("Voir les tarifs")).toBeTruthy();
  expect(screen.getByText("Les prix")).toBeTruthy();
  expect(screen.getByText("Product")).toBeTruthy();
});

test("un crawl arrêté affiche son bandeau", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "stopped" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText("crawl arrêté avant la fin.")).toBeTruthy();
});

test("un crawl interrompu conseille de baisser le débit", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "throttled" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText(/baissez le débit/)).toBeTruthy();
});

test("relancer démarre un crawl sur la même URL et l’ouvre", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  vi.mocked(api.startCrawl).mockResolvedValue({ domain: "exemple.fr", id: "nouvel-id" });
  const onOpen = vi.fn();
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole("button", { name: "relancer" }));
  expect(api.startCrawl).toHaveBeenCalledWith("https://exemple.fr/", meta({ kind: "completed" }).settings);
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith("exemple.fr", "nouvel-id", true));
});

test("relancer refusé affiche le message et reste sur l’écran", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  vi.mocked(api.startCrawl).mockRejectedValue("Un crawl est déjà en cours.");
  const onOpen = vi.fn();
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole("button", { name: "relancer" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Un crawl est déjà en cours.");
  expect(onOpen).not.toHaveBeenCalled();
});

test("pendant le crawl, arrêter est visible et relancer ne l’est pas", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByRole("button", { name: "arrêter" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "relancer" })).toBeNull();
});

test("le tableau ne rend qu’une fenêtre de lignes", async () => {
  const r = result({ kind: "completed" });
  r.rows = Array.from({ length: 2000 }, (_, i) => row(`https://exemple.fr/p${i}`));
  vi.mocked(api.loadCrawl).mockResolvedValue(r);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/p0");
  expect(screen.getAllByRole("row").length).toBeLessThan(200);
});

function withFindings(findings: Finding[], startUrl?: string): CrawlResult {
  const r = result({ kind: "completed" });
  r.findings = findings;
  if (startUrl) r.meta.startUrl = startUrl;
  return r;
}

const TARIFS_FINDINGS: Finding[] = [
  { code: "metaDescriptionTooShort", url: "https://exemple.fr/tarifs", related: [], detail: "42 caractères" },
  { code: "sitemapNotFound", url: "https://exemple.fr/", related: [], detail: null },
];

test("un clic sur un code filtre le tableau sur ses URL", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(withFindings(TARIFS_FINDINGS));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(screen.getAllByRole("row")).toHaveLength(3);
  await userEvent.click(screen.getByRole("button", { name: "Meta description trop courte (moins de 70 caractères), 1" }));
  expect(screen.getAllByRole("row")).toHaveLength(2);
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs");
  const filter = screen.getByRole("status", { name: "filtre actif" });
  expect(filter.textContent).toContain("Meta description trop courte");
  await userEvent.click(screen.getByRole("button", { name: "retirer le filtre" }));
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect(screen.queryByRole("status", { name: "filtre actif" })).toBeNull();
});

test("les URL liées d’un constat entrent dans le filtre", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(
    withFindings([
      {
        code: "duplicateContent",
        url: "https://exemple.fr/",
        related: ["https://exemple.fr/tarifs", "https://autre.fr/hors-crawl"],
        detail: null,
      },
    ]),
  );
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  await userEvent.click(screen.getByRole("button", { name: /^Contenu identique/ }));
  expect(screen.getAllByRole("row")).toHaveLength(3);
});

test("un constat de site filtre sur l’URL de départ, écrite autrement que la ligne", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(
    withFindings([{ code: "sitemapNotFound", url: "https://Exemple.FR", related: [], detail: null }], "https://Exemple.FR"),
  );
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  await userEvent.click(screen.getByRole("button", { name: /^Aucun sitemap/ }));
  expect(screen.getAllByRole("row")).toHaveLength(2);
  expect(firstDataRowUrl()).toBe("https://exemple.fr/");
});

test("un constat de site sans ligne correspondante explique le tableau vide", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(
    withFindings([{ code: "httpsInvalid", url: "http://exemple.fr/", related: [], detail: null }], "http://exemple.fr/"),
  );
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  await userEvent.click(screen.getByRole("button", { name: /^Certificat HTTPS invalide/ }));
  expect(screen.getByText(/le filtre de problème est actif/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "retirer le filtre" })).toBeTruthy();
});

test("le détail d’une URL liste ses problèmes avec le pourquoi et la correction", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(withFindings(TARIFS_FINDINGS));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("row", { name: /tarifs/ }));
  const detail = screen.getByRole("region", { name: "détail de https://exemple.fr/tarifs" });
  expect(within(detail).getByText("Meta description trop courte (moins de 70 caractères)")).toBeTruthy();
  expect(within(detail).getByText("L'extrait donne peu envie de cliquer.")).toBeTruthy();
  expect(within(detail).getByText("La compléter.")).toBeTruthy();
  expect(within(detail).getByText("42 caractères")).toBeTruthy();
  expect(within(detail).queryByText(/Sitemap/)).toBeNull();
});

test("le détail d’une URL sans problème l’indique", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(withFindings([]));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("row", { name: /tarifs/ }));
  expect(screen.getByText("aucun problème sur cette URL.")).toBeTruthy();
});

test("les boutons d’export n’apparaissent qu’avec un résultat chargé", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  const { unmount } = render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByRole("button", { name: "arrêter" });
  expect(screen.queryByRole("button", { name: "exporter les URL" })).toBeNull();
  expect(screen.queryByRole("button", { name: "exporter les problèmes" })).toBeNull();
  unmount();

  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByRole("button", { name: "exporter les URL" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "exporter les problèmes" })).toBeTruthy();
});

test("exporter les URL propose <domaine>-<id>-urls.csv avec toutes les lignes", async () => {
  const r = result({ kind: "completed" });
  vi.mocked(api.loadCrawl).mockResolvedValue(r);
  vi.mocked(api.exportCsv).mockResolvedValue(true);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "exporter les URL" }));
  expect(api.exportCsv).toHaveBeenCalledWith("exemple.fr-abc-urls.csv", urlsCsv(r.rows));
  expect(screen.queryByRole("alert")).toBeNull();
});

test("exporter les problèmes propose <domaine>-<id>-problemes.csv", async () => {
  const r = result({ kind: "completed" });
  r.findings = [{ code: "titleMissing", url: "https://exemple.fr/", related: [], detail: null }];
  vi.mocked(api.loadCrawl).mockResolvedValue(r);
  vi.mocked(api.exportCsv).mockResolvedValue(true);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "exporter les problèmes" }));
  expect(api.exportCsv).toHaveBeenCalledWith("exemple.fr-abc-problemes.csv", problemsCsv(r.findings));
});

test("un export en échec affiche l’erreur", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  vi.mocked(api.exportCsv).mockRejectedValue("impossible d’écrire le fichier : disque plein");
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "exporter les URL" }));
  expect((await screen.findByRole("alert")).textContent).toContain("disque plein");
});

test("une boîte de dialogue annulée n’affiche rien", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  vi.mocked(api.exportCsv).mockResolvedValue(false);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await userEvent.click(await screen.findByRole("button", { name: "exporter les URL" }));
  await waitFor(() => expect(api.exportCsv).toHaveBeenCalled());
  expect(screen.queryByRole("alert")).toBeNull();
});
