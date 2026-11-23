import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { afterAll, afterEach, beforeAll, beforeEach, expect, test, vi } from "vitest";
import type { CrawlMeta, CrawlResult, CrawlStatus, Finding, LiveView, Progress, UrlRow } from "../lib/types";

vi.mock("../lib/api", () => ({
  startCrawl: vi.fn(),
  startListCrawl: vi.fn(),
  stopCrawl: vi.fn(),
  loadCrawl: vi.fn(),
  onProgress: vi.fn(),
  onFinished: vi.fn(),
  exportCsv: vi.fn(),
  peekCrawl: vi.fn(),
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
      checkExternalLinks: true,
      userAgent: null,
      includeRegex: [],
      ignoreRegex: [],
      limitThreshold: 5,
    },
    pages: 2,
    mode: "site",
    listUrls: [],
  };
}

function row(url: string, over: Partial<UrlRow> = {}): UrlRow {
  return {
    url,
    status: 200,
    contentType: "text/html",
    title: null,
    titleLength: null,
    metaDescription: null,
    metaDescriptionLength: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable: "yes",
    depth: 0,
    inlinks: 0,
    offList: false,
    wordCount: 10,
    size: null,
    elapsedMs: 0,
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
          canonicals: [],
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
        probed: false,
      },
    ],
    links: {
      inlinks: {
        "https://exemple.fr/tarifs": [{ from: "https://exemple.fr/", anchor: "Voir les tarifs", nofollow: false }],
      },
      depth: {},
      redirectChains: {},
    },
    sitemap: { found: false, sources: [], urls: [], truncated: false, errors: [], robotsTxt: null, robotsStatus: null },
    externalLinks: [],
    externalUnchecked: 0,
    listSkipped: [],
    robots: { content: null, status: null, sitemaps: [], blocked: [], blockedKnown: false, respected: true },
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

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

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
  vi.mocked(api.peekCrawl).mockResolvedValue(null);
});

test("la progression met à jour le compteur", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalled());
  act(() => progressCb({ done: 1, lastUrl: "u", lastStatus: 200, elapsedMs: 1000 }));
  act(() => progressCb({ done: 2, lastUrl: "u", lastStatus: 200, elapsedMs: 4000 }));
  expect((await screen.findByTestId("progress")).textContent).toBe("2 URL traitées · 4 s");
});

test("la phase des liens externes remplace la ligne de progression", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await waitFor(() => expect(api.loadCrawl).toHaveBeenCalled());
  act(() => progressCb({ done: 2, lastUrl: "u", lastStatus: 200, elapsedMs: 4000 }));
  expect((await screen.findByTestId("progress")).textContent).toBe("2 URL traitées · 4 s");
  act(() =>
    progressCb({ done: 2, lastUrl: "", lastStatus: 0, elapsedMs: 5000, external: { done: 12, total: 48 } }),
  );
  expect((await screen.findByTestId("progress")).textContent).toBe("vérification des liens externes : 12 / 48");
  expect(screen.getByRole("button", { name: "arrêter" })).toBeTruthy();
  // Un événement sans le champ (crawl de pages) revient à la ligne habituelle.
  act(() => progressCb({ done: 3, lastUrl: "u", lastStatus: 200, elapsedMs: 6000 }));
  expect((await screen.findByTestId("progress")).textContent).toBe("3 URL traitées · 6 s");
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

test("un crawl arrêté sans lien non vérifié n'affiche pas la limite de requêtes", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue({ ...result({ kind: "stopped" }), externalUnchecked: 0 });
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText("crawl arrêté avant la fin.")).toBeTruthy();
  expect(screen.queryByText(/limite de requêtes/)).toBeNull();
});

test("l'écran du crawl signale les liens externes non vérifiés", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue({ ...result({ kind: "completed" }), externalUnchecked: 12 });
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText("12 liens externes non vérifiés (limite de requêtes atteinte)")).toBeTruthy();
  cleanup();
  vi.mocked(api.loadCrawl).mockResolvedValue({ ...result({ kind: "completed" }), externalUnchecked: 1 });
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByText("1 lien externe non vérifié (limite de requêtes atteinte)")).toBeTruthy();
  cleanup();
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(screen.queryByText(/non vérifié/)).toBeNull();
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
  // Sans réglages : le backend applique les réglages enregistrés, pas ceux de l'ancien crawl.
  expect(api.startCrawl).toHaveBeenCalledTimes(1);
  expect(vi.mocked(api.startCrawl).mock.calls[0]).toEqual(["https://exemple.fr/"]);
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
  await userEvent.click(screen.getByRole("button", { name: "Meta description trop courte (moins de 70 caractères), opportunité, 1" }));
  expect(screen.getAllByRole("row")).toHaveLength(2);
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs");
  const filter = screen.getByRole("status", { name: "filtre actif" });
  expect(filter.textContent).toContain("Meta description trop courte");
  await userEvent.click(screen.getByRole("button", { name: "retirer le filtre" }));
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect(screen.queryByRole("status", { name: "filtre actif" })).toBeNull();
});

