import { expect, test } from "vitest";
import { findingTitle } from "./catalog";
import { csvCell, problemsCsv, urlsCsv } from "./csv";
import type { Finding, UrlRow } from "./types";

function row(over: Partial<UrlRow> = {}): UrlRow {
  return {
    url: "https://exemple.fr/",
    status: 200,
    contentType: "text/html",
    title: "Accueil",
    metaDescription: null,
    h1: "Bienvenue",
    h1Count: 1,
    canonical: null,
    indexable: true,
    depth: 0,
    inlinks: 3,
    wordCount: 120,
    size: null,
    ...over,
  };
}

function finding(over: Partial<Finding> = {}): Finding {
  return { code: "titleMissing", url: "https://exemple.fr/a", related: [], detail: null, ...over };
}

test("séparateur point-virgule, BOM et CRLF", () => {
  const csv = urlsCsv([row(), row({ url: "https://exemple.fr/b" })]);
  expect(csv.startsWith("﻿")).toBe(true);
  const lines = csv.slice(1).split("\r\n");
  expect(lines[0]).toBe("url;statut;type;title;h1;indexable;prof.;liens entr.;mots");
  expect(lines[1]).toBe("https://exemple.fr/;200;text/html;Accueil;Bienvenue;oui;0;3;120");
  expect(lines).toHaveLength(3);
  expect(csv.replace(/\r\n/g, "")).not.toMatch(/[\r\n]/);
});

test("les champs avec ; \" ou retour à la ligne sont échappés", () => {
  const csv = urlsCsv([row({ title: 'a;b "c"', h1: "ligne1\nligne2" })]);
  expect(csv).toContain('"a;b ""c"""');
  expect(csv).toContain('"ligne1\nligne2"');
});

test("les valeurs absentes donnent des cellules vides", () => {
  const csv = urlsCsv([row({ title: null, contentType: "", depth: null })]);
  expect(csv.slice(1).split("\r\n")[1]).toBe("https://exemple.fr/;200;;;Bienvenue;oui;;3;120");
});

test("problemsCsv donne le niveau et le titre du catalogue", () => {
  const csv = problemsCsv([
    finding({ detail: "manquant", related: ["https://exemple.fr/x", "https://exemple.fr/y"] }),
  ]);
  const lines = csv.slice(1).split("\r\n");
  expect(lines[0]).toBe("niveau;code;problème;URL;détail;URL liées");
  expect(lines[1]).toBe(
    `haute;titleMissing;${findingTitle("titleMissing")};https://exemple.fr/a;manquant;https://exemple.fr/x https://exemple.fr/y`,
  );
});

test("sans constat, problemsCsv ne contient que l’en-tête", () => {
  expect(problemsCsv([]).slice(1)).toBe("niveau;code;problème;URL;détail;URL liées");
});

test("les cellules sont protégées contre l’injection de formule", () => {
  expect(csvCell("=HYPERLINK(\"http://x\";\"a\")")).toBe("\"'=HYPERLINK(\"\"http://x\"\";\"\"a\"\")\"");
  expect(csvCell("+33 2 99 00 00 00")).toBe("'+33 2 99 00 00 00");
  expect(csvCell("@x")).toBe("'@x");
  expect(csvCell("-cmd")).toBe("'-cmd");
  expect(csvCell("\tx")).toBe("'\tx");
  expect(csvCell("\rx")).toBe("\"'\rx\"");
});

test("les nombres, URL et titres ordinaires ne sont pas modifiés", () => {
  expect(csvCell("-5")).toBe("-5");
  expect(csvCell("-5,2")).toBe("-5,2");
  expect(csvCell("https://exemple.fr/a")).toBe("https://exemple.fr/a");
  expect(csvCell("Accueil - Exemple")).toBe("Accueil - Exemple");
});

test("la protection s’applique aux deux exports", () => {
  expect(urlsCsv([row({ title: "=1+1" })])).toContain(";'=1+1;");
  expect(problemsCsv([finding({ detail: "@cmd" })])).toContain(";'@cmd;");
});
