import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import type { Finding, FindingCode } from "../lib/types";
import { ProblemsPanel } from "./ProblemsPanel";

afterEach(cleanup);

function finding(code: FindingCode, n: number): Finding[] {
  return Array.from({ length: n }, (_, i) => ({ code, url: `https://exemple.fr/p${i}`, related: [], detail: null }));
}

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
  render(<ProblemsPanel findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  expect(within(panel).getByRole("heading", { name: "# problèmes 16" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "haute 3" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "moyenne 5" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "basse 8" })).toBeTruthy();
  expect(within(panel).getByRole("heading", { name: "information 4" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "Lien interne cassé, problème, 2" })).toBeTruthy();
});

test("un code sans constat n’apparaît pas", () => {
  render(<ProblemsPanel findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  expect(screen.queryByRole("button", { name: /Erreur serveur/ })).toBeNull();
  expect(screen.queryByRole("heading", { name: /^information/ })).not.toBeNull();
  cleanup();
  render(<ProblemsPanel findings={finding("titleTooLong", 1)} selectedCode={null} onSelect={() => {}} />);
  expect(screen.queryByRole("heading", { name: /^haute/ })).toBeNull();
  expect(screen.queryByRole("heading", { name: /^information/ })).toBeNull();
});

test("sans aucun constat, le panneau le dit", () => {
  render(<ProblemsPanel findings={[]} selectedCode={null} onSelect={() => {}} />);
  expect(screen.getByText("aucun problème constaté.")).toBeTruthy();
});

test("un clic sur un code le sélectionne, un second clic le retire", async () => {
  const onSelect = vi.fn();
  const { rerender } = render(<ProblemsPanel findings={FINDINGS} selectedCode={null} onSelect={onSelect} />);
  const btn = screen.getByRole("button", { name: "Lien interne cassé, problème, 2" });
  expect(btn.getAttribute("aria-pressed")).toBe("false");
  await userEvent.click(btn);
  expect(onSelect).toHaveBeenLastCalledWith("brokenInternalLink");
  rerender(<ProblemsPanel findings={FINDINGS} selectedCode="brokenInternalLink" onSelect={onSelect} />);
  expect(screen.getByRole("button", { name: "Lien interne cassé, problème, 2" }).getAttribute("aria-pressed")).toBe("true");
  await userEvent.click(screen.getByRole("button", { name: "Lien interne cassé, problème, 2" }));
  expect(onSelect).toHaveBeenLastCalledWith(null);
});

test("le panneau affiche le type sur chaque ligne de problème", () => {
  render(<ProblemsPanel findings={FINDINGS} selectedCode={null} onSelect={() => {}} />);
  const panel = screen.getByRole("complementary", { name: "problèmes" });
  const line = within(panel).getByRole("button", { name: "Lien interne cassé, problème, 2" });
  expect(within(line).getByText("problème").className).toContain("dim");
  expect(within(panel).getByRole("button", { name: "Page orpheline, avertissement, 2" })).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "Title trop long (plus de 60 caractères), opportunité, 5" })).toBeTruthy();
});
