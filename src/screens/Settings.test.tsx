import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { CrawlSettings } from "../lib/types";

vi.mock("../lib/api", () => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
}));

import * as api from "../lib/api";
import Settings from "./Settings";

const defaults: CrawlSettings = {
  workers: 1,
  maxReqsPerSec: 2,
  maxRequests: 10000,
  respectRobots: true,
  userAgent: null,
  includeRegex: [],
  ignoreRegex: [],
  limitThreshold: 5,
};

afterEach(cleanup);

beforeEach(() => {
  vi.mocked(api.getSettings).mockResolvedValue(defaults);
  vi.mocked(api.saveSettings).mockReset();
});

async function setField(label: string, value: string) {
  const field = await screen.findByLabelText(label);
  await userEvent.clear(field);
  await userEvent.type(field, value);
}

test("enregistrer des réglages valides appelle saveSettings", async () => {
  vi.mocked(api.saveSettings).mockResolvedValue(undefined);
  render(<Settings onBack={() => {}} />);
  await setField("connexions simultanées", "4");
  await setField("débit (requêtes par seconde)", "0,5");
  await setField("motifs à ignorer (un par ligne)", "^/panier\n\n/compte$");
  await userEvent.click(screen.getByRole("button", { name: "enregistrer" }));

  await waitFor(() =>
    expect(api.saveSettings).toHaveBeenCalledWith({
      ...defaults,
      workers: 4,
      maxReqsPerSec: 0.5,
      ignoreRegex: ["^/panier", "/compte$"],
    }),
  );
  expect(await screen.findByRole("status")).toBeTruthy();
});

test("le point décimal est accepté pour le débit", async () => {
  vi.mocked(api.saveSettings).mockResolvedValue(undefined);
  render(<Settings onBack={() => {}} />);
  await setField("débit (requêtes par seconde)", "0.5");
  await userEvent.click(screen.getByRole("button", { name: "enregistrer" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ ...defaults, maxReqsPerSec: 0.5 }));
});

test("les erreurs de validation s’affichent et rien n’est enregistré", async () => {
  vi.mocked(api.saveSettings).mockRejectedValue([
    "Débit : entre 0,1 et 20 requêtes par seconde (reçu 0).",
    "Motif à ignorer invalide : « ( » ne compile pas.",
  ]);
  render(<Settings onBack={() => {}} />);
  await setField("débit (requêtes par seconde)", "0");
  await userEvent.click(screen.getByRole("button", { name: "enregistrer" }));

  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("Débit : entre 0,1 et 20");
  expect(alert.textContent).toContain("« ( » ne compile pas");
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
  expect(screen.queryByRole("status")).toBeNull();
  expect((screen.getByLabelText("débit (requêtes par seconde)") as HTMLInputElement).value).toBe("0");
});

test("un nombre illisible est refusé sans appeler saveSettings", async () => {
  render(<Settings onBack={() => {}} />);
  await setField("connexions simultanées", "abc");
  await userEvent.click(screen.getByRole("button", { name: "enregistrer" }));
  expect((await screen.findByRole("alert")).textContent).toContain("connexions simultanées");
  expect(api.saveSettings).not.toHaveBeenCalled();
});

test("retour appelle onBack", async () => {
  const onBack = vi.fn();
  render(<Settings onBack={onBack} />);
  await userEvent.click(await screen.findByRole("button", { name: "retour" }));
  expect(onBack).toHaveBeenCalled();
});
