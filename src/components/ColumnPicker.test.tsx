import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { ALL_COLUMN_KEYS, DEFAULT_COLUMN_KEYS } from "../lib/columns";
import { ColumnPicker } from "./ColumnPicker";

afterEach(cleanup);

test("le bouton ouvre et ferme la liste, et annonce son état", async () => {
  const user = userEvent.setup();
  render(<ColumnPicker selected={DEFAULT_COLUMN_KEYS} onChange={() => {}} />);
  const button = screen.getByRole("button", { name: "colonnes" });
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  await user.click(button);
  expect(button.getAttribute("aria-expanded")).toBe("true");
  expect(screen.getAllByRole("checkbox")).toHaveLength(ALL_COLUMN_KEYS.length);
  await user.click(button);
  expect(button.getAttribute("aria-expanded")).toBe("false");
});

test("les cases sont dans l’ordre des colonnes et cochées selon le choix", async () => {
  const user = userEvent.setup();
  render(<ColumnPicker selected={["url", "size"]} onChange={() => {}} />);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  expect((screen.getByRole("checkbox", { name: "url" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("checkbox", { name: "poids" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("checkbox", { name: "title" }) as HTMLInputElement).checked).toBe(false);
  const names = screen.getAllByRole("checkbox").map((b) => b.closest("label")?.textContent);
  expect(names[0]).toBe("url");
  expect(names[names.length - 1]).toBe("temps");
});

test("cocher une colonne l’ajoute dans l’ordre des colonnes", async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  render(<ColumnPicker selected={["url", "size"]} onChange={onChange} />);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "title" }));
  expect(onChange).toHaveBeenCalledWith(["url", "title", "size"]);
});

test("décocher une colonne la retire", async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  render(<ColumnPicker selected={["url", "title", "size"]} onChange={onChange} />);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  await user.click(screen.getByRole("checkbox", { name: "title" }));
  expect(onChange).toHaveBeenCalledWith(["url", "size"]);
});

test("la colonne url ne peut pas être masquée", async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  render(<ColumnPicker selected={DEFAULT_COLUMN_KEYS} onChange={onChange} />);
  await user.click(screen.getByRole("button", { name: "colonnes" }));
  const url = screen.getByRole("checkbox", { name: "url" }) as HTMLInputElement;
  expect(url.checked).toBe(true);
  expect(url.disabled).toBe(true);
  await user.click(url);
  expect(onChange).not.toHaveBeenCalled();
});

test("Échap ferme la liste et rend le focus au bouton", async () => {
  const user = userEvent.setup();
  render(<ColumnPicker selected={DEFAULT_COLUMN_KEYS} onChange={() => {}} />);
  const button = screen.getByRole("button", { name: "colonnes" });
  await user.click(button);
  await user.click(screen.getByRole("checkbox", { name: "title" }));
  await user.keyboard("{Escape}");
  expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  expect(document.activeElement).toBe(button);
});
