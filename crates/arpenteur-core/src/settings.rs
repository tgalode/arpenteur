/// Ressources statiques jamais demandées pendant un crawl (règle D8).
pub const ASSET_IGNORE_REGEX: &str =
    r"(?i)\.(js|mjs|css|woff2?|ttf|otf|eot|pdf|zip|rar|7z|docx?|xlsx?|pptx?|mp3|mp4|webm)(\?|#|$)";

use std::fmt;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};
use siteone_crawler::engine::parsed_url::ParsedUrl;
use siteone_crawler::engine::robots_txt::RobotsTxt;
use siteone_crawler::options::core_options::{CoreOptions, parse_argv};
use siteone_crawler::utils::extract_pcre_regex_pattern;
use url::Url;

use crate::model::{CrawlSettings, SitemapResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlError {
    InvalidUrl(String),
    Options(String),
}

impl fmt::Display for CrawlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrawlError::InvalidUrl(input) => write!(f, "Adresse invalide : {input}"),
            CrawlError::Options(msg) => write!(f, "Réglages invalides : {msg}"),
        }
    }
}

impl std::error::Error for CrawlError {}

pub fn normalize_start_url(input: &str) -> Result<Url, CrawlError> {
    let trimmed = input.trim();
    let invalid = || CrawlError::InvalidUrl(trimmed.to_string());
    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&candidate).map_err(|_| invalid())?;
    let host_ok = url.host_str().is_some_and(|h| !h.is_empty());
    if matches!(url.scheme(), "http" | "https") && host_ok {
        Ok(url)
    } else {
        Err(invalid())
    }
}

/// Le `!` final est un drapeau SiteOne : il retire sa signature du User-Agent.
pub fn default_user_agent() -> String {
    format!("Arpenteur/{}!", env!("CARGO_PKG_VERSION"))
}

/// User-Agent des requêtes propres de l'application, sans le drapeau SiteOne.
pub fn plain_user_agent(settings: &CrawlSettings) -> String {
    let ua = settings
        .user_agent
        .clone()
        .unwrap_or_else(default_user_agent);
    ua.trim_end_matches('!').trim_end().to_string()
}

pub fn build_argv(settings: &CrawlSettings, start: &Url, crawl_dir: &Path) -> Vec<String> {
    let user_agent = settings
        .user_agent
        .clone()
        .unwrap_or_else(default_user_agent);
    let mut argv = vec![
        "siteone-crawler".to_string(),
        format!("--url={start}"),
        format!("--workers={}", settings.workers),
        format!("--max-reqs-per-sec={}", settings.max_reqs_per_sec),
        format!("--max-visited-urls={}", settings.max_requests),
        format!("--max-queue-length={}", settings.max_requests),
        "--http-cache-dir=off".to_string(),
        "--no-color".to_string(),
        "--output-text-file=".to_string(),
        format!("--user-agent={user_agent}"),
        format!(
            "--output-json-file={}",
            crawl_dir.join("siteone.json").display()
        ),
        format!(
            "--output-html-report={}",
            crawl_dir.join("siteone-report.html").display()
        ),
        format!("--ignore-regex={ASSET_IGNORE_REGEX}"),
    ];
    argv.extend(
        settings
            .include_regex
            .iter()
            .map(|r| format!("--include-regex={r}")),
    );
    argv.extend(
        settings
            .ignore_regex
            .iter()
            .map(|r| format!("--ignore-regex={r}")),
    );
    if !settings.respect_robots {
        argv.push("--ignore-robots-txt".to_string());
    }
    argv
}

pub fn build_core_options(
    settings: &CrawlSettings,
    start: &Url,
    crawl_dir: &Path,
    seeds: Vec<String>,
) -> Result<CoreOptions, CrawlError> {
    let mut options = parse_argv(&build_argv(settings, start, crawl_dir))
        .map_err(|e| CrawlError::Options(e.to_string()))?;
    options.url_list_urls = seeds;
    Ok(options)
}

/// Mode liste : SiteOne demande `first` puis `rest`, sans suivre les liens
/// (`single_page`, car `max_depth` n'empêche pas de lire les liens d'une page).
pub fn build_list_core_options(
    settings: &CrawlSettings,
    first: &Url,
    crawl_dir: &Path,
    rest: Vec<String>,
) -> Result<CoreOptions, CrawlError> {
    let mut options = build_core_options(settings, first, crawl_dir, rest)?;
    options.single_page = true;
    Ok(options)
}

/// Compile les motifs comme SiteOne (`Crawler::new`) : un motif invalide est ignoré.
fn compile_patterns<'a>(patterns: impl Iterator<Item = &'a str>) -> Vec<Regex> {
    patterns
        .filter_map(|p| Regex::new(&extract_pcre_regex_pattern(p)).ok())
        .collect()
}

/// Raison pour laquelle une URL n'est pas demandée.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SkipReason {
    Robots,
    Pattern,
}

