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

test("répartition : 8 haute, 16 moyenne, 14 basse, 4 info", () => {
  const count = (level: string) => allCodes.filter((c) => CATALOG[c].level === level).length;
  expect(count("haute")).toBe(8);
  expect(count("moyenne")).toBe(16);
  expect(count("basse")).toBe(14);
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

test("brokenExternalLink reprend les textes de la conception", () => {
  expect(CATALOG.brokenExternalLink.level).toBe("moyenne");
  expect(findingTitle("brokenExternalLink")).toBe("Lien externe cassé");
  expect(findingWhy("brokenExternalLink")).toBe(
    "Une page du site pointe vers une adresse d'un autre site qui n'existe plus. Le visiteur qui clique tombe sur une erreur.",
  );
  expect(findingFix("brokenExternalLink")).toBe("Corriger le lien ou le retirer.");
});

test("les quatre nouvelles entrées reprennent les textes de la conception", () => {
  expect(CATALOG.listedUrlError.level).toBe("haute");
  expect(findingTitle("listedUrlError")).toBe("Adresse de la liste en erreur");
  expect(findingWhy("listedUrlError")).toBe("Une adresse de la liste répond par une erreur, ou ne répond pas.");
  expect(findingFix("listedUrlError")).toBe("Corriger l'adresse, la rediriger ou la retirer.");
  expect(CATALOG.canonicalMultiple.level).toBe("moyenne");
  expect(findingTitle("canonicalMultiple")).toBe("Plusieurs balises canonical");
  expect(findingWhy("canonicalMultiple")).toBe(
    "La page désigne plusieurs adresses de référence. Google peut les ignorer toutes.",
  );
  expect(findingFix("canonicalMultiple")).toBe("Ne garder qu'une balise canonical par page.");
  expect(CATALOG.h1Duplicate.level).toBe("basse");
  expect(findingTitle("h1Duplicate")).toBe("H1 en double");
  expect(findingWhy("h1Duplicate")).toBe("Plusieurs pages ont le même titre principal. Elles se distinguent moins bien.");
  expect(findingFix("h1Duplicate")).toBe("Donner à chaque page un H1 qui lui est propre.");
  expect(CATALOG.h1TooLong.level).toBe("basse");
  expect(findingTitle("h1TooLong")).toBe("H1 trop long (plus de 70 caractères)");
  expect(findingWhy("h1TooLong")).toBe("Un titre principal long se lit mal et dilue le sujet de la page.");
  expect(findingFix("h1TooLong")).toBe("Raccourcir le H1 à l'essentiel.");
});