test("un clic sur « 4xx » du résumé filtre le tableau et la liste de statut le suit", async () => {
  const r = result({ kind: "completed" });
  r.rows = [
    row("https://exemple.fr/", { status: 200 }),
    row("https://exemple.fr/absente", { status: 404 }),
    row("https://exemple.fr/ancienne", { status: 410 }),
  ];
  vi.mocked(api.loadCrawl).mockResolvedValue(r);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/absente");
  expect(screen.getAllByRole("row")).toHaveLength(4);
  await userEvent.click(screen.getByRole("button", { name: "4xx, 2" }));
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect((screen.getByRole("combobox", { name: "classe de statut" }) as HTMLSelectElement).value).toBe("4xx");
  await userEvent.selectOptions(screen.getByRole("combobox", { name: "classe de statut" }), "all");
  expect(screen.getByRole("button", { name: "4xx, 2" }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.getAllByRole("row")).toHaveLength(4);
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

test("le bouton « robots.txt » n’apparaît que pour un crawl chargé et terminé", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  const { unmount } = render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByRole("button", { name: "arrêter" });
  expect(screen.queryByRole("button", { name: "robots.txt" })).toBeNull();
  unmount();

  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  const { unmount: unmountLoaded } = render(
    <Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />,
  );
  await screen.findByText("https://exemple.fr/tarifs");
  expect(screen.queryByRole("button", { name: "robots.txt" })).toBeNull();
  unmountLoaded();

  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "completed" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  expect(await screen.findByRole("button", { name: "robots.txt" })).toBeTruthy();
});

test("« robots.txt » ouvre la vue, Échap la ferme et le focus revient au bouton", async () => {
  const user = userEvent.setup();
  const r = result({ kind: "completed" });
  r.robots = {
    content: "User-agent: *\nDisallow: /prive\n",
    status: 200,
    sitemaps: [],
    blocked: [{ url: "https://exemple.fr/prive", foundOn: "https://exemple.fr/" }],
    blockedKnown: true,
    respected: true,
  };
  vi.mocked(api.loadCrawl).mockResolvedValue(r);
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  const button = await screen.findByRole("button", { name: "robots.txt" });
  await user.click(button);
  expect(screen.getByRole("region", { name: "robots.txt" })).toBeTruthy();
  expect(screen.getByText("https://exemple.fr/prive")).toBeTruthy();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("region", { name: "robots.txt" })).toBeNull();
  expect(document.activeElement).toBe(button);
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

test("un résultat encore en cours, chargé hors suivi, n’affiche aucun bouton d’export", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(screen.queryByRole("button", { name: "exporter les URL" })).toBeNull();
  expect(screen.queryByRole("button", { name: "exporter les problèmes" })).toBeNull();
});

function listResult(): CrawlResult {
  const r = result({ kind: "completed" });
  r.meta.mode = "list";
  r.meta.listUrls = ["https://exemple.fr/", "https://exemple.fr/tarifs"];
  return r;
}

test("l’écran du crawl dit combien d’adresses ont été écartées et pourquoi", async () => {
  const cases: Array<[Array<"robots" | "pattern">, string]> = [
    [["robots", "robots", "pattern"], "3 adresses écartées : 2 par robots.txt, 1 par un motif"],
    [["robots", "robots"], "2 adresses écartées : 2 par robots.txt"],
    [["pattern"], "1 adresse écartée : 1 par un motif"],
  ];
  for (const [reasons, text] of cases) {
    const r = listResult();
    r.listSkipped = reasons.map((reason, i) => ({ url: `https://exemple.fr/x${i}`, reason }));
    vi.mocked(api.loadCrawl).mockResolvedValue(r);
    render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
    const banner = await screen.findByText(text);
    expect(banner.getAttribute("role")).toBe("status");
    cleanup();
  }
  vi.mocked(api.loadCrawl).mockResolvedValue(listResult());
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={() => {}} />);
  await screen.findByText("https://exemple.fr/tarifs");
  expect(screen.queryByText(/écartée/)).toBeNull();
});

test("« relancer » rejoue la liste", async () => {
  vi.mocked(api.loadCrawl).mockResolvedValue(listResult());
  vi.mocked(api.startListCrawl).mockResolvedValue({ domain: "exemple.fr", id: "nouvel-id" });
  const onOpen = vi.fn();
  render(<Crawl domain="exemple.fr" id="abc" running={false} onBack={() => {}} onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole("button", { name: "relancer" }));
  expect(vi.mocked(api.startListCrawl).mock.calls).toEqual([["https://exemple.fr/\nhttps://exemple.fr/tarifs"]]);
  expect(api.startCrawl).not.toHaveBeenCalled();
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith("exemple.fr", "nouvel-id", true));
});

