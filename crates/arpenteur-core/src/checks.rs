use std::collections::{BTreeMap, BTreeSet};

use siteone_crawler::result::visited_url::ERROR_SKIPPED;
use url::Url;

use crate::links::analyze_links;
use crate::model::{
    CollectedData, CrawlMeta, CrawlResult, Finding, FindingCode, LinkReport, PageRecord,
    SitemapResult, UrlRow,
};
use crate::urls::{is_same_site, normalize_url};

pub const IMAGE_HEAVY_BYTES: i64 = 204_800;
pub const THIN_CONTENT_WORDS: u32 = 200;

fn norm(raw: &str) -> String {
    normalize_url(raw).unwrap_or_else(|| raw.to_string())
}

/// `ERROR_SKIPPED` (-6) : SiteOne n'a pas fait la requête (trop de réponses
/// non-200 pour ce nom de fichier), ce n'est pas une erreur de la page.
fn is_error(page: &PageRecord) -> bool {
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

pub fn run_checks(
    pages: &[PageRecord],
    links: &LinkReport,
    sitemap: &SitemapResult,
    start_url: &str,
) -> Vec<Finding> {
    let start = Url::parse(&norm(start_url)).ok();
    let is_internal = |raw: &str| match (&start, Url::parse(&norm(raw))) {
        (Some(s), Ok(t)) => is_same_site(s, &t),
        _ => false,
    };
    let by_url: BTreeMap<String, &PageRecord> = pages.iter().map(|p| (norm(&p.url), p)).collect();

    let mut out: Vec<Finding> = Vec::new();
    let mut push = |code: FindingCode, url: &str, related: Vec<String>| {
        out.push(Finding {
            code,
            url: url.to_string(),
            related,
        });
    };

    // Une page externe (atteinte par redirection) reste une ligne du tableau,
    // sans constat : seules les pages du site sont contrôlées.
    for page in pages {
        let facts = page.facts.as_ref();
        let internal = is_internal(&page.url);

        if internal && is_html_200(page) {
            if let Some(facts) = facts {
                canonical_finding(page, facts.canonical.as_deref(), &by_url, &is_internal)
                    .into_iter()
                    .for_each(|(code, related)| push(code, &page.url, related));
                if facts.json_ld.is_empty() {
                    push(FindingCode::JsonLdMissing, &page.url, vec![]);
                }
                if facts.word_count < THIN_CONTENT_WORDS {
                    push(FindingCode::ThinContent, &page.url, vec![]);
                }
            }
        }
        if internal && page.is_html() {
            if facts.is_some_and(|f| f.json_ld.iter().any(|b| !b.valid)) {
                push(FindingCode::JsonLdInvalid, &page.url, vec![]);
            }
            if is_noindex(page) {
                push(FindingCode::Noindex, &page.url, vec![]);
            }
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
            );
        }
    }

    for (first, hops) in links
        .redirect_chains
        .iter()
        .filter(|(first, _)| is_internal(first))
    {
        push(FindingCode::RedirectChain, first, hops.clone());
    }

    for (target, sources) in &links.inlinks {
        let Some(page) = by_url.get(target) else {
            continue;
        };
        if is_internal(target) && is_error(page) && !sources.is_empty() {
            let related: BTreeSet<String> = sources.iter().map(|s| s.from.clone()).collect();
            push(
                FindingCode::BrokenInternalLink,
                target,
                related.into_iter().collect(),
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
                push(FindingCode::OrphanPage, &page.url, vec![]);
            }
            if is_error(page) {
                push(FindingCode::SitemapUrlError, &page.url, vec![]);
            } else if is_html_200(page) && !is_indexable(page) {
                push(FindingCode::SitemapUrlNonIndexable, &page.url, vec![]);
            }
        }
        for page in pages {
            if is_internal(&page.url)
                && is_indexable(page)
                && !sitemap_urls.contains(&norm(&page.url))
            {
                push(FindingCode::NotInSitemap, &page.url, vec![]);
            }
        }
    } else {
        push(FindingCode::SitemapNotFound, start_url, vec![]);
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
            push(FindingCode::DuplicateContent, url, related);
        }
    }

    out.sort_by_cached_key(|f| (f.code.clone() as usize, f.url.clone()));
    out
}

fn canonical_finding(
    page: &PageRecord,
    canonical: Option<&str>,
    by_url: &BTreeMap<String, &PageRecord>,
    is_internal: &impl Fn(&str) -> bool,
) -> Option<(FindingCode, Vec<String>)> {
    let Some(canonical) = canonical else {
        return Some((FindingCode::CanonicalMissing, vec![]));
    };
    let target = norm(canonical);
    if target == norm(&page.url) {
        return None;
    }
    let code = match by_url.get(&target) {
        Some(t) if is_error(t) => FindingCode::CanonicalToError,
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

pub fn assemble(meta: CrawlMeta, data: CollectedData) -> CrawlResult {
    let links = analyze_links(&data.pages, &meta.start_url);
    let findings = run_checks(&data.pages, &links, &data.sitemap, &meta.start_url);
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
    use crate::model::{Image, JsonLd, Link, PageFacts};

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
        run_checks(pages, &links, sm, start)
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
        let f = run_checks(&pages, &links, &sitemap(&["/", "/orphan"]), START);
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
}
