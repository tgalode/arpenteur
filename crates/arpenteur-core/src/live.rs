//! Instantané d'un crawl en cours : lignes et constats des pages déjà collectées,
//! calculés par le même code qu'à la fin.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::checks::{CheckInput, build_rows, run_checks};
use crate::collector::CollectorHandle;
use crate::links::analyze_links;
use crate::model::{CrawlMeta, CrawlMode, Finding, FindingCode, PageRecord, SitemapResult, UrlRow};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveView {
    pub meta: CrawlMeta,
    pub rows: Vec<UrlRow>,
    pub findings: Vec<Finding>,
}

/// Ce que le moteur donne à lire pendant la collecte.
struct Collecting {
    handle: CollectorHandle,
    sitemap: Arc<SitemapResult>,
    meta: CrawlMeta,
}

/// Emplacement partagé et peu coûteux à cloner : le moteur le remplit quand la collecte
/// commence et le vide quand elle finit, l'application le lit.
#[derive(Clone, Default)]
pub struct LiveCrawl {
    slot: Arc<Mutex<Option<Collecting>>>,
}

/// Vide l'emplacement à la fin de la collecte, quelle qu'en soit la sortie.
pub(crate) struct LiveGuard(LiveCrawl);

impl LiveCrawl {
    pub fn new() -> LiveCrawl {
        LiveCrawl::default()
    }

