import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { ALL_COLUMN_KEYS, DEFAULT_COLUMN_KEYS, loadColumnKeys, saveColumnKeys } from "./columns";

const STORAGE_KEY = "arpenteur.columns";

beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());

test("les 16 colonnes sont dans l’ordre du CSV et les 9 par défaut en sont un sous-ensemble ordonné", () => {
  expect(ALL_COLUMN_KEYS).toHaveLength(16);
  expect(DEFAULT_COLUMN_KEYS).toEqual([
    "url", "status", "contentType", "title", "h1", "indexable", "depth", "inlinks", "wordCount",
  ]);
  expect(ALL_COLUMN_KEYS.filter((k) => DEFAULT_COLUMN_KEYS.includes(k))).toEqual(DEFAULT_COLUMN_KEYS);
});

test("sans choix enregistré, on a les colonnes par défaut", () => {
  expect(loadColumnKeys()).toEqual(DEFAULT_COLUMN_KEYS);
});

test("un choix enregistré est relu", () => {
  saveColumnKeys(["url", "size", "elapsedMs"]);
  expect(loadColumnKeys()).toEqual(["url", "size", "elapsedMs"]);
});

test("un choix relu est remis dans l’ordre des colonnes, sans doublon", () => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(["size", "url", "title", "size"]));
  expect(loadColumnKeys()).toEqual(["url", "title", "size"]);
});

test("la colonne url est toujours présente", () => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(["title", "size"]));
  expect(loadColumnKeys()).toEqual(["url", "title", "size"]);
  saveColumnKeys(["size"]);
  expect(loadColumnKeys()).toEqual(["url", "size"]);
});

test("les clés inconnues sont retirées", () => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(["url", "inconnue", "size", 3, null]));
  expect(loadColumnKeys()).toEqual(["url", "size"]);
});

test("un choix illisible revient aux colonnes par défaut", () => {
  for (const bad of ["{pas du json", "42", "null", "{\"a\":1}", "[]", "[\"inconnue\"]", "\"url\""]) {
    localStorage.setItem(STORAGE_KEY, bad);
    expect(loadColumnKeys()).toEqual(DEFAULT_COLUMN_KEYS);
  }
});

test("un stockage indisponible ne fait pas d’erreur", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  expect(loadColumnKeys()).toEqual(DEFAULT_COLUMN_KEYS);
  expect(() => saveColumnKeys(["url", "size"])).not.toThrow();
});
