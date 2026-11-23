import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import type { CrawlResult, Finding, PageRecord, UrlRow } from "../lib/types";
import { UrlDetail } from "./UrlDetail";

afterEach(cleanup);

const site = (path: string) => `https://exemple.fr${path}`;

function row(url: string): UrlRow {
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
  };
}

function page(url: string): PageRecord {
  return { url, status: 200, contentType: "html", size: 10, elapsedMs: 5, headers: {}, facts: null, probed: false };
}

function finding(over: Partial<Finding> & Pick<Finding, "code" | "url">): Finding {
  return { related: [], detail: null, ...over };
}

function result(findings: Finding[], urls: string[]): CrawlResult {
  return {
    meta: {} as CrawlResult["meta"],
    rows: urls.map(row),
    findings,
    pages: urls.map(page),
    links: { inlinks: {}, depth: {}, redirectChains: {} },
    sitemap: { found: false, sources: [], urls: [], truncated: false, errors: [], robotsTxt: null, robotsStatus: null },
    externalLinks: [],
    externalUnchecked: 0,
    listSkipped: [],
    robots: { content: null, status: null, sitemaps: [], blocked: [], blockedKnown: false, respected: true },
  };
}

function detail(url: string, findings: Finding[], urls: string[], extra: Partial<CrawlResult> = {}) {
  const r = { ...result(findings, urls), ...extra };
  render(<UrlDetail row={row(url)} result={r} />);
  return within(screen.getByRole("region"));
}

test("le détail d'une ligne hors liste le dit, celui d'une ligne de la liste non", () => {
  const off = { ...result([], [site("/a")]), rows: [{ ...row(site("/a")), offList: true }] };
  render(<UrlDetail row={off.rows[0]} result={off} />);
  expect(within(screen.getByRole("region")).getByText(/hors liste/i)).toBeTruthy();
  cleanup();
  const listed = detail(site("/a"), [], [site("/a")]);
  expect(listed.queryByText(/hors liste/i)).toBeNull();
});

test("un title partagé par 3 pages apparaît une fois par page", () => {
  const pages = [site("/a"), site("/b"), site("/c")];
  const findings = pages.map((url) =>
    finding({ code: "titleDuplicate", url, related: pages.filter((u) => u !== url) }),
  );
  const view = detail(site("/a"), findings, pages);
  expect(view.getAllByText("Title en double")).toHaveLength(1);
  expect(view.getByRole("heading", { name: "problèmes (1)" })).toBeTruthy();
});

test("la cible d'un canonical ne reçoit pas le constat de la page source", () => {
  const findings = [
    finding({ code: "canonicalToOtherUrl", url: site("/a"), related: [site("/b")] }),
  ];
  const target = detail(site("/b"), findings, [site("/a"), site("/b")]);
  expect(target.queryByText("Canonical vers une autre page")).toBeNull();
  expect(target.getByText("aucun problème sur cette URL.")).toBeTruthy();
  cleanup();
  const source = detail(site("/a"), findings, [site("/a"), site("/b")]);
  expect(source.getAllByText("Canonical vers une autre page")).toHaveLength(1);
});

test("une page avec deux liens cassés les montre tous deux, avec leur adresse", () => {
  const findings = [
    finding({ code: "brokenInternalLink", url: site("/m1"), related: [site("/a"), site("/b")] }),
    finding({ code: "brokenInternalLink", url: site("/m2"), related: [site("/a")] }),
  ];
  const urls = [site("/a"), site("/b"), site("/m1"), site("/m2")];
  const source = detail(site("/a"), findings, urls);
  expect(source.getAllByText("Lien interne cassé")).toHaveLength(2);
  expect(source.getByText(`vers ${site("/m1")}`)).toBeTruthy();
  expect(source.getByText(`vers ${site("/m2")}`)).toBeTruthy();
  cleanup();
  const other = detail(site("/b"), findings, urls);
  expect(other.getAllByText("Lien interne cassé")).toHaveLength(1);
  expect(other.getByText(`vers ${site("/m1")}`)).toBeTruthy();
  expect(other.queryByText(`vers ${site("/m2")}`)).toBeNull();
  cleanup();
  // La page en erreur elle-même porte le constat, sans « vers ».
  const broken = detail(site("/m1"), findings, urls);
  expect(broken.getAllByText("Lien interne cassé")).toHaveLength(1);
  expect(broken.queryByText(/^vers /)).toBeNull();
});

