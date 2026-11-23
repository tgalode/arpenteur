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

test("répartition validée : 10 haute, 16 moyenne, 15 basse, 4 info", () => {
  const count = (level: string) => allCodes.filter((c) => CATALOG[c].level === level).length;
  expect(count("haute")).toBe(10);
  expect(count("moyenne")).toBe(16);
  expect(count("basse")).toBe(15);
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

test("les trois nouvelles entrées reprennent les textes de la conception", () => {
  expect(CATALOG.soft404.level).toBe("haute");
  expect(findingTitle("soft404")).toBe("Fausse page d'erreur (soft 404)");
  expect(findingWhy("soft404")).toBe(
    "La page affiche le message d'erreur du site, mais répond 200. Google peut l'indexer ou perdre du temps dessus.",
  );
  expect(findingFix("soft404")).toBe("Faire répondre 404 ou 410 à cette adresse, ou y remettre un vrai contenu.");
  expect(CATALOG.soft404Site.level).toBe("haute");
  expect(findingTitle("soft404Site")).toBe("Le site répond 200 aux adresses inexistantes");
  expect(findingWhy("soft404Site")).toBe(
    "Une adresse qui n'existe pas renvoie une page normale au lieu d'une erreur 404. Google ne peut pas distinguer les vraies pages des fausses.",
  );
  expect(findingFix("soft404Site")).toBe("Configurer le serveur pour répondre 404 aux adresses inexistantes.");
  expect(CATALOG.robotsTxtMissing.level).toBe("basse");
  expect(findingTitle("robotsTxtMissing")).toBe("Fichier robots.txt introuvable");
  expect(findingWhy("robotsTxtMissing")).toBe("Sans robots.txt, le sitemap n'est pas déclaré aux moteurs.");
  expect(findingFix("robotsTxtMissing")).toBe(
    "Ajouter un fichier robots.txt à la racine, avec l'adresse du sitemap.",
  );
});
