use std::collections::{BTreeMap, BTreeSet};

use siteone_crawler::result::visited_url::ERROR_SKIPPED;
use url::Url;

use crate::links::analyze_links;
use crate::model::{
    CollectedData, CrawlMeta, CrawlResult, CrawlStatus, FACTS_VERSION, Finding, FindingCode,
    LinkReport, PageFacts, PageRecord, SitemapResult, UrlRow,
};
use crate::siteone::{SiteOneSummary, site_findings};
use crate::urls::{is_same_site, normalize_url};

pub const IMAGE_HEAVY_BYTES: i64 = 204_800;
pub const THIN_CONTENT_WORDS: u32 = 200;
pub const TITLE_MIN_CHARS: usize = 30;
pub const TITLE_MAX_CHARS: usize = 60;
pub const META_MIN_CHARS: usize = 70;
pub const META_MAX_CHARS: usize = 160;

/// Texte aux espaces réduits (suites, retours à la ligne, bords) ; `None` s'il est vide.
fn collapsed(text: Option<&str>) -> Option<String> {
    let joined = text?.split_whitespace().collect::<Vec<_>>().join(" ");
    (!joined.is_empty()).then_some(joined)
}

fn has_brotli(content_encoding: &str) -> bool {
    content_encoding
        .split(',')
        .any(|t| t.trim().eq_ignore_ascii_case("br"))
}

fn norm(raw: &str) -> String {
    normalize_url(raw).unwrap_or_else(|| raw.to_string())
}

/// `ERROR_SKIPPED` (-6) : SiteOne n'a pas fait la requête (trop de réponses
/// non-200 pour ce nom de fichier), ce n'est pas une erreur de la page.
/// Sur un crawl limité, 503 et 429 sont la réponse du serveur à notre débit.
fn is_error(page: &PageRecord, throttled: bool) -> bool {
    if throttled && matches!(page.status, 503 | 429) {
        return false;
    }
    page.status >= 400 || (page.status < 0 && page.status != ERROR_SKIPPED)
}

fn is_html_200(page: &PageRecord) -> bool {
    page.status == 200 && page.is_html()
}

fn has_noindex_token(value: &str) -> bool {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|t| t.to_ascii_lowercase())
        .any(|t| t == "noindex" || t == "none")
}

fn is_noindex(page: &PageRecord) -> bool {
    let meta = page
        .facts
        .as_ref()
        .and_then(|f| f.meta_robots.as_deref())
        .is_some_and(has_noindex_token);
    meta || page.x_robots_tag().is_some_and(has_noindex_token)
}

pub fn is_indexable(page: &PageRecord) -> bool {
    if !is_html_200(page) || is_noindex(page) {
        return false;
    }
    let Some(facts) = &page.facts else {
        return false;
    };
    match &facts.canonical {
        None => true,
        Some(c) => norm(c) == norm(&page.url),
    }
}

pub struct CheckInput<'a> {
    pub pages: &'a [PageRecord],
    pub links: &'a LinkReport,
    pub sitemap: &'a SitemapResult,
    pub start_url: &'a str,
    pub start_content_encoding: Option<&'a str>,
    pub facts_version: u32,
    /// Crawl limité par le serveur : ses 503 et 429 ne sont pas des erreurs du site.
    pub throttled: bool,
}

impl<'a> CheckInput<'a> {
    pub fn new(
        pages: &'a [PageRecord],
        links: &'a LinkReport,
        sitemap: &'a SitemapResult,
        start_url: &'a str,
    ) -> Self {
        Self {
            pages,
            links,
            sitemap,
            start_url,
            start_content_encoding: None,
            facts_version: FACTS_VERSION,
            throttled: false,
        }
    }
}

