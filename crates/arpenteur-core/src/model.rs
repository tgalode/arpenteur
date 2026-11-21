use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct CrawlSettings {
    pub workers: u32,
    pub max_reqs_per_sec: f64,
    pub max_requests: u32,
    pub respect_robots: bool,
    pub user_agent: Option<String>,
    pub include_regex: Vec<String>,
    pub ignore_regex: Vec<String>,
    pub limit_threshold: u32,
}

impl Default for CrawlSettings {
    fn default() -> Self {
        Self {
            workers: 1,
            max_reqs_per_sec: 2.0,
            max_requests: 10_000,
            respect_robots: true,
            user_agent: None,
            include_regex: Vec::new(),
            ignore_regex: Vec::new(),
            limit_threshold: 5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CrawlStatus {
    Running,
    Completed,
    Stopped,
    Throttled,
    Failed { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: u32,
    pub last_url: String,
    pub last_status: i32,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageRecord {
    pub url: String,
    pub status: i32,
    pub content_type: String,
    pub size: Option<i64>,
    pub elapsed_ms: u64,
    pub headers: BTreeMap<String, String>,
    pub facts: Option<PageFacts>,
    /// Ligne ajoutée par une sonde après le crawl : jamais analysée.
    #[serde(default)]
    pub probed: bool,
}

impl PageRecord {
    pub fn location(&self) -> Option<&str> {
        self.headers.get("location").map(String::as_str)
    }

    pub fn x_robots_tag(&self) -> Option<&str> {
        self.headers.get("x-robots-tag").map(String::as_str)
    }

    pub fn is_html(&self) -> bool {
        self.content_type == "html"
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageFacts {
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Vec<String>,
    pub canonical: Option<String>,
    pub meta_robots: Option<String>,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub json_ld: Vec<JsonLd>,
    pub word_count: u32,
    pub text_hash: String,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub headings: Vec<u8>,
    #[serde(default)]
    pub unlinked_phones: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub href: String,
    pub anchor: String,
    pub nofollow: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub src: String,
    pub alt: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JsonLd {
    pub valid: bool,
    pub types: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SitemapResult {
    pub found: bool,
    pub sources: Vec<String>,
    pub urls: Vec<String>,
    pub truncated: bool,
    pub errors: Vec<String>,
    /// Corps du robots.txt lu pendant la découverte, `None` s'il est absent ou illisible.
    #[serde(default)]
    pub robots_txt: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InLink {
    pub from: String,
    pub anchor: String,
    pub nofollow: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LinkReport {
    pub inlinks: BTreeMap<String, Vec<InLink>>,
    pub depth: BTreeMap<String, u32>,
    pub redirect_chains: BTreeMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FindingCode {
    CanonicalMissing,
    CanonicalToOtherUrl,
    CanonicalToRedirect,
    CanonicalToError,
    CanonicalToOtherHost,
    TemporaryRedirect,
    RedirectChain,
    BrokenInternalLink,
    ImageTooHeavy,
    JsonLdMissing,
    JsonLdInvalid,
    OrphanPage,
    NotInSitemap,
    SitemapUrlError,
    SitemapUrlNonIndexable,
    SitemapNotFound,
    DuplicateContent,
    ThinContent,
    Noindex,
    ServerError,
    TitleMissing,
    TitleTooLong,
    TitleTooShort,
    MetaDescriptionMissing,
    MetaDescriptionTooLong,
    MetaDescriptionTooShort,
    H1Missing,
    H1Multiple,
    TitleDuplicate,
    MetaDescriptionDuplicate,
    ImageMissingAlt,
    HeadingStructure,
    HtmlLangMissing,
    PhoneNotClickable,
    ModernCompressionMissing,
    HttpsInvalid,
    SecurityHeadersMissing,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub code: FindingCode,
    pub url: String,
    pub related: Vec<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Indexability {
    Yes,
    No,
    NotAnalyzed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UrlRow {
    pub url: String,
    pub status: i32,
    pub content_type: String,
    pub title: Option<String>,
    pub title_length: Option<u32>,
    pub meta_description: Option<String>,
    pub meta_description_length: Option<u32>,
    pub h1: Option<String>,
    pub h1_count: u32,
    pub canonical: Option<String>,
    pub indexable: Indexability,
    pub depth: Option<u32>,
    pub inlinks: u32,
    pub word_count: Option<u32>,
    pub size: Option<i64>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrawlLocation {
    pub domain: String,
    pub id: String,
    pub dir: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrawlMeta {
    pub id: String,
    pub domain: String,
    pub start_url: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: CrawlStatus,
    pub settings: CrawlSettings,
    pub pages: u32,
}

/// Version des faits collectés par page : 2 depuis que la langue de la page est relevée.
pub const FACTS_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectedData {
    pub pages: Vec<PageRecord>,
    pub sitemap: SitemapResult,
    /// 0 pour un fichier écrit avant ce champ : certains faits n'y ont jamais été relevés.
    #[serde(default)]
    pub facts_version: u32,
    /// `None` : non observé ; `Some("")` : aucun `Content-Encoding` reçu.
    #[serde(default)]
    pub start_content_encoding: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SiteSummary {
    pub domain: String,
    pub crawls: Vec<CrawlMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrawlResult {
    pub meta: CrawlMeta,
    pub rows: Vec<UrlRow>,
    pub findings: Vec<Finding>,
    pub pages: Vec<PageRecord>,
    pub links: LinkReport,
    pub sitemap: SitemapResult,
}
