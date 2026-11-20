/// Ressources statiques jamais demandées pendant un crawl (règle D8).
pub const ASSET_IGNORE_REGEX: &str =
    r"(?i)\.(js|mjs|css|woff2?|ttf|otf|eot|pdf|zip|rar|7z|docx?|xlsx?|pptx?|mp3|mp4|webm)(\?|#|$)";

use std::fmt;
use std::path::Path;

use regex::Regex;
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

/// Compile les motifs comme SiteOne (`Crawler::new`) : un motif invalide est ignoré.
fn compile_patterns<'a>(patterns: impl Iterator<Item = &'a str>) -> Vec<Regex> {
    patterns
        .filter_map(|p| Regex::new(&extract_pcre_regex_pattern(p)).ok())
        .collect()
}

/// URL du sitemap à donner à SiteOne en graines. SiteOne n'applique ni robots.txt
/// ni `--include-regex` / `--ignore-regex` aux URL de `--url-list`
/// (`add_url_to_queue_static`) : on les applique ici, avec ses règles pour un lien.
pub(crate) fn crawl_seeds(settings: &CrawlSettings, sitemap: &SitemapResult) -> Vec<String> {
    let include = compile_patterns(settings.include_regex.iter().map(String::as_str));
    let ignore = compile_patterns(
        std::iter::once(ASSET_IGNORE_REGEX).chain(settings.ignore_regex.iter().map(String::as_str)),
    );
    let robots = match (&sitemap.robots_txt, settings.respect_robots) {
        (Some(body), true) => Some(RobotsTxt::parse(body)),
        _ => None,
    };
    sitemap
        .urls
        .iter()
        .filter(|url| {
            let parsed = ParsedUrl::parse(url, None);
            let full = parsed.get_full_url(true, false);
            let included = include.is_empty() || include.iter().any(|re| re.is_match(&full));
            let ignored = ignore.iter().any(|re| re.is_match(&full));
            // SiteOne ne consulte pas robots.txt pour les fichiers statiques.
            let allowed =
                parsed.is_static_file() || robots.as_ref().is_none_or(|r| r.is_allowed(url));
            included && !ignored && allowed
        })
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