pub fn run_checks(input: &CheckInput) -> Vec<Finding> {
    let CheckInput {
        pages,
        links,
        sitemap,
        start_url,
        start_content_encoding,
        facts_version,
        throttled,
    } = *input;
    let start = Url::parse(&norm(start_url)).ok();
    let is_internal = |raw: &str| match (&start, Url::parse(&norm(raw))) {
        (Some(s), Ok(t)) => is_same_site(s, &t),
        _ => false,
    };
    let by_url: BTreeMap<String, &PageRecord> = pages.iter().map(|p| (norm(&p.url), p)).collect();

    let mut out: Vec<Finding> = Vec::new();
    let mut push = |code: FindingCode, url: &str, related: Vec<String>, detail: Option<String>| {
        out.push(Finding {
            code,
            url: url.to_string(),
            related,
            detail,
        });
    };

    // Une page externe (atteinte par redirection) reste une ligne du tableau,
    // sans constat : seules les pages du site sont contrôlées.
    for page in pages {
        let facts = page.facts.as_ref();
        let internal = is_internal(&page.url);

        if internal && is_html_200(page) {
            if let Some(facts) = facts {
                canonical_finding(
                    page,
                    facts.canonical.as_deref(),
                    &by_url,
                    &is_internal,
                    throttled,
                )
                .into_iter()
                .for_each(|(code, related)| push(code, &page.url, related, None));
                if facts.json_ld.is_empty() {
                    push(FindingCode::JsonLdMissing, &page.url, vec![], None);
                }
                if facts.word_count < THIN_CONTENT_WORDS {
                    push(FindingCode::ThinContent, &page.url, vec![], None);
                }
                // Une page en noindex n'a pas à être optimisée pour la recherche.
                if !is_noindex(page) {
                    match collapsed(facts.title.as_deref()).map(|t| t.chars().count()) {
                        None => push(FindingCode::TitleMissing, &page.url, vec![], None),
                        Some(n) if n < TITLE_MIN_CHARS => {
                            push(FindingCode::TitleTooShort, &page.url, vec![], None)
                        }
                        Some(n) if n > TITLE_MAX_CHARS => {
                            push(FindingCode::TitleTooLong, &page.url, vec![], None)
                        }
                        Some(_) => {}
                    }
                    match collapsed(facts.meta_description.as_deref()).map(|t| t.chars().count()) {
                        None => push(FindingCode::MetaDescriptionMissing, &page.url, vec![], None),
                        Some(n) if n < META_MIN_CHARS => push(
                            FindingCode::MetaDescriptionTooShort,
                            &page.url,
                            vec![],
                            None,
                        ),
                        Some(n) if n > META_MAX_CHARS => {
                            push(FindingCode::MetaDescriptionTooLong, &page.url, vec![], None)
                        }
                        Some(_) => {}
                    }
                    match facts.h1.len() {
                        0 => push(FindingCode::H1Missing, &page.url, vec![], None),
                        1 => {}
                        _ => push(FindingCode::H1Multiple, &page.url, vec![], None),
                    }
                }
                let no_alt: Vec<String> = facts
                    .images
                    .iter()
                    .filter(|i| i.alt.is_none())
                    .map(|i| i.src.clone())
                    .collect();
                if !no_alt.is_empty() {
                    push(FindingCode::ImageMissingAlt, &page.url, no_alt, None);
                }
                if facts.headings.windows(2).any(|w| w[1] > w[0] + 1) {
                    push(FindingCode::HeadingStructure, &page.url, vec![], None);
                }
                // Avant la version 2 la langue n'était pas relevée : `None` ne dit rien.
                if facts_version >= 2 && collapsed(facts.lang.as_deref()).is_none() {
                    push(FindingCode::HtmlLangMissing, &page.url, vec![], None);
                }
                if let Some(first) = facts.unlinked_phones.first() {
                    push(
                        FindingCode::PhoneNotClickable,
                        &page.url,
                        vec![],
                        Some(first.clone()),
                    );
                }
            }
        }
        if internal && page.is_html() {
            if facts.is_some_and(|f| f.json_ld.iter().any(|b| !b.valid)) {
                push(FindingCode::JsonLdInvalid, &page.url, vec![], None);
            }
            if is_noindex(page) {
                push(FindingCode::Noindex, &page.url, vec![], None);
            }
        }
        if internal && (500..=599).contains(&page.status) && is_error(page, throttled) {
            push(FindingCode::ServerError, &page.url, vec![], None);
        }
        if matches!(page.status, 302 | 303 | 307) && internal {
            let target = page
                .location()
                .and_then(|l| Url::parse(&norm(&page.url)).ok()?.join(l.trim()).ok())
                .map(|u| norm(u.as_str()));
            push(
                FindingCode::TemporaryRedirect,
                &page.url,
                target.into_iter().collect(),
                None,
            );
        }
        if page.content_type == "image" && page.size.is_some_and(|s| s > IMAGE_HEAVY_BYTES) {
            let me = norm(&page.url);
            let users: BTreeSet<String> = pages
                .iter()
                .filter(|p| p.is_html())
                .filter(|p| {
                    p.facts
                        .as_ref()
                        .is_some_and(|f| f.images.iter().any(|i| norm(&i.src) == me))
                })
                .map(|p| p.url.clone())
                .collect();
            push(
                FindingCode::ImageTooHeavy,
                &page.url,
                users.into_iter().collect(),
                None,
            );
        }
    }

    for (first, hops) in links
        .redirect_chains
        .iter()
        .filter(|(first, _)| is_internal(first))
    {
        push(FindingCode::RedirectChain, first, hops.clone(), None);
    }

    for (target, sources) in &links.inlinks {
        let Some(page) = by_url.get(target) else {
            continue;
        };
        if is_internal(target) && is_error(page, throttled) && !sources.is_empty() {
            let related: BTreeSet<String> = sources.iter().map(|s| s.from.clone()).collect();
            push(
                FindingCode::BrokenInternalLink,
                target,
                related.into_iter().collect(),
                None,
            );
        }
    }

    let start_norm = norm(start_url);
    let sitemap_urls: BTreeSet<String> = sitemap.urls.iter().map(|u| norm(u)).collect();
    if sitemap.found {
        for url in &sitemap_urls {
            let Some(page) = by_url.get(url) else {
                continue;
            };
            if *url != start_norm
                && page.status == 200
                && links.inlinks.get(url).is_none_or(|v| v.is_empty())
            {
                push(FindingCode::OrphanPage, &page.url, vec![], None);
            }
            if is_error(page, throttled) {
                push(FindingCode::SitemapUrlError, &page.url, vec![], None);
            } else if is_html_200(page) && !is_indexable(page) {
                push(FindingCode::SitemapUrlNonIndexable, &page.url, vec![], None);
            }
        }
        for page in pages {
            if is_internal(&page.url)
                && is_indexable(page)
                && !sitemap_urls.contains(&norm(&page.url))
            {
                push(FindingCode::NotInSitemap, &page.url, vec![], None);
            }
        }
    } else {
        push(FindingCode::SitemapNotFound, start_url, vec![], None);
    }

    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for page in pages
        .iter()
        .filter(|p| is_html_200(p) && is_internal(&p.url))
    {
        if let Some(f) = &page.facts {
            if f.word_count > 0 {
                groups.entry(&f.text_hash).or_default().push(&page.url);
            }
        }
    }
    for members in groups.values().filter(|m| m.len() >= 2) {
        for url in members {
            let mut related: Vec<String> = members
                .iter()
                .filter(|m| *m != url)
                .map(|m| m.to_string())
                .collect();
            related.sort();
            push(FindingCode::DuplicateContent, url, related, None);
        }
    }

    let indexable: Vec<&PageRecord> = pages
        .iter()
        .filter(|p| is_internal(&p.url) && is_indexable(p))
        .collect();
    for (code, text_of) in [
        (
            FindingCode::TitleDuplicate,
            (|f: &PageFacts| f.title.clone()) as fn(&PageFacts) -> Option<String>,
        ),
        (FindingCode::MetaDescriptionDuplicate, |f: &PageFacts| {
            f.meta_description.clone()
        }),
    ] {
        let mut by_text: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for page in &indexable {
            let text = page
                .facts
                .as_ref()
                .and_then(|f| collapsed(text_of(f).as_deref()));
            if let Some(text) = text {
                by_text.entry(text).or_default().push(&page.url);
            }
        }
        for members in by_text.values().filter(|m| m.len() >= 2) {
            for url in members {
                let mut related: Vec<String> = members
                    .iter()
                    .filter(|m| *m != url)
                    .map(|m| m.to_string())
                    .collect();
                related.sort();
                push(code.clone(), url, related, None);
            }
        }
    }

    // Constat de site : il vient d'une requête dédiée, les en-têtes du crawl sont décompressés.
    if let Some(encoding) = start_content_encoding.filter(|v| !has_brotli(v)) {
        push(
            FindingCode::ModernCompressionMissing,
            start_url,
            vec![],
            (!encoding.is_empty()).then(|| encoding.to_string()),
        );
    }

    out.sort_by_cached_key(|f| (f.code.clone() as usize, f.url.clone()));
    out
}

