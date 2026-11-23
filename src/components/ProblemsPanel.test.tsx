import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { StatusClass } from "../lib/table";
import type { Finding, FindingCode, Indexability, UrlRow } from "../lib/types";
import { ProblemsPanel } from "./ProblemsPanel";

beforeEach(() => localStorage.clear());
afterEach(cleanup);

function finding(code: FindingCode, n: number): Finding[] {
  return Array.from({ length: n }, (_, i) => ({ code, url: `https://exemple.fr/p${i}`, related: [], detail: null }));
}

function row(i: number, status: number, indexable: Indexability = "yes"): UrlRow {
  return {
    url: `https://exemple.fr/r${i}`,
    status,
    contentType: "text/html",
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
    offList: false,
    wordCount: null,
    size: null,
    elapsedMs: 0,
  };
}

// 4 URL en 2xx dont 3 indexables, 1 en 3xx, 2 en 4xx.
const ROWS: UrlRow[] = [
  row(0, 200),
  row(1, 200),
  row(2, 200),
  row(3, 200, "no"),
  row(4, 301, "no"),
  row(5, 404, "no"),
  row(6, 404, "no"),
];

// 3 haute, 5 moyenne, 8 basse, 4 information : 16 problèmes hors information.
const FINDINGS: Finding[] = [
  ...finding("brokenInternalLink", 2),
  ...finding("titleMissing", 1),
  ...finding("h1Missing", 3),
  ...finding("orphanPage", 2),
  ...finding("titleTooLong", 5),
  ...finding("phoneNotClickable", 3),
  ...finding("noindex", 4),
];

test("le panneau groupe par niveau avec les compteurs", () => {
  render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  expect(within(panel).getByRole("heading", { name: "# problèmes 16" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "haute 3" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "moyenne 5" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "basse 8" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "information 4" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "Lien interne cassé, problème, 2" })).toBeTruthy();
});

test("un code sans constat n’apparaît pas", () => {
  render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  expect(screen.queryByRole("button", { name: /Erreur serveur/ })).toBeNull();
  expect(screen.queryByRole("heading", { name: /^information/ })).not.toBeNull();
  cleanup();
  render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={finding("titleTooLong", 1)} selectedCode={null} onSelect={() => {}} />);
  expect(screen.queryByRole("heading", { name: /^haute/ })).toBeNull();
  expect(screen.queryByRole("heading", { name: /^information/ })).toBeNull();
});

test("sans aucun constat, le panneau le dit", () => {
  render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={[]} selectedCode={null} onSelect={() => {}} />);
  expect(screen.getByText("aucun problème constaté.")).toBeTruthy();
});

test("un clic sur un code le sélectionne, un second clic le retire", async () => {
  const onSelect = vi.fn();
  const { rerender } = render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={FINDINGS} selectedCode={null} onSelect={onSelect} />);
  const btn = screen.getByRole("button", { name: "Lien interne cassé, problème, 2" });
  expect(btn.getAttribute("aria-pressed")).toBe("false");
  await userEvent.click(btn);
  expect(onSelect).toHaveBeenLastCalledWith("brokenInternalLink");
  rerender(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={FINDINGS} selectedCode="brokenInternalLink" onSelect={onSelect} />);
  expect(screen.getByRole("button", { name: "Lien interne cassé, problème, 2" }).getAttribute("aria-pressed")).toBe("true");
  await userEvent.click(screen.getByRole("button", { name: "Lien interne cassé, problème, 2" }));
  expect(onSelect).toHaveBeenLastCalledWith(null);
});

