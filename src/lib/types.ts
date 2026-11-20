export interface CrawlSettings {
  workers: number;
  maxReqsPerSec: number;
  maxRequests: number;
  respectRobots: boolean;
  userAgent: string | null;
  includeRegex: string[];
  ignoreRegex: string[];
  limitThreshold: number;
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
  | "canonicalMissing"
  | "canonicalToOtherUrl"
  | "canonicalToRedirect"
  | "canonicalToError"
  | "canonicalToOtherHost"
  | "temporaryRedirect"
  | "redirectChain"
  | "brokenInternalLink"
  | "imageTooHeavy"
  | "jsonLdMissing"
  | "jsonLdInvalid"
  | "orphanPage"
  | "notInSitemap"
  | "sitemapUrlError"
  | "sitemapUrlNonIndexable"
  | "sitemapNotFound"
  | "duplicateContent"
  | "thinContent"
  | "noindex";

export interface Finding {
  code: FindingCode;
  url: string;
  related: string[];
}

export interface UrlRow {
  url: string;
  status: number;
  contentType: string;
  title: string | null;
  metaDescription: string | null;
  h1: string | null;
  h1Count: number;
  canonical: string | null;
  indexable: boolean;
  depth: number | null;
  inlinks: number;
  wordCount: number | null;
  size: number | null;
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

export interface CrawlResult {
  meta: CrawlMeta;
  rows: UrlRow[];
  findings: Finding[];
  pages: PageRecord[];
  links: LinkReport;
  sitemap: SitemapResult;
}

export interface StartedCrawl {
  domain: string;
  id: string;
}