fn canonical_finding(
    page: &PageRecord,
    canonical: Option<&str>,
    by_url: &BTreeMap<String, &PageRecord>,
    is_internal: &impl Fn(&str) -> bool,
    throttled: bool,
) -> Option<(FindingCode, Vec<String>)> {
    let Some(canonical) = canonical else {
        return Some((FindingCode::CanonicalMissing, vec![]));
    };
    let target = norm(canonical);
    if target == norm(&page.url) {
        return None;
    }
    let code = match by_url.get(&target) {
        Some(t) if is_error(t, throttled) => FindingCode::CanonicalToError,
        Some(t) if (300..400).contains(&t.status) => FindingCode::CanonicalToRedirect,
        _ if !is_internal(&target) => FindingCode::CanonicalToOtherHost,
        _ => FindingCode::CanonicalToOtherUrl,
    };
    Some((code, vec![canonical.to_string()]))
}

pub fn build_rows(pages: &[PageRecord], links: &LinkReport) -> Vec<UrlRow> {
    pages
        .iter()
        .map(|p| {
            let key = norm(&p.url);
            let facts = p.facts.as_ref();
            UrlRow {
                url: p.url.clone(),
                status: p.status,
                content_type: p.content_type.clone(),
                title: facts.and_then(|f| f.title.clone()),
                meta_description: facts.and_then(|f| f.meta_description.clone()),
                h1: facts.and_then(|f| f.h1.first().cloned()),
                h1_count: facts.map_or(0, |f| f.h1.len() as u32),
                canonical: facts.and_then(|f| f.canonical.clone()),
                indexable: is_indexable(p),
                depth: links.depth.get(&key).copied(),
                inlinks: links.inlinks.get(&key).map_or(0, |v| v.len() as u32),
                word_count: facts.map(|f| f.word_count),
                size: p.size,
            }
        })
        .collect()
}

