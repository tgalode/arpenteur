import { t, type FrKey } from "../i18n/fr";
import type { FindingCode } from "./types";

export type Level = "haute" | "moyenne" | "basse" | "info";

/** Du plus grave au moins grave. */
export const LEVELS: Level[] = ["haute", "moyenne", "basse", "info"];

export const CATALOG: Record<FindingCode, { level: Level }> = {
  brokenInternalLink: { level: "haute" },
  serverError: { level: "haute" },
  canonicalToError: { level: "haute" },
  canonicalToRedirect: { level: "haute" },
  sitemapUrlError: { level: "haute" },
  titleMissing: { level: "haute" },
  httpsInvalid: { level: "haute" },
  listedUrlError: { level: "haute" },
  redirectChain: { level: "moyenne" },
  temporaryRedirect: { level: "moyenne" },
  canonicalMissing: { level: "moyenne" },
  canonicalToOtherHost: { level: "moyenne" },
  canonicalMultiple: { level: "moyenne" },
  orphanPage: { level: "moyenne" },
  duplicateContent: { level: "moyenne" },
  titleDuplicate: { level: "moyenne" },
  metaDescriptionMissing: { level: "moyenne" },
  h1Missing: { level: "moyenne" },
  jsonLdInvalid: { level: "moyenne" },
  sitemapNotFound: { level: "moyenne" },
  sitemapUrlNonIndexable: { level: "moyenne" },
  imageMissingAlt: { level: "moyenne" },
  imageTooHeavy: { level: "moyenne" },
  brokenExternalLink: { level: "moyenne" },
  titleTooLong: { level: "basse" },
  titleTooShort: { level: "basse" },
  metaDescriptionTooLong: { level: "basse" },
  metaDescriptionTooShort: { level: "basse" },
  metaDescriptionDuplicate: { level: "basse" },
  h1Multiple: { level: "basse" },
  h1Duplicate: { level: "basse" },
  h1TooLong: { level: "basse" },
  headingStructure: { level: "basse" },
  jsonLdMissing: { level: "basse" },
  htmlLangMissing: { level: "basse" },
  securityHeadersMissing: { level: "basse" },
  phoneNotClickable: { level: "basse" },
  modernCompressionMissing: { level: "basse" },
  canonicalToOtherUrl: { level: "info" },
  noindex: { level: "info" },
  notInSitemap: { level: "info" },
  thinContent: { level: "info" },
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
