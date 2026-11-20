import { describe, expect, test } from "vitest";
import { filterRows, sortRows, statusText, statusTone } from "./table";
import type { UrlRow } from "./types";

function row(p: Partial<UrlRow>): UrlRow {
  return {
    url: "https://x.fr/",
    status: 200,
    contentType: "html",
    title: null,
    metaDescription: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable: true,
    depth: 0,
    inlinks: 0,
    wordCount: null,
    size: null,
    ...p,
  };
}

describe("sortRows", () => {
  test("sortRows met les valeurs nulles en dernier", () => {
    const rows = [row({ depth: 2 }), row({ depth: null }), row({ depth: 0 })];
    expect(sortRows(rows, "depth", "asc").map((r) => r.depth)).toEqual([0, 2, null]);
    expect(sortRows(rows, "depth", "desc").map((r) => r.depth)).toEqual([2, 0, null]);
    expect(rows.map((r) => r.depth)).toEqual([2, null, 0]);
  });

  test("sortRows trie le texte à la française", () => {
    const rows = [row({ title: "fable" }), row({ title: "école" }), row({ title: "eau" })];
    expect(sortRows(rows, "title", "asc").map((r) => r.title)).toEqual(["eau", "école", "fable"]);
  });

  test("sortRows trie les booléens false avant true", () => {
    const rows = [row({ indexable: true }), row({ indexable: false })];
    expect(sortRows(rows, "indexable", "asc").map((r) => r.indexable)).toEqual([false, true]);
  });
});

describe("filterRows", () => {
  test("filterRows cherche dans url, title et h1", () => {
    const rows = [
      row({ url: "https://x.fr/chat" }),
      row({ url: "https://x.fr/a", title: "Le CHIEN" }),
      row({ url: "https://x.fr/b", h1: "Oiseau" }),
    ];
    const f = (query: string) => filterRows(rows, { query, statusClass: "all" }).length;
    expect(f("chat")).toBe(1);
    expect(f("chien")).toBe(1);
    expect(f("OISEAU")).toBe(1);
    expect(f("")).toBe(3);
    expect(f("zzz")).toBe(0);
  });

  test("filterRows par classe de statut", () => {
    const rows = [
      row({ status: 200 }),
      row({ status: 301 }),
      row({ status: 404 }),
      row({ status: 503 }),
      row({ status: -1 }),
      row({ status: -6 }),
      row({ status: -2 }),
    ];
    const f = (statusClass: "all" | "2xx" | "3xx" | "4xx" | "5xx" | "error") =>
      filterRows(rows, { query: "", statusClass }).map((r) => r.status);
    expect(f("2xx")).toEqual([200]);
    expect(f("3xx")).toEqual([301]);
    expect(f("4xx")).toEqual([404]);
    expect(f("5xx")).toEqual([503]);
    expect(f("error")).toEqual([-1, -2]);
    expect(f("all")).toHaveLength(7);
  });
});

describe("statusText", () => {
  test("statusText nomme les statuts négatifs de SiteOne", () => {
    expect(statusText(200)).toBe("200");
    expect(statusText(404)).toBe("404");
    expect(statusText(-1)).toBe("connexion impossible");
    expect(statusText(-2)).toBe("délai dépassé");
    expect(statusText(-6)).toBe("ignorée");
    expect(statusText(-3)).toBe("erreur réseau");
  });
});

test("statusTone associe une teinte à chaque statut de crawl", () => {
  expect(statusTone("completed")).toBe("ok");
  expect(statusTone("running")).toBe("redir");
  expect(statusTone("stopped")).toBe("warn");
  expect(statusTone("throttled")).toBe("warn");
  expect(statusTone("failed")).toBe("err");
});