pub fn assemble(
    meta: CrawlMeta,
    data: CollectedData,
    siteone: Option<SiteOneSummary>,
) -> CrawlResult {
    let links = analyze_links(&data.pages, &meta.start_url);
    let mut findings = run_checks(&CheckInput {
        start_content_encoding: data.start_content_encoding.as_deref(),
        facts_version: data.facts_version,
        throttled: meta.status == CrawlStatus::Throttled,
        ..CheckInput::new(&data.pages, &links, &data.sitemap, &meta.start_url)
    });
    if let Some(summary) = &siteone {
        findings.extend(site_findings(summary, &meta.start_url));
    }
    let rows = build_rows(&data.pages, &links);
    CrawlResult {
        meta,
        rows,
        findings,
        pages: data.pages,
        links,
        sitemap: data.sitemap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CollectedData, Image, JsonLd, Link, PageFacts};

    const BASE: &str = "https://x.fr";
    const START: &str = "https://x.fr/";

    struct P(PageRecord);

    fn full(path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{BASE}{path}")
        }
    }

    /// Page HTML 200 sans défaut : canonical sur elle-même, JSON-LD valide, 300 mots, contenu unique.
    fn html(path: &str) -> P {
        let url = full(path);
        P(PageRecord {
            url: url.clone(),
            status: 200,
            content_type: "html".into(),
            size: Some(1000),
            elapsed_ms: 0,
            headers: BTreeMap::new(),
            facts: Some(PageFacts {
                title: Some("t".into()),
                meta_description: None,
                h1: vec!["h".into()],
                canonical: Some(url.clone()),
                meta_robots: None,
                links: vec![],
                images: vec![],
                json_ld: vec![JsonLd {
                    valid: true,
                    types: vec![],
                }],
                word_count: 300,
                text_hash: format!("hash-{url}"),
                lang: None,
                headings: Vec::new(),
                unlinked_phones: Vec::new(),
            }),
        })
    }

    fn other(path: &str, status: i32, content_type: &str) -> PageRecord {
        PageRecord {
            url: full(path),
            status,
            content_type: content_type.into(),
            size: None,
            elapsed_ms: 0,
            headers: BTreeMap::new(),
            facts: None,
        }
    }

    fn redirect(path: &str, status: i32, to: &str) -> PageRecord {
        let mut p = other(path, status, "redirect");
        p.headers.insert("location".into(), to.into());
        p
    }

    impl P {
        fn facts(&mut self) -> &mut PageFacts {
            self.0.facts.as_mut().unwrap()
        }
        fn canonical(mut self, c: Option<&str>) -> P {
            self.facts().canonical = c.map(full);
            self
        }
        fn robots(mut self, r: &str) -> P {
            self.facts().meta_robots = Some(r.into());
            self
        }
        fn header(mut self, k: &str, v: &str) -> P {
            self.0.headers.insert(k.into(), v.into());
            self
        }
        fn words(mut self, n: u32) -> P {
            self.facts().word_count = n;
            self
        }
        fn hash(mut self, h: &str) -> P {
            self.facts().text_hash = h.into();
            self
        }
        fn json_ld(mut self, blocks: &[bool]) -> P {
            self.facts().json_ld = blocks
                .iter()
                .map(|&valid| JsonLd {
                    valid,
                    types: vec![],
                })
                .collect();
            self
        }
        fn links(mut self, hrefs: &[&str]) -> P {
            self.facts().links = hrefs
                .iter()
                .map(|h| Link {
                    href: full(h),
                    anchor: String::new(),
                    nofollow: false,
                })
                .collect();
            self
        }
        fn images(mut self, srcs: &[&str]) -> P {
            self.facts().images = srcs
                .iter()
                .map(|s| Image {
                    src: full(s),
                    alt: None,
                })
                .collect();
            self
        }
        fn title(mut self, t: Option<&str>) -> P {
            self.facts().title = t.map(Into::into);
            self
        }
        fn meta(mut self, m: Option<&str>) -> P {
            self.facts().meta_description = m.map(Into::into);
            self
        }
        fn h1s(mut self, n: usize) -> P {
            self.facts().h1 = (0..n).map(|i| format!("h{i}")).collect();
            self
        }
        fn images_alt(mut self, imgs: &[(&str, Option<&str>)]) -> P {
            self.facts().images = imgs
                .iter()
                .map(|(s, a)| Image {
                    src: full(s),
                    alt: a.map(Into::into),
                })
                .collect();
            self
        }
        fn headings(mut self, levels: &[u8]) -> P {
            self.facts().headings = levels.to_vec();
            self
        }
        fn lang(mut self, l: Option<&str>) -> P {
            self.facts().lang = l.map(Into::into);
            self
        }
        fn phones(mut self, p: &[&str]) -> P {
            self.facts().unlinked_phones = p.iter().map(|s| s.to_string()).collect();
            self
        }
        fn rec(self) -> PageRecord {
            self.0
        }
    }

    fn sitemap(paths: &[&str]) -> SitemapResult {
        SitemapResult {
            found: true,
            sources: vec![],
            urls: paths.iter().map(|p| full(p)).collect(),
            truncated: false,
            errors: vec![],
            robots_txt: None,
        }
    }

    fn no_sitemap() -> SitemapResult {
        SitemapResult::default()
    }

    fn check_from(pages: &[PageRecord], sm: &SitemapResult, start: &str) -> Vec<Finding> {
        let links = analyze_links(pages, start);
        run_checks(&CheckInput::new(pages, &links, sm, start))
    }

    fn check(pages: &[PageRecord], sm: &SitemapResult) -> Vec<Finding> {
        check_from(pages, sm, START)
    }

    fn of(findings: &[Finding], code: FindingCode) -> Vec<&Finding> {
        findings.iter().filter(|f| f.code == code).collect()
    }

    fn urls_of(findings: &[Finding], code: FindingCode) -> Vec<String> {
        of(findings, code).iter().map(|f| f.url.clone()).collect()
    }

    #[test]
    fn canonical_missing_only_on_html_200() {
        let pages = vec![
            html("/a").canonical(None).rec(),
            html("/ok").rec(),
            other("/gone", 404, ""),
            other("/img.png", 200, "image"),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(urls_of(&f, FindingCode::CanonicalMissing), vec![full("/a")]);
    }

    #[test]
    fn self_canonical_is_not_flagged() {
        let pages = vec![html("/a").rec()];
        let f = check(&pages, &no_sitemap());
        for code in [
            FindingCode::CanonicalMissing,
            FindingCode::CanonicalToOtherUrl,
            FindingCode::CanonicalToRedirect,
            FindingCode::CanonicalToError,
            FindingCode::CanonicalToOtherHost,
        ] {
            assert!(of(&f, code).is_empty());
        }
    }

    #[test]
    fn canonical_to_other_url_is_reported() {
        let pages = vec![html("/a").canonical(Some("/b")).rec(), html("/b").rec()];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::CanonicalToOtherUrl);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, full("/a"));
        assert_eq!(found[0].related, vec![full("/b")]);
        assert!(of(&f, FindingCode::CanonicalToRedirect).is_empty());
    }

    #[test]
    fn canonical_to_redirect() {
        let pages = vec![
            html("/a").canonical(Some("/old")).rec(),
            redirect("/old", 301, "/b"),
            html("/b").rec(),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(
            urls_of(&f, FindingCode::CanonicalToRedirect),
            vec![full("/a")]
        );
        assert!(of(&f, FindingCode::CanonicalToOtherUrl).is_empty());
    }

    #[test]
    fn canonical_to_error() {
        let pages = vec![
            html("/a").canonical(Some("/gone")).rec(),
            other("/gone", 404, ""),
            html("/b").canonical(Some("/dead")).rec(),
            other("/dead", -1, ""),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(
            urls_of(&f, FindingCode::CanonicalToError),
            vec![full("/a"), full("/b")]
        );
        assert!(of(&f, FindingCode::CanonicalToOtherUrl).is_empty());
    }

    #[test]
    fn canonical_to_other_host() {
        let pages = vec![html("/a").canonical(Some("https://autre.com/a")).rec()];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::CanonicalToOtherHost);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].related, vec!["https://autre.com/a".to_string()]);
        assert!(of(&f, FindingCode::CanonicalToOtherUrl).is_empty());
    }

    #[test]
    fn apex_canonical_that_redirects_is_redirect_not_other_host() {
        let www = "https://www.x.fr";
        let pages = vec![
            html(&format!("{www}/a"))
                .canonical(Some("https://x.fr/a"))
                .rec(),
            redirect("https://x.fr/a", 301, &format!("{www}/a")),
        ];
        let f = check_from(&pages, &no_sitemap(), "https://www.x.fr/");
        assert_eq!(
            urls_of(&f, FindingCode::CanonicalToRedirect),
            vec![format!("{www}/a")]
        );
        assert!(of(&f, FindingCode::CanonicalToOtherHost).is_empty());
    }

    #[test]
    fn temporary_redirect_on_internal_url_only() {
        let pages = vec![
            redirect("/r302", 302, "/a"),
            redirect("/r303", 303, "/a"),
            redirect("/r307", 307, "/a"),
            redirect("/r301", 301, "/a"),
            redirect("/r308", 308, "/a"),
            redirect("https://autre.com/r302", 302, "/a"),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(
            urls_of(&f, FindingCode::TemporaryRedirect),
            vec![full("/r302"), full("/r303"), full("/r307")]
        );
        assert_eq!(
            of(&f, FindingCode::TemporaryRedirect)[0].related,
            vec![full("/a")]
        );
    }

    #[test]
    fn redirect_chain_reported_with_hops() {
        let pages = vec![
            redirect("/a", 301, "/b"),
            redirect("/b", 301, "/c"),
            html("/c").rec(),
            redirect("/s", 301, "/c"),
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::RedirectChain);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, full("/a"));
        assert_eq!(found[0].related, vec![full("/b"), full("/c")]);
    }

    #[test]
    fn broken_internal_link_lists_all_sources() {
        let pages = vec![
            html("/")
                .links(&["/missing", "/dead", "/ok", "/never"])
                .rec(),
            html("/a").links(&["/missing"]).rec(),
            html("/ok").rec(),
            other("/missing", 404, ""),
            other("/dead", -1, ""),
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::BrokenInternalLink);
        assert_eq!(found.len(), 2);
        let missing = found.iter().find(|x| x.url == full("/missing")).unwrap();
        assert_eq!(missing.related, vec![full("/"), full("/a")]);
        let dead = found.iter().find(|x| x.url == full("/dead")).unwrap();
        assert_eq!(dead.related, vec![full("/")]);
    }

    #[test]
    fn image_too_heavy_threshold() {
        let mut at = other("/at.jpg", 200, "image");
        at.size = Some(IMAGE_HEAVY_BYTES);
        let mut over = other("/over.jpg", 200, "image");
        over.size = Some(IMAGE_HEAVY_BYTES + 1);
        let pages = vec![
            html("/b").images(&["/over.jpg"]).rec(),
            html("/a").images(&["/over.jpg", "/at.jpg"]).rec(),
            at,
            over,
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::ImageTooHeavy);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, full("/over.jpg"));
        assert_eq!(found[0].related, vec![full("/a"), full("/b")]);
    }

    #[test]
    fn json_ld_missing_and_invalid() {
        let pages = vec![
            html("/none").json_ld(&[]).rec(),
            html("/bad").json_ld(&[true, false, false]).rec(),
            html("/good").json_ld(&[true]).rec(),
            other("/gone", 404, ""),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(urls_of(&f, FindingCode::JsonLdMissing), vec![full("/none")]);
        assert_eq!(urls_of(&f, FindingCode::JsonLdInvalid), vec![full("/bad")]);
    }

    #[test]
    fn orphan_page_from_sitemap() {
        let pages = vec![
            html("/").links(&["/linked"]).rec(),
            html("/linked").rec(),
            html("/orphan").rec(),
            other("/gone", 404, ""),
        ];
        let sm = sitemap(&["/", "/linked", "/orphan", "/gone", "/never"]);
        let f = check(&pages, &sm);
        assert_eq!(urls_of(&f, FindingCode::OrphanPage), vec![full("/orphan")]);
    }

    #[test]
    fn not_in_sitemap_only_when_sitemap_found() {
        let pages = vec![
            html("/").rec(),
            html("/in").rec(),
            html("/out").rec(),
            html("/noidx").robots("noindex").rec(),
            html("https://autre.com/ext").rec(),
        ];
        let f = check(&pages, &sitemap(&["/", "/in"]));
        assert_eq!(urls_of(&f, FindingCode::NotInSitemap), vec![full("/out")]);
        let f = check(&pages, &no_sitemap());
        assert!(of(&f, FindingCode::NotInSitemap).is_empty());
    }

    #[test]
    fn sitemap_url_error_and_non_indexable() {
        let pages = vec![
            other("/gone", 404, ""),
            other("/dead", -1, ""),
            html("/noidx").robots("noindex").rec(),
            html("/ok").rec(),
        ];
        let sm = sitemap(&["/gone", "/dead", "/noidx", "/ok", "/never"]);
        let f = check(&pages, &sm);
        assert_eq!(
            urls_of(&f, FindingCode::SitemapUrlError),
            vec![full("/dead"), full("/gone")]
        );
        assert_eq!(
            urls_of(&f, FindingCode::SitemapUrlNonIndexable),
            vec![full("/noidx")]
        );
    }

    #[test]
    fn url_skipped_by_siteone_is_not_an_error() {
        let pages = vec![
            html("/").links(&["/ignoree"]).rec(),
            other("/ignoree", -6, ""),
            other("/ignoree-aussi", -6, ""),
        ];
        let f = check(&pages, &sitemap(&["/", "/ignoree-aussi"]));
        assert!(of(&f, FindingCode::BrokenInternalLink).is_empty(), "{f:?}");
        assert!(of(&f, FindingCode::SitemapUrlError).is_empty(), "{f:?}");
    }

    #[test]
    fn sitemap_not_found_reported_once() {
        let pages = vec![html("/").rec()];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::SitemapNotFound);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, START);
        let f = check(&pages, &sitemap(&["/"]));
        assert!(of(&f, FindingCode::SitemapNotFound).is_empty());
    }

    #[test]
    fn duplicate_content_groups_pages() {
        let pages = vec![
            html("/d1").hash("same").rec(),
            html("/d2").hash("same").rec(),
            html("/d3").hash("same").rec(),
            html("/u").hash("unique").rec(),
            html("/e1").words(0).hash("empty").rec(),
            html("/e2").words(0).hash("empty").rec(),
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::DuplicateContent);
        assert_eq!(found.len(), 3);
        let d1 = found.iter().find(|x| x.url == full("/d1")).unwrap();
        assert_eq!(d1.related, vec![full("/d2"), full("/d3")]);
        assert!(found.iter().all(|x| x.url != full("/u")));
        assert!(found.iter().all(|x| !x.url.contains("/e")));
    }

    #[test]
    fn thin_content_threshold() {
        let pages = vec![
            html("/thin").words(THIN_CONTENT_WORDS - 1).rec(),
            html("/zero").words(0).rec(),
            html("/ok").words(THIN_CONTENT_WORDS).rec(),
            other("/img.png", 200, "image"),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(
            urls_of(&f, FindingCode::ThinContent),
            vec![full("/thin"), full("/zero")]
        );
    }

    #[test]
    fn noindex_from_meta_or_header() {
        let pages = vec![
            html("/meta").robots("Index, NoIndex").rec(),
            html("/none").robots("none").rec(),
            html("/header")
                .header("x-robots-tag", "googlebot: noindex")
                .rec(),
            html("/fine").robots("index, follow").rec(),
            html("/plain").rec(),
        ];
        let f = check(&pages, &no_sitemap());
        assert_eq!(
            urls_of(&f, FindingCode::Noindex),
            vec![full("/header"), full("/meta"), full("/none")]
        );
    }

    #[test]
    fn is_indexable_rules() {
        assert!(is_indexable(&html("/a").rec()));
        assert!(!is_indexable(&html("/a").robots("noindex").rec()));
        assert!(!is_indexable(
            &html("/a").header("x-robots-tag", "none").rec()
        ));
        assert!(!is_indexable(&html("/a").canonical(Some("/b")).rec()));
        assert!(is_indexable(&html("/a").canonical(None).rec()));
        assert!(!is_indexable(&other("/a", 404, "")));
        assert!(!is_indexable(&other("/a", 200, "image")));
    }

    #[test]
    fn build_rows_maps_pages_in_order() {
        let pages = vec![
            html("/").links(&["/a"]).rec(),
            html("/a").rec(),
            other("/gone", 404, ""),
        ];
        let links = analyze_links(&pages, START);
        let rows = build_rows(&pages, &links);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].url, full("/"));
        assert_eq!(rows[0].depth, Some(0));
        assert_eq!(rows[0].inlinks, 0);
        assert_eq!(rows[0].h1.as_deref(), Some("h"));
        assert_eq!(rows[0].h1_count, 1);
        assert!(rows[0].indexable);
        assert_eq!(rows[1].depth, Some(1));
        assert_eq!(rows[1].inlinks, 1);
        assert_eq!(rows[1].word_count, Some(300));
        assert_eq!(rows[2].word_count, None);
        assert_eq!(rows[2].depth, None);
        assert!(!rows[2].indexable);
    }

    #[test]
    fn external_pages_reached_by_redirect_have_no_finding() {
        let landing = "https://autre.com/landing";
        let invalid = "https://autre.com/invalide";
        let pages = vec![
            html("/").links(&["/go", "/go2", "/go3"]).hash("meme").rec(),
            redirect("/go", 301, landing),
            redirect("/go2", 301, invalid),
            redirect("/go3", 301, "https://autre.com/r1"),
            html(landing)
                .canonical(None)
                .json_ld(&[])
                .words(10)
                .robots("noindex")
                .hash("meme")
                .rec(),
            html(invalid)
                .canonical(Some("https://autre.com/x"))
                .json_ld(&[false])
                .rec(),
            redirect("https://autre.com/r1", 301, "https://autre.com/r2"),
            redirect("https://autre.com/r2", 301, "https://autre.com/r3"),
        ];
        let f = check(&pages, &sitemap(&["/"]));
        assert!(
            f.iter().all(|x| !x.url.starts_with("https://autre.com")),
            "{f:?}"
        );
        assert!(of(&f, FindingCode::DuplicateContent).is_empty());
    }

    #[test]
    fn external_page_linking_to_sitemap_url_does_not_hide_orphan() {
        let pages = vec![
            html("/").links(&["/go"]).rec(),
            redirect("/go", 301, "https://autre.com/landing"),
            html("https://autre.com/landing").links(&["/orphan"]).rec(),
            html("/orphan").rec(),
        ];
        let links = analyze_links(&pages, START);
        assert!(!links.inlinks.contains_key(&full("/orphan")));
        let sm = sitemap(&["/", "/orphan"]);
        let f = run_checks(&CheckInput::new(&pages, &links, &sm, START));
        assert_eq!(urls_of(&f, FindingCode::OrphanPage), vec![full("/orphan")]);
    }

    #[test]
    fn findings_are_sorted_by_code_then_url() {
        let pages = vec![
            html("/z").words(1).rec(),
            html("/a").words(1).rec(),
            html("/m").canonical(None).rec(),
        ];
        let f = check(&pages, &no_sitemap());
        let keys: Vec<(usize, String)> = f
            .iter()
            .map(|x| (x.code.clone() as usize, x.url.clone()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    /// Page sans aucun défaut des nouveaux contrôles de page.
    fn clean(path: &str) -> P {
        html(path)
            .title(Some(&"t".repeat(40)))
            .meta(Some(&"m".repeat(100)))
            .lang(Some("fr"))
            .header("content-encoding", "br")
    }

    fn throttled_run(pages: &[PageRecord], sm: &SitemapResult, throttled: bool) -> Vec<Finding> {
        let links = analyze_links(pages, START);
        run_checks(&CheckInput {
            throttled,
            ..CheckInput::new(pages, &links, sm, START)
        })
    }

    fn throttle_pages(status: i32) -> Vec<PageRecord> {
        vec![
            clean("/")
                .links(&["/limite"])
                .canonical(Some("/limite"))
                .rec(),
            other("/limite", status, "html"),
        ]
    }

    const ERROR_CODES: [FindingCode; 4] = [
        FindingCode::ServerError,
        FindingCode::BrokenInternalLink,
        FindingCode::SitemapUrlError,
        FindingCode::CanonicalToError,
    ];

    #[test]
    fn throttled_crawl_ignores_503_and_429() {
        let sm = sitemap(&["/", "/limite"]);
        for status in [503, 429] {
            let f = throttled_run(&throttle_pages(status), &sm, true);
            for code in ERROR_CODES {
                assert!(of(&f, code.clone()).is_empty(), "{status} {code:?}");
            }
        }
        let f = throttled_run(&throttle_pages(500), &sm, true);
        for code in ERROR_CODES {
            assert_eq!(of(&f, code.clone()).len(), 1, "500 {code:?}");
        }
        let f = throttled_run(&throttle_pages(503), &sm, false);
        for code in ERROR_CODES {
            assert_eq!(of(&f, code.clone()).len(), 1, "non limité 503 {code:?}");
        }
        let f = throttled_run(&throttle_pages(429), &sm, false);
        for code in &ERROR_CODES[1..] {
            assert_eq!(of(&f, code.clone()).len(), 1, "non limité 429 {code:?}");
        }
    }

    fn noindex_codes() -> [FindingCode; 8] {
        [
            FindingCode::TitleMissing,
            FindingCode::TitleTooLong,
            FindingCode::TitleTooShort,
            FindingCode::MetaDescriptionMissing,
            FindingCode::MetaDescriptionTooLong,
            FindingCode::MetaDescriptionTooShort,
            FindingCode::H1Missing,
            FindingCode::H1Multiple,
        ]
    }

    #[test]
    fn noindex_page_skips_title_meta_and_h1_checks() {
        let bare = |p: P| {
            p.title(None)
                .meta(None)
                .h1s(0)
                .images_alt(&[("/i.png", None)])
                .lang(None)
        };
        let f = check(&[bare(html("/a").robots("noindex")).rec()], &no_sitemap());
        for code in noindex_codes() {
            assert!(of(&f, code.clone()).is_empty(), "{code:?}");
        }
        assert_eq!(of(&f, FindingCode::Noindex).len(), 1);
        assert_eq!(of(&f, FindingCode::ImageMissingAlt).len(), 1);
        assert_eq!(of(&f, FindingCode::HtmlLangMissing).len(), 1);

        let f = check(&[bare(html("/a")).rec()], &no_sitemap());
        for code in [
            FindingCode::TitleMissing,
            FindingCode::MetaDescriptionMissing,
            FindingCode::H1Missing,
        ] {
            assert_eq!(of(&f, code.clone()).len(), 1, "{code:?}");
        }
    }

    #[test]
    fn noindex_page_skips_too_long_too_short_and_multiple_h1() {
        let long = html("/long")
            .robots("noindex")
            .title(Some(&"t".repeat(80)))
            .meta(Some(&"m".repeat(200)))
            .h1s(2);
        let short = html("/short")
            .robots("noindex")
            .title(Some("t"))
            .meta(Some("m"));
        let f = check(&[long.rec(), short.rec()], &no_sitemap());
        for code in noindex_codes() {
            assert!(of(&f, code.clone()).is_empty(), "{code:?}");
        }
    }

    #[test]
    fn noindex_by_x_robots_tag_also_skips() {
        let page = html("/a")
            .header("x-robots-tag", "noindex")
            .title(None)
            .meta(None)
            .h1s(0)
            .rec();
        let f = check(&[page], &no_sitemap());
        for code in noindex_codes() {
            assert!(of(&f, code.clone()).is_empty(), "{code:?}");
        }
        assert_eq!(of(&f, FindingCode::Noindex).len(), 1);
    }

    fn only(pages: &[PageRecord], code: FindingCode) -> Vec<String> {
        urls_of(&check(pages, &no_sitemap()), code)
    }

    #[test]
    fn server_error_on_internal_5xx() {
        let pages = vec![
            other("/boom", 503, "html"),
            other("/json", 500, "json"),
            other("/gone", 404, "html"),
            other("/limite", 599, ""),
            other("/ok", 200, "html"),
            other("/lim", 600, ""),
            other("https://autre.com/x", 500, "html"),
        ];
        assert_eq!(
            only(&pages, FindingCode::ServerError),
            vec![full("/boom"), full("/json"), full("/limite")]
        );
    }

    #[test]
    fn title_rules() {
        let pages = vec![
            clean("/none").title(None).rec(),
            clean("/blank").title(Some("   ")).rec(),
            clean("/c29").title(Some(&"a".repeat(29))).rec(),
            clean("/c30").title(Some(&"a".repeat(30))).rec(),
            clean("/c60").title(Some(&"a".repeat(60))).rec(),
            clean("/c61").title(Some(&"a".repeat(61))).rec(),
            clean("/spaces")
                .title(Some(&format!("{}   {}", "a".repeat(15), "b".repeat(13))))
                .rec(),
        ];
        assert_eq!(
            only(&pages, FindingCode::TitleMissing),
            vec![full("/blank"), full("/none")]
        );
        assert_eq!(
            only(&pages, FindingCode::TitleTooShort),
            vec![full("/c29"), full("/spaces")]
        );
        assert_eq!(only(&pages, FindingCode::TitleTooLong), vec![full("/c61")]);
    }

    #[test]
    fn meta_description_rules() {
        let m = |n: usize| Some("m".repeat(n));
        let pages = vec![
            clean("/none").meta(None).rec(),
            clean("/c69").meta(m(69).as_deref()).rec(),
            clean("/c70").meta(m(70).as_deref()).rec(),
            clean("/c160").meta(m(160).as_deref()).rec(),
            clean("/c161").meta(m(161).as_deref()).rec(),
        ];
        assert_eq!(
            only(&pages, FindingCode::MetaDescriptionMissing),
            vec![full("/none")]
        );
        assert_eq!(
            only(&pages, FindingCode::MetaDescriptionTooShort),
            vec![full("/c69")]
        );
        assert_eq!(
            only(&pages, FindingCode::MetaDescriptionTooLong),
            vec![full("/c161")]
        );
    }

    #[test]
    fn h1_rules() {
        let pages = vec![
            clean("/zero").h1s(0).rec(),
            clean("/one").h1s(1).rec(),
            clean("/two").h1s(2).rec(),
        ];
        assert_eq!(only(&pages, FindingCode::H1Missing), vec![full("/zero")]);
        assert_eq!(only(&pages, FindingCode::H1Multiple), vec![full("/two")]);
    }

    #[test]
    fn duplicates_only_among_indexable_pages() {
        let t = Some("Un titre assez long pour passer les bornes");
        let m =
            Some("Une description suffisamment longue pour passer les bornes de longueur, oui.");
        let pages = vec![
            clean("/a").title(t).meta(m).rec(),
            clean("/b").title(t).meta(m).rec(),
            clean("/c")
                .title(Some("Titre unique de trente caracteres ok"))
                .rec(),
        ];
        let f = check(&pages, &no_sitemap());
        let dup = of(&f, FindingCode::TitleDuplicate);
        assert_eq!(dup.len(), 2);
        assert_eq!(dup[0].url, full("/a"));
        assert_eq!(dup[0].related, vec![full("/b")]);
        assert_eq!(dup[1].related, vec![full("/a")]);
        let dup = of(&f, FindingCode::MetaDescriptionDuplicate);
        assert_eq!(dup.len(), 2);
        assert_eq!(dup[0].related, vec![full("/b")]);

        // une page en noindex n'est pas indexable : plus de doublon
        let pages = vec![
            clean("/a").title(t).meta(m).rec(),
            clean("/b").title(t).meta(m).robots("noindex").rec(),
        ];
        let f = check(&pages, &no_sitemap());
        assert!(of(&f, FindingCode::TitleDuplicate).is_empty());
        assert!(of(&f, FindingCode::MetaDescriptionDuplicate).is_empty());

        // titres absents : pas un doublon
        let pages = vec![
            clean("/a").title(None).meta(None).rec(),
            clean("/b").title(None).meta(None).rec(),
        ];
        let f = check(&pages, &no_sitemap());
        assert!(of(&f, FindingCode::TitleDuplicate).is_empty());
        assert!(of(&f, FindingCode::MetaDescriptionDuplicate).is_empty());
    }

    #[test]
    fn image_missing_alt_ignores_decorative() {
        let pages = vec![
            clean("/a")
                .images_alt(&[
                    ("/i.png", None),
                    ("/j.png", Some("")),
                    ("/k.png", Some("chat")),
                ])
                .rec(),
            clean("/b").images_alt(&[("/j.png", Some(""))]).rec(),
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::ImageMissingAlt);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, full("/a"));
        assert_eq!(found[0].related, vec![full("/i.png")]);
    }

    #[test]
    fn heading_structure_detects_skips() {
        let pages = vec![
            clean("/skip").headings(&[1, 3]).rec(),
            clean("/ok").headings(&[1, 2, 3, 2]).rec(),
            clean("/down").headings(&[2, 3]).rec(),
            clean("/up").headings(&[1, 2, 3, 1, 2]).rec(),
            clean("/none").headings(&[]).rec(),
        ];
        assert_eq!(
            only(&pages, FindingCode::HeadingStructure),
            vec![full("/skip")]
        );
    }

    #[test]
    fn html_lang_missing() {
        let pages = vec![
            clean("/none").lang(None).rec(),
            clean("/blank").lang(Some(" ")).rec(),
            clean("/fr").lang(Some("fr")).rec(),
        ];
        assert_eq!(
            only(&pages, FindingCode::HtmlLangMissing),
            vec![full("/blank"), full("/none")]
        );
    }

    #[test]
    fn phone_not_clickable() {
        let pages = vec![
            clean("/tel")
                .phones(&["01 23 45 67 89", "02 34 56 78 90"])
                .rec(),
            clean("/ok").phones(&[]).rec(),
        ];
        let f = check(&pages, &no_sitemap());
        let found = of(&f, FindingCode::PhoneNotClickable);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, full("/tel"));
        assert_eq!(found[0].detail.as_deref(), Some("01 23 45 67 89"));
    }

    #[test]
    fn brotli_missing_is_one_site_finding() {
        let pages = vec![clean("/a").rec(), other("/gone", 404, "html")];
        let with = |enc: Option<&str>| {
            let links = analyze_links(&pages, START);
            let sm = no_sitemap();
            run_checks(&CheckInput {
                start_content_encoding: enc,
                ..CheckInput::new(&pages, &links, &sm, START)
            })
        };

        let f = with(Some("gzip"));
        let found = of(&f, FindingCode::ModernCompressionMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, START);
        assert_eq!(found[0].detail.as_deref(), Some("gzip"));

        let f = with(Some(""));
        let found = of(&f, FindingCode::ModernCompressionMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, START);
        assert_eq!(found[0].detail, None);

        for enc in [Some("br"), Some("gzip, br"), Some("BR"), None] {
            let f = with(enc);
            assert!(
                of(&f, FindingCode::ModernCompressionMissing).is_empty(),
                "{enc:?}"
            );
        }
    }

    #[test]
    fn missing_lang_is_reported_only_from_facts_version_2() {
        let pages = vec![html("/").lang(None).rec()];
        let links = analyze_links(&pages, START);
        let run = |version: u32| {
            let sm = no_sitemap();
            run_checks(&CheckInput {
                facts_version: version,
                ..CheckInput::new(&pages, &links, &sm, START)
            })
            .iter()
            .any(|f| f.code == FindingCode::HtmlLangMissing)
        };
        assert!(!run(0));
        assert!(!run(1));
        assert!(run(2));
    }

    #[test]
    fn collected_data_without_facts_version_loads_as_zero() {
        let mut json = serde_json::to_value(CollectedData::default()).unwrap();
        json.as_object_mut().unwrap().remove("factsVersion");
        let data: CollectedData = serde_json::from_value(json).unwrap();
        assert_eq!(data.facts_version, 0);
    }

    #[test]
    fn collected_data_without_start_content_encoding_loads() {
        let mut json = serde_json::to_value(CollectedData::default()).unwrap();
        json.as_object_mut().unwrap().remove("startContentEncoding");
        let data: CollectedData = serde_json::from_value(json).unwrap();
        assert_eq!(data.start_content_encoding, None);
    }

    #[test]
    fn checks_only_internal_html_200() {
        let bad = |url: &str, status: i32| {
            let mut p = html(url)
                .title(None)
                .meta(None)
                .h1s(0)
                .lang(None)
                .headings(&[1, 3])
                .phones(&["01 23 45 67 89"])
                .images_alt(&[("/i.png", None)])
                .rec();
            p.status = status;
            p
        };
        let pages = vec![bad("https://autre.com/a", 200), bad("/gone", 404)];
        let f = check(&pages, &no_sitemap());
        for code in [
            FindingCode::TitleMissing,
            FindingCode::MetaDescriptionMissing,
            FindingCode::H1Missing,
            FindingCode::HtmlLangMissing,
            FindingCode::HeadingStructure,
            FindingCode::PhoneNotClickable,
            FindingCode::ImageMissingAlt,
        ] {
            assert!(of(&f, code.clone()).is_empty(), "{code:?}");
        }
    }

    #[test]
    fn findings_without_detail_load_from_older_saves() {
        let f: Finding =
            serde_json::from_str(r#"{"code":"noindex","url":"https://x.fr/","related":[]}"#)
                .unwrap();
        assert_eq!(f.detail, None);
    }

    /// Match exhaustif : ajouter une variante sans l'inscrire ici ne compile plus.
    fn declared_index(code: &FindingCode) -> usize {
        use FindingCode::*;
        match code {
            CanonicalMissing => 0,
            CanonicalToOtherUrl => 1,
            CanonicalToRedirect => 2,
            CanonicalToError => 3,
            CanonicalToOtherHost => 4,
            TemporaryRedirect => 5,
            RedirectChain => 6,
            BrokenInternalLink => 7,
            ImageTooHeavy => 8,
            JsonLdMissing => 9,
            JsonLdInvalid => 10,
            OrphanPage => 11,
            NotInSitemap => 12,
            SitemapUrlError => 13,
            SitemapUrlNonIndexable => 14,
            SitemapNotFound => 15,
            DuplicateContent => 16,
            ThinContent => 17,
            Noindex => 18,
            ServerError => 19,
            TitleMissing => 20,
            TitleTooLong => 21,
            TitleTooShort => 22,
            MetaDescriptionMissing => 23,
            MetaDescriptionTooLong => 24,
            MetaDescriptionTooShort => 25,
            H1Missing => 26,
            H1Multiple => 27,
            TitleDuplicate => 28,
            MetaDescriptionDuplicate => 29,
            ImageMissingAlt => 30,
            HeadingStructure => 31,
            HtmlLangMissing => 32,
            PhoneNotClickable => 33,
            ModernCompressionMissing => 34,
            HttpsInvalid => 35,
            SecurityHeadersMissing => 36,
        }
    }

    #[test]
    fn finding_codes_match_shared_list() {
        use FindingCode::*;
        let all = [
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
        ];
        for (i, code) in all.iter().enumerate() {
            assert_eq!(declared_index(code), i, "ordre de déclaration : {code:?}");
            assert_eq!(code.clone() as usize, i);
        }
        let serialized: Vec<String> = all
            .iter()
            .map(|c| {
                serde_json::to_value(c)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        let shared: Vec<String> =
            serde_json::from_str(include_str!("../../../src/lib/finding-codes.json")).unwrap();
        assert_eq!(serialized, shared);
    }
}
