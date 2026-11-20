import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { version } from "../../package.json";
import notices from "../../THIRD_PARTY_NOTICES.md?raw";
import licence from "../licenses/siteone-crawler.txt?raw";
import About from "./About";

afterEach(cleanup);

test("l’écran À propos affiche la version et la licence MIT de SiteOne", () => {
  render(<About onBack={() => {}} />);
  expect(screen.getByText(`v${version}`)).toBeTruthy();
  expect(screen.getByText(/SiteOne Crawler/, { selector: "h3" })).toBeTruthy();
  expect(screen.getByText(/Copyright \(c\) 2023-2026 Ján Regeš/)).toBeTruthy();
  expect(screen.getByText(/Permission is hereby granted, free of charge/)).toBeTruthy();
  expect(screen.getByText(/THE SOFTWARE IS PROVIDED "AS IS"/)).toBeTruthy();
});

test("À propos : retour appelle onBack", async () => {
  const onBack = vi.fn();
  render(<About onBack={onBack} />);
  await userEvent.click(screen.getByRole("button", { name: "retour" }));
  expect(onBack).toHaveBeenCalled();
});

test("les notices tierces reprennent le texte de licence affiché", () => {
  expect(notices).toContain(licence);
});