// Résultats pendant le crawl : minuteries simulées, l'horloge de `Date` suit.

function liveView(rows: UrlRow[], findings: Finding[] = []): LiveView {
  return { meta: meta({ kind: "running" }), rows, findings };
}

const LIVE_ROWS = [
  row("https://exemple.fr/", { title: "Accueil", h1: "Bienvenue" }),
  row("https://exemple.fr/tarifs", { title: "Tarifs", h1: "Nos tarifs", depth: 1, inlinks: 1 }),
];

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

async function renderRunning() {
  vi.useFakeTimers();
  vi.mocked(api.loadCrawl).mockResolvedValue(result({ kind: "running" }));
  render(<Crawl domain="exemple.fr" id="abc" running={true} onBack={() => {}} onOpen={() => {}} />);
  await advance(0);
}

// userEvent attend un setTimeout que les minuteries simulées de vitest ne déclenchent jamais.
function type(label: string, value: string) {
  fireEvent.change(screen.getByRole("searchbox", { name: label }), { target: { value } });
}

test("pendant le crawl, le panneau et le tableau se remplissent et « résultats provisoires » s’affiche", async () => {
  vi.mocked(api.peekCrawl).mockResolvedValue(liveView(LIVE_ROWS, TARIFS_FINDINGS));
  await renderRunning();
  expect(screen.getByText("https://exemple.fr/tarifs")).toBeTruthy();
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect(screen.getByRole("button", { name: /^Meta description trop courte/ })).toBeTruthy();
  expect(screen.getByText("résultats provisoires")).toBeTruthy();
  expect(screen.getByRole("button", { name: "arrêter" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "exporter les URL" })).toBeNull();
  expect(screen.queryByRole("button", { name: "exporter les problèmes" })).toBeNull();
});

