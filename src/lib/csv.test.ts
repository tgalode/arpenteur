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
    indexable: "yes",
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

test("le CSV écrit oui, non ou non analysé", () => {
  const csv = urlsCsv([
    row({ indexable: "yes" }),
    row({ indexable: "no" }),
    row({ indexable: "notAnalyzed" }),
  ]);
  const lines = csv.slice(1).split("\r\n");
  expect(lines.slice(1, 4).map((l) => l.split(";")[5])).toEqual(["oui", "non", "non analysé"]);
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

test("problemsCsv trie par niveau, puis par code du catalogue, puis par URL", () => {
  const csv = problemsCsv([
    finding({ code: "canonicalToOtherUrl", url: "https://exemple.fr/z" }),
    finding({ code: "titleTooLong", url: "https://exemple.fr/b" }),
    finding({ code: "canonicalMissing", url: "https://exemple.fr/b" }),
    finding({ code: "titleMissing", url: "https://exemple.fr/b" }),
    finding({ code: "canonicalMissing", url: "https://exemple.fr/a" }),
    finding({ code: "brokenInternalLink", url: "https://exemple.fr/m" }),
    finding({ code: "titleMissing", url: "https://exemple.fr/a" }),
  ]);
  const lines = csv.slice(1).split("\r\n").slice(1);
  expect(lines.map((l) => l.split(";").slice(0, 2).concat(l.split(";")[3]).join(" "))).toEqual([
    "haute brokenInternalLink https://exemple.fr/m",
    "haute titleMissing https://exemple.fr/a",
    "haute titleMissing https://exemple.fr/b",
    "moyenne canonicalMissing https://exemple.fr/a",
    "moyenne canonicalMissing https://exemple.fr/b",
    "basse titleTooLong https://exemple.fr/b",
    "information canonicalToOtherUrl https://exemple.fr/z",
  ]);
});

test("problemsCsv ne modifie pas la liste reçue", () => {
  const findings = [finding({ code: "canonicalMissing" }), finding({ code: "titleMissing" })];
  problemsCsv(findings);
  expect(findings.map((f) => f.code)).toEqual(["canonicalMissing", "titleMissing"]);
});