/// Les règles de SiteOne pour un lien (motifs à inclure et à ignorer, robots.txt),
/// réutilisables hors du crawl. Les motifs sont testés avant robots.txt.
pub struct UrlFilter {
    include: Vec<Regex>,
    ignore: Vec<Regex>,
    robots: Option<RobotsTxt>,
}

impl UrlFilter {
    /// `robots_txt` est ignoré si `settings.respect_robots` est faux.
    pub fn new(settings: &CrawlSettings, robots_txt: Option<&str>) -> UrlFilter {
        UrlFilter {
            include: compile_patterns(settings.include_regex.iter().map(String::as_str)),
            ignore: compile_patterns(
                std::iter::once(ASSET_IGNORE_REGEX)
                    .chain(settings.ignore_regex.iter().map(String::as_str)),
            ),
            robots: robots_txt
                .filter(|_| settings.respect_robots)
                .map(RobotsTxt::parse),
        }
    }

    pub fn check(&self, url: &str) -> Result<(), SkipReason> {
        let parsed = ParsedUrl::parse(url, None);
        let full = parsed.get_full_url(true, false);
        let included = self.include.is_empty() || self.include.iter().any(|re| re.is_match(&full));
        if !included || self.ignore.iter().any(|re| re.is_match(&full)) {
            return Err(SkipReason::Pattern);
        }
        // SiteOne ne consulte pas robots.txt pour les fichiers statiques.
        let allowed =
            parsed.is_static_file() || self.robots.as_ref().is_none_or(|r| r.is_allowed(url));
        if allowed {
            Ok(())
        } else {
            Err(SkipReason::Robots)
        }
    }
}