test("un instantané est demandé toutes les 2 secondes", async () => {
  await renderRunning();
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
  await advance(1999);
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
  await advance(1);
  expect(api.peekCrawl).toHaveBeenCalledTimes(2);
  await advance(2000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(3);
});

test("un instantané de 800 ms repousse le suivant à 3,2 secondes", async () => {
  vi.mocked(api.peekCrawl).mockImplementation(
    () => new Promise((resolve) => setTimeout(() => resolve(liveView(LIVE_ROWS)), 800)),
  );
  await renderRunning();
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
  await advance(800 + 3199);
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
  await advance(1);
  expect(api.peekCrawl).toHaveBeenCalledTimes(2);
});

test("un instantané de 500 ms garde l’intervalle de 2 secondes", async () => {
  vi.mocked(api.peekCrawl).mockImplementation(
    () => new Promise((resolve) => setTimeout(() => resolve(liveView(LIVE_ROWS)), 500)),
  );
  await renderRunning();
  await advance(500 + 1999);
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
  await advance(1);
  expect(api.peekCrawl).toHaveBeenCalledTimes(2);
});

test("un instantané encore en cours n’en lance pas un second", async () => {
  vi.mocked(api.peekCrawl).mockImplementation(() => new Promise(() => {}));
  await renderRunning();
  await advance(20000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(1);
});

test("un instantané nul ou en erreur garde le dernier affichage", async () => {
  vi.mocked(api.peekCrawl)
    .mockResolvedValueOnce(liveView(LIVE_ROWS, TARIFS_FINDINGS))
    .mockResolvedValueOnce(null)
    .mockRejectedValueOnce("erreur")
    .mockResolvedValue(null);
  await renderRunning();
  expect(screen.getAllByRole("row")).toHaveLength(3);
  await advance(2000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(2);
  expect(screen.getAllByRole("row")).toHaveLength(3);
  await advance(2000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(3);
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect(screen.queryByRole("alert")).toBeNull();
  // Le suivi continue après un échec.
  await advance(2000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(4);
  expect(screen.getByText("https://exemple.fr/tarifs")).toBeTruthy();
});

test("le filtre de problème, la recherche et le tri survivent à un nouvel instantané", async () => {
  const more = [...LIVE_ROWS, row("https://exemple.fr/tarifs-pro", { depth: 2 })];
  const findings = [
    ...TARIFS_FINDINGS,
    { code: "metaDescriptionTooShort", url: "https://exemple.fr/tarifs-pro", related: [], detail: null } as Finding,
  ];
  vi.mocked(api.peekCrawl)
    .mockResolvedValueOnce(liveView(LIVE_ROWS, TARIFS_FINDINGS))
    .mockResolvedValue(liveView(more, findings));
  await renderRunning();
  fireEvent.click(screen.getByRole("button", { name: /^Meta description trop courte/ }));
  type("filtre", "tarifs");
  fireEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  fireEvent.click(screen.getByRole("columnheader", { name: /prof\./ }));
  const table = screen.getByRole("table");
  expect(screen.getAllByRole("row")).toHaveLength(2);

  await advance(2000);

  expect(api.peekCrawl).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("table")).toBe(table);
  expect(screen.getByRole("status", { name: "filtre actif" })).toBeTruthy();
  expect((screen.getByRole("searchbox", { name: "filtre" }) as HTMLInputElement).value).toBe("tarifs");
  expect(screen.getByRole("columnheader", { name: /prof\./ }).getAttribute("aria-sort")).toBe("descending");
  expect(screen.getAllByRole("row")).toHaveLength(3);
  expect(firstDataRowUrl()).toBe("https://exemple.fr/tarifs-pro");
});

test("un clic sur une ligne pendant le crawl dit que le détail s’ouvre une fois le crawl terminé", async () => {
  vi.mocked(api.peekCrawl).mockResolvedValue(liveView(LIVE_ROWS));
  await renderRunning();
  expect(screen.queryByText("le détail s’ouvre une fois le crawl terminé")).toBeNull();
  fireEvent.click(screen.getByText("https://exemple.fr/tarifs"));
  expect(screen.getByText("le détail s’ouvre une fois le crawl terminé")).toBeTruthy();
  expect(screen.queryByRole("region", { name: /détail de/ })).toBeNull();
});

test("à la fin du crawl, le résultat complet remplace l’instantané sans changer de filtre", async () => {
  vi.mocked(api.peekCrawl).mockResolvedValue(liveView(LIVE_ROWS, TARIFS_FINDINGS));
  await renderRunning();
  fireEvent.click(screen.getByRole("button", { name: /^Meta description trop courte/ }));
  type("filtre", "tarifs");
  fireEvent.change(screen.getByRole("combobox", { name: "classe de statut" }), { target: { value: "2xx" } });
  expect(screen.getByText("résultats provisoires")).toBeTruthy();

  const final = withFindings(TARIFS_FINDINGS);
  final.rows = [...final.rows, row("https://exemple.fr/tarifs-pro")];
  vi.mocked(api.loadCrawl).mockResolvedValue(final);
  await act(async () => finishedCb(meta({ kind: "completed" })));
  await advance(0);

  expect(screen.queryByText("résultats provisoires")).toBeNull();
  expect(screen.getByRole("status", { name: "filtre actif" })).toBeTruthy();
  expect((screen.getByRole("searchbox", { name: "filtre" }) as HTMLInputElement).value).toBe("tarifs");
  expect((screen.getByRole("combobox", { name: "classe de statut" }) as HTMLSelectElement).value).toBe("2xx");
  expect(screen.getByRole("button", { name: "exporter les URL" })).toBeTruthy();
  // Le détail s'ouvre de nouveau, le suivi est arrêté.
  fireEvent.click(screen.getByText("https://exemple.fr/tarifs"));
  expect(screen.getByRole("region", { name: "détail de https://exemple.fr/tarifs" })).toBeTruthy();
  const calls = vi.mocked(api.peekCrawl).mock.calls.length;
  await advance(10000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(calls);
});

test("quitter l’écran arrête le suivi", async () => {
  await renderRunning();
  cleanup();
  const calls = vi.mocked(api.peekCrawl).mock.calls.length;
  await advance(10000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(calls);
});

test("un instantané vide, avant la première page, montre le panneau et le tableau vides", async () => {
  vi.mocked(api.peekCrawl).mockResolvedValue(liveView([]));
  await renderRunning();
  expect(screen.getByText("résultats provisoires")).toBeTruthy();
  expect(screen.getByRole("table")).toBeTruthy();
  expect(screen.getAllByRole("row")).toHaveLength(1);
  expect(screen.getByRole("button", { name: "arrêter" })).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("si le résultat final ne se charge pas, les instantanés s’arrêtent quand même", async () => {
  await renderRunning();
  vi.mocked(api.loadCrawl).mockRejectedValue("lecture impossible");
  await act(async () => finishedCb(meta({ kind: "completed" })));
  await advance(0);
  expect(screen.getByRole("alert").textContent).toContain("lecture impossible");
  const calls = vi.mocked(api.peekCrawl).mock.calls.length;
  await advance(10000);
  expect(api.peekCrawl).toHaveBeenCalledTimes(calls);
});
