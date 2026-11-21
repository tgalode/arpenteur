export interface CrawlSettings {
  workers: number;
  maxReqsPerSec: number;
  maxRequests: number;
  respectRobots: boolean;
  userAgent: string | null;
  includeRegex: string[];
  ignoreRegex: string[];
  limitThreshold: number;
  checkExternalLinks: boolean;
}

export type CrawlStatus =
  | { kind: "running" }
  | { kind: "completed" }
  | { kind: "stopped" }
  | { kind: "throttled" }
  | { kind: "failed"; message: string };

export interface Progress {
  done: number;
  lastUrl: string;
  lastStatus: number;
  elapsedMs: number;
}

export interface PageFacts {
  title: string | null;
  metaDescription: string | null;
  h1: string[];
  canonical: string | null;
  metaRobots: string | null;
  links: Link[];
  images: Image[];
  jsonLd: JsonLd[];
  wordCount: number;
  textHash: string;
  lang: string | null;
  headings: number[];
  unlinkedPhones: string[];
}

export interface PageRecord {
  url: string;
  status: number;
  contentType: string;
  size: number | null;
  elapsedMs: number;
  headers: Record<string, string>;
  facts: PageFacts | null;
  probed: boolean;
}

export interface Link {
  href: string;
  anchor: string;
  nofollow: boolean;
}

export interface Image {
  src: string;
  alt: string | null;
}

export interface JsonLd {
  valid: boolean;
  types: string[];
}

export interface SitemapResult {
  found: boolean;
  sources: string[];
  urls: string[];
  truncated: boolean;
  errors: string[];
  robotsTxt: string | null;
}

export interface InLink {
  from: string;
  anchor: string;
  nofollow: boolean;
}

export interface LinkReport {
  inlinks: Record<string, InLink[]>;
  depth: Record<string, number>;
  redirectChains: Record<string, string[]>;
}

export type FindingCode =
  | "brokenInternalLink"
  | "serverError"
  | "canonicalToError"
  | "canonicalToRedirect"
  | "sitemapUrlError"
  | "titleMissing"
  | "httpsInvalid"
  | "redirectChain"
  | "temporaryRedirect"
  | "canonicalMissing"
  | "canonicalToOtherHost"
  | "orphanPage"
  | "duplicateContent"
  | "titleDuplicate"
  | "metaDescriptionMissing"
  | "h1Missing"
  | "jsonLdInvalid"
  | "sitemapNotFound"
  | "sitemapUrlNonIndexable"
  | "imageMissingAlt"
  | "imageTooHeavy"
  | "titleTooLong"
  | "titleTooShort"
  | "metaDescriptionTooLong"
  | "metaDescriptionTooShort"
  | "metaDescriptionDuplicate"
  | "h1Multiple"
  | "headingStructure"
  | "jsonLdMissing"
  | "htmlLangMissing"
  | "securityHeadersMissing"
  | "phoneNotClickable"
  | "modernCompressionMissing"
  | "canonicalToOtherUrl"
  | "noindex"
  | "notInSitemap"
  | "thinContent"
  | "brokenExternalLink";

export interface Finding {
  code: FindingCode;
  url: string;
  related: string[];
  detail: string | null;
}

export type Indexability = "yes" | "no" | "notAnalyzed";

export interface UrlRow {
  url: string;
  status: number;
  contentType: string;
  title: string | null;
  titleLength: number | null;
  metaDescription: string | null;
  metaDescriptionLength: number | null;
  h1: string | null;
  h1Count: number;
  canonical: string | null;
  indexable: Indexability;
  depth: number | null;
  inlinks: number;
  wordCount: number | null;
  size: number | null;
  elapsedMs: number;
}

export interface CrawlMeta {
  id: string;
  domain: string;
  startUrl: string;
  startedAt: string;
  finishedAt: string | null;
  status: CrawlStatus;
  settings: CrawlSettings;
  pages: number;
}

export interface SiteSummary {
  domain: string;
  crawls: CrawlMeta[];
}

export interface ExternalCheck {
  url: string;
  outcome: "ok" | "broken" | "unknown";
  /** Statut final ; null sans réponse. */
  status: number | null;
  finalUrl: string | null;
}

export interface CrawlResult {
  meta: CrawlMeta;
  rows: UrlRow[];
  findings: Finding[];
  pages: PageRecord[];
  links: LinkReport;
  sitemap: SitemapResult;
  externalLinks: ExternalCheck[];
  externalUnchecked: number;
}

export interface StartedCrawl {
  domain: string;
  id: string;
}