test("le panneau affiche le type sur chaque ligne de problème", () => {
  render(<ProblemsPanel rows={ROWS} statusClass="all" onStatusClass={() => {}} findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  const line = within(panel).getByRole("button", { name: "Lien interne cassé, problème, 2" });
  expect(within(line).getByText("problème").className).toContain("dim");
  expect(within(panel).getByRole("button", { name: "Page orpheline, avertissement, 2" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "Title trop long (plus de 60 caractères), opportunité, 5" })).toBeTruthy();
});

function Harness({ findings = FINDINGS }: { findings?: Finding[] }) {
  const [statusClass, setStatusClass] = useState<StatusClass>("all");
  return (
    <>
      <ProblemsPanel
        findings={findings}
        rows={ROWS}
        selectedCode={null}
        onSelect={() => {}}
        statusClass={statusClass}
        onStatusClass={setStatusClass}
      />
      <output data-testid="status-class">{statusClass}</output>
    </>
  );
}

test("le résumé donne le nombre d’URL, leur répartition et les pages indexables", () => {
  render(<Harness />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  expect(within(panel).getByRole("heading", { name: "# vue d'ensemble" })).toBeTruthy();
  expect(within(panel).getByText("URL").parentElement?.textContent).toContain("7");
  expect(within(panel).getByRole("button", { name: "2xx, 4" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "3xx, 1" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "4xx, 2" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "5xx, 0" })).toBeTruthy();
  expect(within(panel).getByText("indexables").parentElement?.textContent).toContain("3");
});

test("les lignes ignorées sont comptées à part, en texte simple, seulement s'il y en a", () => {
  render(<ProblemsPanel rows={[row(0, 200), row(1, 404), row(2, -6), row(3, -6)]} statusClass="all" onStatusClass={() => {}} findings={[]} selectedCode={null} onSelect={() => {}} />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  const line = within(panel).getByText("ignorées");
  expect(line.parentElement?.textContent).toBe("ignorées2");
  expect(line.closest("button")).toBeNull();
  expect(within(panel).getByText("URL").parentElement?.textContent).toContain("4");
  cleanup();
  render(<Harness />);
  expect(within(screen.getByRole("complementary", { name: "problèmes" })).queryByText("ignorées")).toBeNull();
});

test("un clic sur « 4xx » règle le filtre de statut, un second clic le retire", async () => {
  render(<Harness />);
  const btn = () => screen.getByRole("button", { name: "4xx, 2" });
  expect(btn().getAttribute("aria-pressed")).toBe("false");
  await userEvent.click(btn());
  expect(screen.getByTestId("status-class").textContent).toBe("4xx");
  expect(btn().getAttribute("aria-pressed")).toBe("true");
  await userEvent.click(btn());
  expect(screen.getByTestId("status-class").textContent).toBe("all");
  expect(btn().getAttribute("aria-pressed")).toBe("false");
});

test("le classement par priorité est celui de départ", () => {
  render(<Harness />);
  const group = screen.getByRole("group", { name: "classement des problèmes" });
  expect(within(group).getByRole("button", { name: "priorité" }).getAttribute("aria-pressed")).toBe("true");
  expect(within(group).getByRole("button", { name: "thème" }).getAttribute("aria-pressed")).toBe("false");
});

test("le classement par thème regroupe les problèmes sous leur thème, avec le compteur du thème", async () => {
  render(<Harness />);
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  expect(within(panel).getByRole("heading", { name: "# problèmes 16" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "liens 4" })).toBeTruthy(); // 2 cassés + 2 orphelines
  expect(within(panel).getByRole("heading", { name: "titles 6" })).toBeTruthy(); // 1 manquant + 5 trop longs
  expect(within(panel).getByRole("heading", { name: "titres H1 à H6 3" })).toBeTruthy();
  expect(within(panel).queryByRole("heading", { name: /^haute/ })).toBeNull();
});

test("dans un thème, les problèmes sont rangés par niveau puis par nombre", async () => {
  render(<Harness />);
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  const titles = within(panel).getByRole("heading", { name: "titles 6" });
  const list = titles.nextElementSibling as HTMLElement;
  const names = within(list).getAllByRole("button").map((b) => b.getAttribute("aria-label"));
  // titleMissing (haute, 1) avant titleTooLong (basse, 5)
  expect(names).toEqual(["Title absent, problème, 1", "Title trop long (plus de 60 caractères), opportunité, 5"]);
});

test("l’ordre des thèmes est celui de THEMES", async () => {
  render(<Harness />);
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  const heads = within(panel).getAllByRole("heading", { level: 3 }).map((h) => h.textContent ?? "");
  const order = ["liens", "titles", "titres H1 à H6"].map((l) => heads.findIndex((h) => h.startsWith(l)));
  expect(order.every((i) => i >= 0)).toBe(true);
  expect([...order].sort((a, b) => a - b)).toEqual(order);
});

test("un thème sans problème n’apparaît pas", async () => {
  render(<Harness findings={finding("titleTooLong", 2)} />);
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  expect(within(panel).getByRole("heading", { name: "titles 2" })).toBeTruthy();
  expect(within(panel).queryByRole("heading", { name: /^liens/ })).toBeNull();
  expect(within(panel).queryByRole("heading", { name: /^images/ })).toBeNull();
});

test("les codes de niveau information restent à part, hors du total, dans les deux classements", async () => {
  render(<Harness />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  for (const mode of ["priorité", "thème"]) {
    await userEvent.click(screen.getByRole("button", { name: mode }));
    expect(within(panel).getByRole("heading", { name: "# problèmes 16" })).toBeTruthy();
    expect(within(panel).getByRole("heading", { name: "information 4" })).toBeTruthy();
    expect(within(panel).getAllByRole("button", { name: /Noindex/i })).toHaveLength(1);
  }
  // en thème, le thème de noindex ne compte pas ces 4 constats
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  expect(within(panel).queryByRole("heading", { name: /^indexation et sitemap/ })).toBeNull();
});

test("le classement choisi est retenu", async () => {
  const { unmount } = render(<Harness />);
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  unmount();
  render(<Harness />);
  expect(screen.getByRole("button", { name: "thème" }).getAttribute("aria-pressed")).toBe("true");
});

test("un classement enregistré invalide revient à priorité", () => {
  localStorage.setItem("arpenteur.grouping", "n’importe quoi");
  render(<Harness />);
  expect(screen.getByRole("button", { name: "priorité" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("heading", { name: "haute 3" })).toBeTruthy();
});

test("un stockage qui lève ne casse pas le panneau", async () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("stockage bloqué");
  });
  render(<Harness />);
  expect(screen.getByRole("button", { name: "priorité" }).getAttribute("aria-pressed")).toBe("true");
  await userEvent.click(screen.getByRole("button", { name: "thème" }));
  expect(screen.getByRole("heading", { name: "liens 4" })).toBeTruthy();
});
