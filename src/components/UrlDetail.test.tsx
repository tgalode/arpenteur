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
    metaDescription: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable: true,
    depth: 0,
    inlinks: 0,
    wordCount: 10,
    size: null,
  };
}

function page(url: string): PageRecord {
  return { url, status: 200, contentType: "html", size: 10, elapsedMs: 5, headers: {}, facts: null };
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
    sitemap: { found: false, sources: [], urls: [], truncated: false, errors: [], robotsTxt: null },
  };
}

function detail(url: string, findings: Finding[], urls: string[]) {
  const r = result(findings, urls);
  render(<UrlDetail row={row(url)} result={r} />);
  return within(screen.getByRole("region"));
}

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
