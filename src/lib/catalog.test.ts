import { expect, test } from "vitest";
import codes from "./finding-codes.json";
import { CATALOG, findingFix, findingTitle, findingWhy } from "./catalog";
import type { FindingCode } from "./types";

const allCodes = codes as FindingCode[];

test("chaque code de finding-codes.json a une entrée et ses textes", () => {
  expect(Object.keys(CATALOG).sort()).toEqual([...allCodes].sort());
  for (const code of allCodes) {
    expect(CATALOG[code], code).toBeDefined();
    expect(findingTitle(code), code).not.toBe("");
    expect(findingWhy(code), code).not.toBe("");
    if (CATALOG[code].level === "info") {
      expect(findingFix(code), code).toBeNull();
    } else {
      expect(findingFix(code), code).toBeTruthy();
    }
  }
});

test("répartition validée : 7 haute, 14 moyenne, 12 basse, 4 info", () => {
  const count = (level: string) => allCodes.filter((c) => CATALOG[c].level === level).length;
  expect(count("haute")).toBe(7);
  expect(count("moyenne")).toBe(14);
  expect(count("basse")).toBe(12);
  expect(count("info")).toBe(4);
});

test("textes repris du catalogue", () => {
  expect(findingTitle("canonicalToRedirect")).toBe("Canonical vers une redirection");
  expect(findingWhy("canonicalToRedirect")).toBe(
    "La page désigne comme référence une adresse qui redirige. Les signaux envoyés à Google se contredisent.",
  );
  expect(findingFix("canonicalToRedirect")).toBe("Pointer le canonical vers l'adresse finale.");
  expect(findingTitle("httpsInvalid")).toBe("Certificat HTTPS invalide");
});