/// URL du sitemap à donner à SiteOne en graines. SiteOne n'applique ni robots.txt
/// ni `--include-regex` / `--ignore-regex` aux URL de `--url-list`
/// (`add_url_to_queue_static`) : on les applique ici, avec ses règles pour un lien.
pub(crate) fn crawl_seeds(settings: &CrawlSettings, sitemap: &SitemapResult) -> Vec<String> {
    let filter = UrlFilter::new(settings, sitemap.robots_txt.as_deref());
    sitemap
        .urls
        .iter()
        .filter(|url| filter.check(url).is_ok())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start() -> Url {
        Url::parse("https://exemple.fr/").unwrap()
    }

    #[test]
    fn normalize_adds_scheme_and_trims() {
        assert_eq!(
            normalize_start_url("exemple.fr").unwrap().as_str(),
            "https://exemple.fr/"
        );
        assert_eq!(
            normalize_start_url("  https://exemple.fr/x ")
                .unwrap()
                .as_str(),
            "https://exemple.fr/x"
        );
    }

    #[test]
    fn normalize_rejects_non_urls() {
        assert!(normalize_start_url("pas une url").is_err());
        assert!(normalize_start_url("ftp://exemple.fr").is_err());
    }

    #[test]
    fn argv_carries_defaults() {
        let dir = Path::new("/tmp/crawl");
        let argv = build_argv(&CrawlSettings::default(), &start(), dir);
        assert_eq!(argv[0], "siteone-crawler");
        assert_eq!(argv[1], "--url=https://exemple.fr/");
        for expected in [
            "--workers=1",
            "--max-reqs-per-sec=2",
            "--max-visited-urls=10000",
            "--max-queue-length=10000",
            "--http-cache-dir=off",
            "--no-color",
            "--output-text-file=",
        ] {
            assert!(argv.iter().any(|a| a == expected), "{expected}");
        }
        assert!(argv.contains(&format!("--ignore-regex={ASSET_IGNORE_REGEX}")));
        assert!(argv.contains(&format!("--user-agent={}", default_user_agent())));
        assert!(argv.contains(&format!(
            "--output-json-file={}",
            dir.join("siteone.json").display()
        )));
        assert!(argv.contains(&format!(
            "--output-html-report={}",
            dir.join("siteone-report.html").display()
        )));
        assert!(
            !argv
                .iter()
                .any(|a| a == "--ignore-robots-txt" || a == "--help" || a == "--version")
        );
    }

    #[test]
    fn argv_maps_custom_settings() {
        let settings = CrawlSettings {
            respect_robots: false,
            user_agent: Some("MonBot/1.0".to_string()),
            include_regex: vec!["/blog/".to_string(), "/news/".to_string()],
            ignore_regex: vec!["/admin".to_string()],
            ..CrawlSettings::default()
        };
        let argv = build_argv(&settings, &start(), Path::new("/tmp/crawl"));
        assert!(argv.contains(&"--ignore-robots-txt".to_string()));
        assert!(argv.contains(&"--user-agent=MonBot/1.0".to_string()));
        assert!(argv.contains(&"--include-regex=/blog/".to_string()));
        assert!(argv.contains(&"--include-regex=/news/".to_string()));
        assert!(argv.contains(&"--ignore-regex=/admin".to_string()));
        assert!(argv.contains(&format!("--ignore-regex={ASSET_IGNORE_REGEX}")));
    }

    #[test]
    fn core_options_parse_and_carry_seeds() {
        let settings = CrawlSettings {
            max_reqs_per_sec: 0.5,
            ..CrawlSettings::default()
        };
        let options = build_core_options(
            &settings,
            &start(),
            Path::new("/tmp/crawl"),
            vec!["https://exemple.fr/orphan".to_string()],
        )
        .unwrap();
        assert_eq!(
            options.url_list_urls,
            vec!["https://exemple.fr/orphan".to_string()]
        );
    }

    #[test]
    fn list_options_do_not_follow_links() {
        let site = build_core_options(
            &CrawlSettings::default(),
            &start(),
            Path::new("/tmp/c"),
            vec![],
        );
        assert!(!site.unwrap().single_page);
        let options = build_list_core_options(
            &CrawlSettings::default(),
            &start(),
            Path::new("/tmp/c"),
            vec!["https://exemple.fr/a".to_string()],
        )
        .unwrap();
        assert!(options.single_page);
        assert_eq!(options.url_list_urls, ["https://exemple.fr/a"]);
    }

    #[test]
    fn crawl_seeds_apply_robots_and_regexes() {
        let sitemap = SitemapResult {
            found: true,
            urls: [
                "/",
                "/blog/a",
                "/blog/admin",
                "/private/b",
                "/blog/doc.pdf",
                "/news/c",
            ]
            .iter()
            .map(|p| format!("https://exemple.fr{p}"))
            .collect(),
            robots_txt: Some("User-agent: *\nDisallow: /private\n".to_string()),
            ..SitemapResult::default()
        };
        let seeds = |settings: &CrawlSettings| -> Vec<String> {
            crawl_seeds(settings, &sitemap)
                .iter()
                .map(|u| Url::parse(u).unwrap().path().to_string())
                .collect()
        };

        let defaults = CrawlSettings::default();
        assert_eq!(seeds(&defaults), ["/", "/blog/a", "/blog/admin", "/news/c"]);

        let filtered = CrawlSettings {
            include_regex: vec!["/blog/".to_string(), "/private/".to_string()],
            ignore_regex: vec!["/admin".to_string()],
            ..CrawlSettings::default()
        };
        assert_eq!(seeds(&filtered), ["/blog/a"]);

        let no_robots = CrawlSettings {
            respect_robots: false,
            ..filtered
        };
        assert_eq!(seeds(&no_robots), ["/blog/a", "/private/b"]);
    }

    #[test]
    fn url_filter_reports_robots_and_pattern() {
        let robots = "User-agent: *\nDisallow: /prive\n";
        let at = |p: &str| format!("https://exemple.fr{p}");

        let filter = UrlFilter::new(&CrawlSettings::default(), Some(robots));
        assert_eq!(filter.check(&at("/prive/x")), Err(SkipReason::Robots));
        assert_eq!(filter.check(&at("/public/x")), Ok(()));
        // Un fichier statique échappe à robots.txt mais pas aux motifs de ressources.
        assert_eq!(filter.check(&at("/prive/x.pdf")), Err(SkipReason::Pattern));

        let ignoring = CrawlSettings {
            ignore_regex: vec!["/tag/".to_string()],
            ..CrawlSettings::default()
        };
        let filter = UrlFilter::new(&ignoring, Some(robots));
        assert_eq!(filter.check(&at("/tag/a")), Err(SkipReason::Pattern));
        assert_eq!(filter.check(&at("/a")), Ok(()));

        let including = CrawlSettings {
            include_regex: vec!["/blog/".to_string()],
            ..CrawlSettings::default()
        };
        let filter = UrlFilter::new(&including, Some(robots));
        assert_eq!(filter.check(&at("/blog/a")), Ok(()));
        assert_eq!(filter.check(&at("/autre")), Err(SkipReason::Pattern));

        // Les deux motifs et robots.txt : le motif est signalé en premier.
        assert_eq!(filter.check(&at("/prive/x")), Err(SkipReason::Pattern));

        let lax = CrawlSettings {
            respect_robots: false,
            ..CrawlSettings::default()
        };
        let filter = UrlFilter::new(&lax, Some(robots));
        assert_eq!(filter.check(&at("/prive/x")), Ok(()));
        assert_eq!(
            UrlFilter::new(&CrawlSettings::default(), None).check(&at("/prive/x")),
            Ok(())
        );
    }

    #[test]
    fn plain_user_agent_drops_the_siteone_flag() {
        let mut settings = CrawlSettings::default();
        assert_eq!(
            plain_user_agent(&settings),
            format!("Arpenteur/{}", env!("CARGO_PKG_VERSION"))
        );
        settings.user_agent = Some("Mon-UA !".to_string());
        assert_eq!(plain_user_agent(&settings), "Mon-UA");
        settings.user_agent = Some("Mon-UA".to_string());
        assert_eq!(plain_user_agent(&settings), "Mon-UA");
    }

    #[test]
    fn crawl_error_displays_in_french() {
        assert_eq!(
            CrawlError::InvalidUrl("x y".into()).to_string(),
            "Adresse invalide : x y"
        );
        assert_eq!(
            CrawlError::Options("zut".into()).to_string(),
            "Réglages invalides : zut"
        );
    }
}
