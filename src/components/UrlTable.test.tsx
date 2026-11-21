import { cleanup, render, screen, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, expect, test } from "vitest";
import type { Indexability, UrlRow } from "../lib/types";
import { UrlTable } from "./UrlTable";

function row(url: string, indexable: Indexability): UrlRow {
  return {
    url,
    status: 200,
    contentType: "html",
    title: null,
    metaDescription: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable,
    depth: 0,
    inlinks: 0,
    wordCount: null,
    size: null,
  };
}

// jsdom ne mesure aucune mise en page : le virtualiseur lit clientHeight/clientWidth.
const originalHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");
const originalWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, value: 600 });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, value: 1000 });
});

afterAll(() => {
  for (const [key, d] of [
    ["clientHeight", originalHeight],
    ["clientWidth", originalWidth],
  ] as const) {
    if (d) Object.defineProperty(HTMLElement.prototype, key, d);
    else delete (HTMLElement.prototype as unknown as Record<string, unknown>)[key];
  }
});

afterEach(cleanup);

test("indexable affiche oui, non ou non analysé", () => {
  render(
    <UrlTable
      rows={[
        row("https://exemple.fr/a", "yes"),
        row("https://exemple.fr/b", "no"),
        row("https://exemple.fr/c", "notAnalyzed"),
      ]}
      selectedUrl={null}
      urlFilter={null}
      onSelect={() => {}}
    />,
  );
  const cells = screen.getAllByRole("row").slice(1).map((r) => within(r).getAllByRole("cell")[5]);
  expect(cells.map((c) => c.textContent)).toEqual(["oui", "non", "non analysé"]);
  expect(cells.map((c) => c.className)).toEqual(["tone-ok", "tone-err", "dim"]);
});
