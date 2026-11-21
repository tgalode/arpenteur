import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, beforeEach, expect, test } from "vitest";
import type { Indexability, UrlRow } from "../lib/types";
import { UrlTable } from "./UrlTable";

function row(url: string, indexable: Indexability, over: Partial<UrlRow> = {}): UrlRow {
  return {
    url,
    status: 200,
    contentType: "html",
    title: null,
    titleLength: null,
    metaDescription: null,
    metaDescriptionLength: null,
    h1: null,
    h1Count: 0,
    canonical: null,
    indexable,
    depth: 0,
    inlinks: 0,
    wordCount: null,
    size: null,
    elapsedMs: 0,
    ...over,
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

beforeEach(() => localStorage.clear());
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

function renderTable(rows: UrlRow[]) {
  return render(<UrlTable rows={rows} selectedUrl={null} urlFilter={null} onSelect={() => {}} />);
}

const headers = () => screen.getAllByRole("columnheader").map((h) => h.textContent);

test("le tableau montre les 9 colonnes par défaut", () => {
  renderTable([row("https://exemple.fr/a", "yes")]);
  expect(headers()).toEqual(["url", "statut", "type", "title", "h1", "indexable", "prof.", "liens entr.", "mots"]);
});

test("cocher une colonne l’affiche et le choix est retenu", async () => {
  const user = userEvent.setup();
  const first = renderTable([row("https://exemple.fr/a", "yes", { size: 86016, elapsedMs: 212 })]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "poids" }));
  expect(headers()).toContain("poids");
  expect(headers()[headers().length - 1]).toBe("poids");
  first.unmount();
  renderTable([row("https://exemple.fr/a", "yes")]);
  expect(headers()).toContain("poids");
});

test("décocher une colonne la masque", async () => {
  const user = userEvent.setup();
  renderTable([row("https://exemple.fr/a", "yes")]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "h1" }));
  expect(headers()).not.toContain("h1");
  expect(headers()).toHaveLength(8);
});

test("la colonne url ne peut pas être masquée", async () => {
  const user = userEvent.setup();
  renderTable([row("https://exemple.fr/a", "yes")]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "url" }));
  expect(headers()[0]).toBe("url");
});

test("un choix enregistré invalide revient aux colonnes par défaut", () => {
  localStorage.setItem("arpenteur.columns", "{pas du json");
  renderTable([row("https://exemple.fr/a", "yes")]);
  expect(headers()).toHaveLength(9);
  expect(headers()[0]).toBe("url");
});

test("poids et temps sont lisibles dans le tableau", async () => {
  const user = userEvent.setup();
  renderTable([
    row("https://exemple.fr/a", "yes", { size: 86016, elapsedMs: 212 }),
    row("https://exemple.fr/b", "yes", { size: null, elapsedMs: 5 }),
  ]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "poids" }));
  await user.click(screen.getByRole("checkbox", { name: "temps" }));
  const cells = screen.getAllByRole("row").slice(1).map((r) => within(r).getAllByRole("cell"));
  const last = (i: number) => cells[i].slice(-2).map((c) => c.textContent);
  expect(last(0)).toEqual(["84 Ko", "212 ms"]);
  expect(last(1)).toEqual(["–", "5 ms"]);
});

test("un clic sur l’en-tête poids trie le tableau", async () => {
  const user = userEvent.setup();
  localStorage.setItem("arpenteur.columns", JSON.stringify(["url", "size"]));
  renderTable([
    row("https://exemple.fr/a", "yes", { size: 90000 }),
    row("https://exemple.fr/b", "yes", { size: 500 }),
  ]);
  await user.click(screen.getByRole("columnheader", { name: "poids" }));
  const urls = screen.getAllByRole("row").slice(1).map((r) => within(r).getAllByRole("cell")[0].textContent);
  expect(urls).toEqual(["https://exemple.fr/b", "https://exemple.fr/a"]);
});

test("masquer la colonne triée retire le tri", async () => {
  const user = userEvent.setup();
  localStorage.setItem("arpenteur.columns", JSON.stringify(["url", "size"]));
  renderTable([
    row("https://exemple.fr/a", "yes", { size: 90000 }),
    row("https://exemple.fr/b", "yes", { size: 500 }),
  ]);
  const urls = () => screen.getAllByRole("row").slice(1).map((r) => within(r).getAllByRole("cell")[0].textContent);
  await user.click(screen.getByRole("columnheader", { name: "poids" }));
  expect(urls()).toEqual(["https://exemple.fr/b", "https://exemple.fr/a"]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "poids" }));
  expect(urls()).toEqual(["https://exemple.fr/a", "https://exemple.fr/b"]);
  const sorts = screen.getAllByRole("columnheader").map((h) => h.getAttribute("aria-sort"));
  expect(sorts).toEqual(["none"]);
});

test("la barre d’outils garde la recherche en enfant direct et les cases hors de ce sélecteur", async () => {
  const user = userEvent.setup();
  const { container } = renderTable([row("https://exemple.fr/a", "yes")]);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  const tools = container.querySelector(".urltable-tools") as HTMLElement;
  expect(tools.querySelector(":scope > input[type=search]")).not.toBeNull();
  const boxes = tools.querySelectorAll("input[type=checkbox]");
  expect(boxes.length).toBeGreaterThan(0);
  expect(tools.querySelectorAll(":scope > input[type=checkbox]")).toHaveLength(0);
});

test("sans réponse HTTP, le temps est absent", async () => {
  const user = userEvent.setup();
  localStorage.setItem("arpenteur.columns", JSON.stringify(["url", "elapsedMs"]));
  renderTable([
    row("https://exemple.fr/a", "yes", { status: -1, elapsedMs: 0 }),
    row("https://exemple.fr/b", "yes", { status: 200, elapsedMs: 212 }),
  ]);
  const times = () => screen.getAllByRole("row").slice(1).map((r) => within(r).getAllByRole("cell")[1].textContent);
  expect(times()).toEqual(["–", "212 ms"]);
  await user.click(screen.getByRole("columnheader", { name: "temps" }));
  expect(times()).toEqual(["212 ms", "–"]);
  await user.click(screen.getByRole("columnheader", { name: "temps" }));
  expect(times()).toEqual(["212 ms", "–"]);
});
