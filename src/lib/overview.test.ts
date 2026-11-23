import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { loadGrouping, saveGrouping, crawlSummary } from "./overview";
import { filterRows, type StatusClass } from "./table";
import type { Indexability, UrlRow } from "./types";

const STORAGE_KEY = "arpenteur.grouping";

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

function row(i: number, status: number, indexable: Indexability): UrlRow {
  return {
    url: `https://exemple.fr/p${i}`,
    status,
    contentType: "text/html",
    title: null,
    titleLength: null,
    metaDescription: null,
    metaDescriptionLength: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable,
    depth: 0,
    inlinks: 0,
    offList: false,
    wordCount: null,
    size: null,
    elapsedMs: 0,
  };
}

function rows(spec: Array<[number, number, number]>): UrlRow[] {
  // [statut, nombre de lignes, dont indexables]
  const out: UrlRow[] = [];
  for (const [status, n, indexable] of spec) {
    for (let k = 0; k < n; k++) out.push(row(out.length, status, k < indexable ? "yes" : "no"));
  }
  return out;
}

test("le résumé compte les URL par réponse et les pages indexables", () => {
  const all = rows([
    [200, 88, 71],
    [301, 4, 0],
    [404, 3, 0],
  ]);
  expect(all).toHaveLength(95);
  expect(crawlSummary(all)).toEqual({
    urls: 95,
    byStatus: { "2xx": 88, "3xx": 4, "4xx": 3, "5xx": 0, error: 0 },
    indexable: 71,
  });
});

test("chaque nombre du résumé égale le nombre de lignes du tableau une fois la classe choisie", () => {
  const all = rows([
    [200, 2, 1],
    [204, 1, 0],
    [302, 1, 0],
    [410, 1, 0],
    [503, 2, 0],
    [-1, 2, 0],
    [-6, 3, 0],
  ]);
  const summary = crawlSummary(all);
  for (const c of ["2xx", "3xx", "4xx", "5xx", "error"] as const) {
    const cls: StatusClass = c;
    expect(summary.byStatus[c]).toBe(filterRows(all, { query: "", statusClass: cls }).length);
  }
  expect(summary.byStatus.error).toBe(2);
  expect(summary.urls).toBe(all.length);
});

test("un crawl vide donne un résumé à zéro", () => {
  expect(crawlSummary([])).toEqual({
    urls: 0,
    byStatus: { "2xx": 0, "3xx": 0, "4xx": 0, "5xx": 0, error: 0 },
    indexable: 0,
  });
});

test("sans choix enregistré, le classement est priorité", () => {
  expect(loadGrouping()).toBe("priorite");
});

test("le classement choisi est retenu", () => {
  saveGrouping("theme");
  expect(loadGrouping()).toBe("theme");
  saveGrouping("priorite");
  expect(loadGrouping()).toBe("priorite");
});

test("un classement enregistré invalide revient à priorité", () => {
  for (const bad of ["inconnu", "", "42", "null", "\"theme\"", "Theme"]) {
    localStorage.setItem(STORAGE_KEY, bad);
    expect(loadGrouping()).toBe("priorite");
  }
});

test("un stockage indisponible ne fait pas d’erreur", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  expect(loadGrouping()).toBe("priorite");
  expect(() => saveGrouping("theme")).not.toThrow();
});