    pub(crate) fn begin(
        &self,
        handle: CollectorHandle,
        sitemap: SitemapResult,
        meta: CrawlMeta,
    ) -> LiveGuard {
        *self.lock() = Some(Collecting {
            handle,
            sitemap: Arc::new(sitemap),
            meta,
        });
        LiveGuard(self.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Collecting>> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `None` tant qu'aucun crawl ne collecte, et dès que la collecte est finie.
    pub fn snapshot(&self) -> Option<LiveView> {
        let (handle, sitemap, meta) = {
            let slot = self.lock();
            let c = slot.as_ref()?;
            (c.handle.clone(), c.sitemap.clone(), c.meta.clone())
        };
        // Seule la copie des pages se fait sous le verrou du collecteur.
        Some(view_of(handle.pages(), &sitemap, meta))
    }
}

const GROUP_CODES: [FindingCode; 4] = [
    FindingCode::TitleDuplicate,
    FindingCode::MetaDescriptionDuplicate,
    FindingCode::H1Duplicate,
    FindingCode::DuplicateContent,
];

/// Lignes et constats des pages données, calculés hors de tout verrou.
fn view_of(pages: Vec<PageRecord>, sitemap: &SitemapResult, mut meta: CrawlMeta) -> LiveView {
    let links = analyze_links(&pages, &meta.start_url);
    let mut findings = run_checks(&CheckInput {
        list: (meta.mode == CrawlMode::List).then_some(&meta.list_urls[..]),
        partial: true,
        // Un 429 ou un 503 n'est jamais une erreur dans un instantané : si le crawl finit
        // limité, ces constats auraient disparu.
        throttled: true,
        ..CheckInput::new(&pages, &links, sitemap, &meta.start_url)
    });
    let rows = build_rows(&pages, &links, meta.mode, &meta.list_urls);
    // Chaque membre d'un groupe a son constat : la liste des autres ferait N x (N-1) adresses.
    for f in findings
        .iter_mut()
        .filter(|f| GROUP_CODES.contains(&f.code))
    {
        f.related.clear();
    }
    meta.pages = pages.len() as u32;
    LiveView {
        meta,
        rows,
        findings,
    }
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        *self.0.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::{CrawlSettings, CrawlStatus, FindingCode, JsonLd, Link, PageFacts};

    const BASE: &str = "https://x.fr";

    fn meta() -> CrawlMeta {
        CrawlMeta {
            id: "a".into(),
            domain: "x.fr".into(),
            start_url: format!("{BASE}/"),
            started_at: String::new(),
            finished_at: None,
            status: CrawlStatus::Running,
            settings: CrawlSettings::default(),
            pages: 0,
            mode: CrawlMode::Site,
            list_urls: vec![],
        }
    }

    fn page(path: &str, status: i32, links: &[&str], canonical: Option<&str>) -> PageRecord {
        let url = format!("{BASE}{path}");
        let html = status == 200;
        PageRecord {
            url: url.clone(),
            status,
            content_type: if html { "html" } else { "other" }.into(),
            size: None,
            elapsed_ms: 0,
            headers: BTreeMap::new(),
            facts: html.then(|| PageFacts {
                title: Some("t".into()),
                meta_description: None,
                h1: vec!["h".into()],
                canonical: canonical.map(|c| format!("{BASE}{c}")),
                canonicals: vec![],
                meta_robots: None,
                links: links
                    .iter()
                    .map(|l| Link {
                        href: format!("{BASE}{l}"),
                        anchor: String::new(),
                        nofollow: false,
                    })
                    .collect(),
                images: vec![],
                json_ld: vec![JsonLd {
                    valid: true,
                    types: vec![],
                }],
                word_count: 300,
                text_hash: format!("hash-{url}"),
                lang: None,
                headings: vec![],
                unlinked_phones: vec![],
            }),
            probed: false,
        }
    }

    fn sitemap(paths: &[&str]) -> SitemapResult {
        SitemapResult {
            found: true,
            urls: paths.iter().map(|p| format!("{BASE}{p}")).collect(),
            ..SitemapResult::default()
        }
    }

    fn urls_of(view: &LiveView, code: FindingCode) -> Vec<String> {
        view.findings
            .iter()
            .filter(|f| f.code == code)
            .map(|f| f.url.clone())
            .collect()
    }

    const ERROR_CODES: [FindingCode; 4] = [
        FindingCode::ServerError,
        FindingCode::BrokenInternalLink,
        FindingCode::SitemapUrlError,
        FindingCode::CanonicalToError,
    ];

    #[test]
    fn snapshot_never_counts_429_and_503_as_errors() {
        let pages = vec![
            page("/", 200, &["/e503", "/e429", "/c"], None),
            page("/e503", 503, &[], None),
            page("/e429", 429, &[], None),
            page("/c", 200, &[], Some("/e503")),
        ];
        let sm = sitemap(&["/", "/e503", "/e429", "/c"]);
        let view = view_of(pages, &sm, meta());
        for code in ERROR_CODES {
            assert!(
                urls_of(&view, code.clone()).is_empty(),
                "{code:?} {:?}",
                view.findings
            );
        }
        // Pas de constat de canonical du tout : il ne doit pas être renommé à la fin.
        for code in [
            FindingCode::CanonicalToOtherUrl,
            FindingCode::CanonicalToRedirect,
        ] {
            assert!(urls_of(&view, code.clone()).is_empty(), "{code:?}");
        }
        assert!(
            !view.findings.iter().any(|f| f.url == format!("{BASE}/c")
                && format!("{:?}", f.code).starts_with("Canonical")),
            "{:?}",
            view.findings
        );
    }

    #[test]
    fn final_result_names_the_canonical_to_a_503_page_an_error() {
        let pages = vec![
            page("/", 200, &["/e503", "/c"], None),
            page("/e503", 503, &[], None),
            page("/c", 200, &[], Some("/e503")),
        ];
        let sm = sitemap(&["/"]);
        let links = analyze_links(&pages, &format!("{BASE}/"));
        let findings = run_checks(&CheckInput::new(&pages, &links, &sm, &format!("{BASE}/")));
        assert!(
            findings
                .iter()
                .any(|f| f.code == FindingCode::CanonicalToError && f.url == format!("{BASE}/c")),
            "{findings:?}"
        );
    }

    fn same_head_pages(n: usize) -> Vec<PageRecord> {
        let mut pages = vec![page("/", 200, &[], None)];
        for i in 0..n {
            pages.push(page(&format!("/p{i}"), 200, &[], None));
        }
        pages
    }

    #[test]
    fn snapshot_empties_related_of_group_findings() {
        let mut pages = same_head_pages(2);
        for p in &mut pages {
            let f = p.facts.as_mut().unwrap();
            f.title = Some("Même titre".into());
            f.meta_description = Some("Même description".into());
            f.h1 = vec!["Même H1".into()];
            f.text_hash = "meme".into();
        }
        let view = view_of(pages, &sitemap(&["/"]), meta());
        for code in [
            FindingCode::TitleDuplicate,
            FindingCode::MetaDescriptionDuplicate,
            FindingCode::H1Duplicate,
            FindingCode::DuplicateContent,
        ] {
            let found: Vec<_> = view.findings.iter().filter(|f| f.code == code).collect();
            assert_eq!(found.len(), 3, "{code:?}");
            assert!(found.iter().all(|f| f.related.is_empty()), "{code:?}");
        }
    }

    #[test]
    fn final_result_keeps_related_of_group_findings() {
        let mut pages = same_head_pages(2);
        for p in &mut pages {
            p.facts.as_mut().unwrap().title = Some("Même titre".into());
        }
        let sm = sitemap(&["/"]);
        let links = analyze_links(&pages, &format!("{BASE}/"));
        let findings = run_checks(&CheckInput::new(&pages, &links, &sm, &format!("{BASE}/")));
        let dup: Vec<_> = findings
            .iter()
            .filter(|f| f.code == FindingCode::TitleDuplicate)
            .collect();
        assert_eq!(dup.len(), 3);
        assert!(dup.iter().all(|f| f.related.len() == 2), "{dup:?}");
    }

    #[test]
    fn snapshot_keeps_related_of_non_group_findings() {
        let pages = vec![
            page("/", 200, &["/c", "/b"], None),
            page("/b", 200, &[], None),
            page("/c", 200, &[], Some("/b")),
        ];
        let view = view_of(pages, &sitemap(&["/"]), meta());
        let f = view
            .findings
            .iter()
            .find(|f| f.code == FindingCode::CanonicalToOtherUrl && f.url == format!("{BASE}/c"))
            .expect("canonical vers une cible connue");
        assert_eq!(f.related, vec![format!("{BASE}/b")]);
    }

    #[test]
    fn snapshot_still_reports_500_and_404() {
        let pages = vec![
            page("/", 200, &["/e500", "/e404", "/c"], None),
            page("/e500", 500, &[], None),
            page("/e404", 404, &[], None),
            page("/c", 200, &[], Some("/e404")),
        ];
        let sm = sitemap(&["/", "/e500", "/e404"]);
        let view = view_of(pages, &sm, meta());
        assert_eq!(
            urls_of(&view, FindingCode::ServerError),
            vec![format!("{BASE}/e500")]
        );
        assert!(!urls_of(&view, FindingCode::BrokenInternalLink).is_empty());
        assert_eq!(
            urls_of(&view, FindingCode::CanonicalToError),
            vec![format!("{BASE}/c")]
        );
        assert_eq!(
            urls_of(&view, FindingCode::SitemapUrlError),
            vec![format!("{BASE}/e404"), format!("{BASE}/e500")]
        );
    }

    #[test]
    fn snapshot_before_the_first_page_is_empty_without_findings() {
        let view = view_of(vec![], &sitemap(&["/"]), meta());
        assert!(view.rows.is_empty());
        assert_eq!(view.meta.pages, 0);
        assert!(
            view.findings
                .iter()
                .all(|f| f.url == format!("{BASE}/") && f.related.is_empty()),
            "{:?}",
            view.findings
        );
    }
}