test("une page qui utilise une image lourde la voit, avec l'adresse de l'image", () => {
  const findings = [finding({ code: "imageTooHeavy", url: site("/big.jpg"), related: [site("/a")] })];
  const urls = [site("/a"), site("/b"), site("/big.jpg")];
  const user = detail(site("/a"), findings, urls);
  expect(user.getAllByText("Image lourde (plus de 200 Ko)")).toHaveLength(1);
  expect(user.getByText(`image ${site("/big.jpg")}`)).toBeTruthy();
  cleanup();
  const unrelated = detail(site("/b"), findings, urls);
  expect(unrelated.queryByText("Image lourde (plus de 200 Ko)")).toBeNull();
});

test("les autres codes ne passent pas par les URL liées (redirections)", () => {
  const findings = [
    finding({ code: "temporaryRedirect", url: site("/old"), related: [site("/new")] }),
    finding({ code: "redirectChain", url: site("/s1"), related: [site("/s2"), site("/s3")] }),
  ];
  const urls = [site("/old"), site("/new"), site("/s1"), site("/s2"), site("/s3")];
  expect(detail(site("/new"), findings, urls).queryByText("Redirection temporaire (302, 303, 307)")).toBeNull();
  cleanup();
  expect(detail(site("/s2"), findings, urls).queryByText("Chaîne de redirections")).toBeNull();
  cleanup();
  expect(detail(site("/old"), findings, urls).getAllByText("Redirection temporaire (302, 303, 307)")).toHaveLength(1);
});

test("un même constat reçu deux fois n'est montré qu'une fois", () => {
  const dup = finding({ code: "brokenInternalLink", url: site("/m1"), related: [site("/a"), site("/a")] });
  const view = detail(site("/a"), [dup, { ...dup }], [site("/a"), site("/m1")]);
  expect(view.getAllByText("Lien interne cassé")).toHaveLength(1);
});

test("le détail d'une page liste ses liens externes cassés avec leur adresse", () => {
  const dead = "https://autre.exemple/mort";
  const findings = [finding({ code: "brokenExternalLink", url: dead, related: [site("/a")], detail: "404" })];
  const urls = [site("/a"), site("/b")];
  const source = detail(site("/a"), findings, urls);
  expect(source.getAllByText("Lien externe cassé")).toHaveLength(1);
  expect(source.getByText(`vers ${dead}`)).toBeTruthy();
  expect(source.getByText("404")).toBeTruthy();
  cleanup();
  const other = detail(site("/b"), findings, urls);
  expect(other.queryByText("Lien externe cassé")).toBeNull();
});

function withLinks(hrefs: string[]): PageRecord {
  const base = page(site("/a"));
  return {
    ...base,
    facts: {
      title: null,
      metaDescription: null,
      h1: [],
      canonical: null,
      canonicals: [],
      metaRobots: null,
      links: hrefs.map((href) => ({ href, anchor: "", nofollow: false })),
      images: [],
      jsonLd: [],
      wordCount: 10,
      textHash: "x",
      lang: null,
      headings: [],
      unlinkedPhones: [],
    },
  };
}

test("un lien sortant externe vérifié affiche son statut", () => {
  const checked = "https://autre.exemple/ok";
  const gone = "https://disparu.exemple/";
  const unchecked = "https://jamais.exemple/";
  const view = detail(site("/a"), [], [site("/a")], {
    pages: [withLinks([checked, gone, unchecked, site("/b")])],
    externalLinks: [
      { url: checked, outcome: "ok", status: 200, finalUrl: null },
      { url: gone, outcome: "broken", status: null, finalUrl: null },
    ],
  });
  const li = (href: string) => view.getByText(href).closest("li")!;
  expect(within(li(checked)).getByText("200")).toBeTruthy();
  expect(within(li(gone)).getByText("domaine introuvable")).toBeTruthy();
  expect(li(unchecked).textContent).toBe(`${unchecked} –`);
  expect(li(site("/b")).textContent).toBe(`${site("/b")} –`);
});

test("un lien externe cassé en 404 affiche 404 dans les liens sortants", () => {
  const dead = "https://autre.exemple/mort";
  const view = detail(site("/a"), [], [site("/a")], {
    pages: [withLinks([dead])],
    externalLinks: [{ url: dead, outcome: "broken", status: 404, finalUrl: null }],
  });
  expect(within(view.getByText(dead).closest("li")!).getByText("404")).toBeTruthy();
});

test("le détail d'une URL affiche le type à côté du niveau", () => {
  const view = detail(site("/a"), [finding({ code: "titleMissing", url: site("/a") })], [site("/a")]);
  const item = view.getByText("Title absent").closest("li")!;
  expect(item.textContent).toContain("haute");
  expect(within(item).getByText("problème")).toBeTruthy();
});
