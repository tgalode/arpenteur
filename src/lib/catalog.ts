import { t, type FrKey } from "../i18n/fr";
import type { FindingCode } from "./types";

export type Level = "haute" | "moyenne" | "basse" | "info";

/** Du plus grave au moins grave. */
export const LEVELS: Level[] = ["haute", "moyenne", "basse", "info"];

export type Kind = "probleme" | "avertissement" | "opportunite";

export const KINDS: Kind[] = ["probleme", "avertissement", "opportunite"];

export type Theme =
  | "reponses"
  | "liens"
  | "titles"
  | "metaDescriptions"
  | "headings"
  | "canonical"
  | "indexation"
  | "contenu"
  | "images"
  | "technique";

/** Dans l'ordre du tableau de la conception. */
export const THEMES: Theme[] = [
  "reponses",
  "liens",
  "titles",
  "metaDescriptions",
  "headings",
  "canonical",
  "indexation",
  "contenu",
  "images",
  "technique",
];

export const CATALOG: Record<FindingCode, { level: Level; kind: Kind; theme: Theme }> = {
  brokenInternalLink: { level: "haute", kind: "probleme", theme: "liens" },
  serverError: { level: "haute", kind: "probleme", theme: "reponses" },
  canonicalToError: { level: "haute", kind: "probleme", theme: "canonical" },
  canonicalToRedirect: { level: "haute", kind: "probleme", theme: "canonical" },
  sitemapUrlError: { level: "haute", kind: "probleme", theme: "indexation" },
  titleMissing: { level: "haute", kind: "probleme", theme: "titles" },
  httpsInvalid: { level: "haute", kind: "probleme", theme: "technique" },
  listedUrlError: { level: "haute", kind: "probleme", theme: "reponses" },
  soft404: { level: "haute", kind: "probleme", theme: "reponses" },
  soft404Site: { level: "haute", kind: "probleme", theme: "reponses" },
  redirectChain: { level: "moyenne", kind: "probleme", theme: "reponses" },
  temporaryRedirect: { level: "moyenne", kind: "avertissement", theme: "reponses" },
  canonicalMissing: { level: "moyenne", kind: "probleme", theme: "canonical" },
  canonicalToOtherHost: { level: "moyenne", kind: "avertissement", theme: "canonical" },
  canonicalMultiple: { level: "moyenne", kind: "probleme", theme: "canonical" },
  orphanPage: { level: "moyenne", kind: "avertissement", theme: "liens" },
  duplicateContent: { level: "moyenne", kind: "probleme", theme: "contenu" },
  titleDuplicate: { level: "moyenne", kind: "probleme", theme: "titles" },
  metaDescriptionMissing: { level: "moyenne", kind: "opportunite", theme: "metaDescriptions" },
  h1Missing: { level: "moyenne", kind: "probleme", theme: "headings" },
  jsonLdInvalid: { level: "moyenne", kind: "probleme", theme: "technique" },
  sitemapNotFound: { level: "moyenne", kind: "avertissement", theme: "indexation" },
  sitemapUrlNonIndexable: { level: "moyenne", kind: "probleme", theme: "indexation" },
  imageMissingAlt: { level: "moyenne", kind: "probleme", theme: "images" },
  imageTooHeavy: { level: "moyenne", kind: "opportunite", theme: "images" },
  brokenExternalLink: { level: "moyenne", kind: "probleme", theme: "liens" },
  titleTooLong: { level: "basse", kind: "opportunite", theme: "titles" },
  titleTooShort: { level: "basse", kind: "opportunite", theme: "titles" },
  metaDescriptionTooLong: { level: "basse", kind: "opportunite", theme: "metaDescriptions" },
  metaDescriptionTooShort: { level: "basse", kind: "opportunite", theme: "metaDescriptions" },
  metaDescriptionDuplicate: { level: "basse", kind: "opportunite", theme: "metaDescriptions" },
  h1Multiple: { level: "basse", kind: "avertissement", theme: "headings" },
  h1Duplicate: { level: "basse", kind: "opportunite", theme: "headings" },
  h1TooLong: { level: "basse", kind: "opportunite", theme: "headings" },
  headingStructure: { level: "basse", kind: "avertissement", theme: "headings" },
  jsonLdMissing: { level: "basse", kind: "opportunite", theme: "technique" },
  htmlLangMissing: { level: "basse", kind: "avertissement", theme: "technique" },
  securityHeadersMissing: { level: "basse", kind: "avertissement", theme: "technique" },
  phoneNotClickable: { level: "basse", kind: "opportunite", theme: "technique" },
  modernCompressionMissing: { level: "basse", kind: "opportunite", theme: "technique" },
  robotsTxtMissing: { level: "basse", kind: "avertissement", theme: "indexation" },
  canonicalToOtherUrl: { level: "info", kind: "avertissement", theme: "canonical" },
  noindex: { level: "info", kind: "avertissement", theme: "indexation" },
  notInSitemap: { level: "info", kind: "avertissement", theme: "indexation" },
  thinContent: { level: "info", kind: "opportunite", theme: "contenu" },
};

export function findingTitle(code: FindingCode): string {
  return t(`finding_${code}_title`);
}

export function findingWhy(code: FindingCode): string {
  return t(`finding_${code}_why`);
}

export function findingFix(code: FindingCode): string | null {
  if (CATALOG[code].level === "info") return null;
  return t(`finding_${code}_fix` as FrKey);
}

export function kindLabel(kind: Kind): string {
  return t(`kind_${kind}` as FrKey);
}

export function themeLabel(theme: Theme): string {
  return t(`theme_${theme}` as FrKey);
}
