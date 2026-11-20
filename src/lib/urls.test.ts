import { expect, test } from "vitest";
import { normalizeUrl } from "./urls";

test("normalizeUrl retire le fragment, minuscule l’hôte, retire le port par défaut", () => {
  expect(normalizeUrl("https://Exemple.FR:443/a#frag")).toBe("https://exemple.fr/a");
});

test("normalizeUrl ajoute / au chemin vide", () => {
  expect(normalizeUrl("https://exemple.fr")).toBe("https://exemple.fr/");
});

test("normalizeUrl laisse intacte une entrée invalide", () => {
  expect(normalizeUrl("pas une url")).toBe("pas une url");
});
