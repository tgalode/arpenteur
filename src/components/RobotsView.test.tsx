import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import type { RobotsReport } from "../lib/types";
import { RobotsView } from "./RobotsView";

afterEach(cleanup);

function robots(over: Partial<RobotsReport> = {}): RobotsReport {
  return {
    content: "User-agent: *\nDisallow: /prive\nSitemap: https://exemple.fr/sitemap.xml\n",
    status: 200,
    sitemaps: ["https://exemple.fr/sitemap.xml"],
    blocked: [{ url: "https://exemple.fr/prive?x=1", foundOn: "https://exemple.fr/" }],
    blockedKnown: true,
    respected: true,
    ...over,
  };
}

function view(over: Partial<RobotsReport> = {}, onClose = () => {}, finished = true) {
  render(<RobotsView robots={robots(over)} finished={finished} onClose={onClose} />);
  return within(screen.getByRole("region", { name: "robots.txt" }));
}

test("la vue affiche le contenu du fichier, ses sitemaps et les adresses bloquées avec leur page source", () => {
  const v = view();
  expect(v.getByText(/Disallow: \/prive/).tagName).toBe("PRE");
  const sitemaps = within(v.getByRole("list", { name: "sitemaps déclarés" }));
  expect(sitemaps.getByText("https://exemple.fr/sitemap.xml")).toBeTruthy();
  const blocked = within(v.getByRole("list", { name: "adresses bloquées" }));
  expect(blocked.getByText("https://exemple.fr/prive?x=1")).toBeTruthy();
  expect(blocked.getByText("trouvée sur https://exemple.fr/")).toBeTruthy();
});

test("le contenu est montré comme du texte, jamais interprété", () => {
  const v = view({ content: "<script>alert(1)</script><b>gras</b>" });
  expect(v.getByText("<script>alert(1)</script><b>gras</b>").tagName).toBe("PRE");
  expect(document.querySelector("b")).toBeNull();
});

test("un fichier absent affiche « introuvable »", () => {
  const v = view({ content: null, status: 404, sitemaps: [] });
  expect(v.getByText("introuvable")).toBeTruthy();
  expect(v.queryByRole("list", { name: "sitemaps déclarés" })).toBeNull();
  expect(v.getByText("aucun sitemap déclaré")).toBeTruthy();
});

test("sans rapport SiteOne sur un crawl non terminé, la vue dit que la liste est disponible pour un crawl terminé", () => {
  const v = view({ blockedKnown: false, blocked: [] }, () => {}, false);
  expect(v.getByText("liste disponible pour un crawl terminé")).toBeTruthy();
  expect(v.queryByText("liste indisponible pour ce crawl")).toBeNull();
  expect(v.queryByText("aucune adresse bloquée")).toBeNull();
});

test("sans rapport lisible sur un crawl terminé, la vue dit que la liste est indisponible", () => {
  const v = view({ blockedKnown: false, blocked: [] }, () => {}, true);
  expect(v.getByText("liste indisponible pour ce crawl")).toBeTruthy();
  expect(v.queryByText("liste disponible pour un crawl terminé")).toBeNull();
});

test("si robots.txt n’est pas respecté, une liste non vide n’est pas montrée", () => {
  const v = view({ respected: false });
  expect(v.getByText(/rien n’a été bloqué/)).toBeTruthy();
  expect(v.queryByRole("list", { name: "adresses bloquées" })).toBeNull();
  expect(v.queryByText("https://exemple.fr/prive?x=1")).toBeNull();
});

test("une liste connue et vide dit qu'aucune adresse n'a été bloquée", () => {
  const v = view({ blocked: [] });
  expect(v.getByText("aucune adresse bloquée")).toBeTruthy();
  expect(v.queryByText("liste disponible pour un crawl terminé")).toBeNull();
});

test("en mode liste sans rapport, les adresses connues sont listées sans page source", () => {
  const v = view({ blockedKnown: false, blocked: [{ url: "https://exemple.fr/prive", foundOn: null }] }, () => {}, false);
  expect(v.getByText("https://exemple.fr/prive")).toBeTruthy();
  expect(v.getByText("page source inconnue")).toBeTruthy();
  expect(v.getByText("liste disponible pour un crawl terminé")).toBeTruthy();
});

test("la vue rappelle que les fichiers (images, scripts) ne sont pas listés", () => {
  const v = view();
  expect(v.getByText(/images, scripts, feuilles de style/)).toBeTruthy();
});

test("si robots.txt n’est pas respecté, la vue dit que rien n’a été bloqué", () => {
  const v = view({ respected: false, blocked: [], blockedKnown: false });
  expect(v.getByText(/n’a pas été respecté pour ce crawl : rien n’a été bloqué/)).toBeTruthy();
  expect(v.queryByText("liste disponible pour un crawl terminé")).toBeNull();
  expect(v.queryByRole("list", { name: "adresses bloquées" })).toBeNull();
});

test("« fermer » et Échap ferment la vue", async () => {
  const user = userEvent.setup();
  const onClose = vi.fn();
  const v = view({}, onClose);
  await user.click(v.getByRole("button", { name: "fermer" }));
  expect(onClose).toHaveBeenCalledTimes(1);
  await user.keyboard("{Escape}");
  expect(onClose).toHaveBeenCalledTimes(2);
});
