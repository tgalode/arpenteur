mod support;

use std::time::{Duration, Instant};

use std::sync::Arc;

use arpenteur_core::collector::CrawlEvents;
use arpenteur_core::engine::run_crawl;
use arpenteur_core::model::{
    CrawlMeta, CrawlResult, CrawlSettings, CrawlStatus, FindingCode, Progress, UrlRow,
};
use arpenteur_core::sitemap::{discover, discover_with_limit};
use arpenteur_core::storage::CrawlStore;
use support::{Variant, start_fixture_site};
use tokio_util::sync::CancellationToken;
use url::Url;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Arpenteur-test")
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_sitemap_declared_in_robots() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 5);
    assert!(result.urls.contains(&format!("{}/orphan", site.base)));
    assert_eq!(result.sources, vec![format!("{}/sitemap.xml", site.base)]);
    assert!(!result.truncated);
    let robots = result.robots_txt.as_deref().unwrap();
    assert!(robots.contains(&format!("Sitemap: {}/sitemap.xml", site.base)));
}

#[tokio::test(flavor = "multi_thread")]
async fn falls_back_to_sitemap_xml() {
    let site = start_fixture_site(Variant::NoSitemapLine).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_sitemap_is_not_found() {
    let site = start_fixture_site(Variant::NoSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(!result.found);
    assert!(result.urls.is_empty());
    assert!(result.sources.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn follows_sitemap_index_and_caps() {
    let site = start_fixture_site(Variant::HugeSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 10_000);
    assert!(result.truncated);
    assert!(site.requested("/s1.xml"));
    assert!(!site.requested("/s2.xml"));
}

#[tokio::test(flavor = "multi_thread")]
async fn respects_min_interval() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let interval = Duration::from_millis(200);
    let began = Instant::now();
    let result = discover(&client(), &start, interval, 10_000).await;
    let elapsed = began.elapsed();
    assert!(result.found);
    let made = site
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|p| *p == "/robots.txt" || p.ends_with(".xml"))
        .count() as u32;
    assert!(made >= 2);
    assert!(
        elapsed >= interval * (made - 1),
        "{elapsed:?} pour {made} requêtes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn oversize_sitemap_records_error() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover_with_limit(&client(), &start, Duration::ZERO, 10_000, 120).await;
    assert!(result.urls.len() < 5);
    assert!(result.errors.iter().any(|e| e.contains("dépasse 120")));
}

#[tokio::test(flavor = "multi_thread")]
async fn truncated_sitemap_records_error() {
    let site = start_fixture_site(Variant::TruncatedSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(!result.found);
    assert!(result.errors.iter().any(|e| e.contains("tronqué")));
}

struct NoEvents;

impl CrawlEvents for NoEvents {
    fn progress(&self, _p: &Progress) {}
}

struct CancelOnFirstProgress(CancellationToken);

impl CrawlEvents for CancelOnFirstProgress {
    fn progress(&self, _p: &Progress) {
        self.0.cancel();
    }
}

fn fast_settings() -> CrawlSettings {
    CrawlSettings {
        workers: 2,
        max_reqs_per_sec: 50.0,
        ..CrawlSettings::default()
    }
}

type CrawlRun = (
    support::FixtureSite,
    CrawlStore,
    arpenteur_core::model::CrawlLocation,
    CrawlMeta,
    tempfile::TempDir,
);

async fn run(
    variant: Variant,
    events: impl FnOnce(&CancellationToken) -> Arc<dyn CrawlEvents>,
) -> CrawlRun {
    run_with(variant, fast_settings(), events).await
}

async fn run_with(
    variant: Variant,
    settings: CrawlSettings,
    events: impl FnOnce(&CancellationToken) -> Arc<dyn CrawlEvents>,
) -> CrawlRun {
    let site = start_fixture_site(variant).await;
    let root = tempfile::tempdir().unwrap();
    let store = CrawlStore::new(root.path().to_path_buf());
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let loc = store
        .create_crawl(start.host_str().unwrap(), chrono::Local::now())
        .unwrap();
    let cancel = CancellationToken::new();
    let events = events(&cancel);
    let meta = run_crawl(settings, start, store.clone(), loc.clone(), events, cancel).await;
    (site, store, loc, meta, root)
}

#[tokio::test(flavor = "multi_thread")]
async fn crawl_completes_and_writes_files() {
    let (site, store, loc, meta, _root) = run(Variant::Standard, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    for f in ["crawl.json", "arpenteur.json", "siteone.json"] {
        assert!(loc.dir.join(f).exists(), "{f}");
    }
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, CrawlStatus::Completed);
    assert_eq!(m.pages as usize, data.pages.len());
    for path in ["/", "/a", "/b", "/orphan", "/img/big.jpg"] {
        let url = format!("{}{}", site.base, path);
        assert!(data.pages.iter().any(|p| p.url == url), "{url}");
    }
    assert!(data.sitemap.found);
    assert!(!site.requested("/app.js") && !site.requested("/style.css"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sitemap_seeds_respect_robots_and_asset_filter() {
    let (site, store, loc, meta, _root) =
        run(Variant::RobotsDisallow, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(site.requested("/orphan"));
    assert!(!site.requested("/private/secret"));
    assert!(!site.requested("/doc.pdf"));
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    for path in ["/private/secret", "/doc.pdf", "/orphan"] {
        let url = format!("{}{}", site.base, path);
        assert!(data.sitemap.urls.contains(&url), "{url}");
    }
}

/// Vérifie que le site n'est plus sollicité une fois `run_crawl` revenu.
/// SiteOne lance une tâche par URL et l'annulation ne les interrompt pas : au plus
/// `workers - 1` requêtes déjà lancées arrivent encore après le retour.
/// Plafond du total, tiré du journal du site : les robots.txt et sitemap.xml
/// demandés (découverte et SiteOne), "/", `pages` pages au-delà de "/", plus `workers`.
async fn assert_site_left_alone(site: &support::FixtureSite, workers: u32, pages: usize) {
    let at_return = site.requests.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let log = site.requests.lock().unwrap().clone();
    let late = log.len() - at_return;
    assert!(
        late <= workers as usize - 1,
        "{late} requête(s) après la fin, workers={workers} : {log:?}"
    );
    let fixed = log
        .iter()
        .filter(|p| *p == "/robots.txt" || *p == "/sitemap.xml")
        .count();
    let bound = fixed + 1 + pages + workers as usize;
    assert!(
        log.len() <= bound,
        "{} requêtes pour un plafond de {bound}, workers={workers} : {log:?}",
        log.len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_keeps_collected_pages() {
    let settings = CrawlSettings {
        workers: 1,
        ..fast_settings()
    };
    let (site, store, loc, meta, _root) = run_with(Variant::Slow, settings, |cancel| {
        Arc::new(CancelOnFirstProgress(cancel.clone()))
    })
    .await;
    assert_eq!(meta.status, CrawlStatus::Stopped);
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, CrawlStatus::Stopped);
    assert!(!data.pages.is_empty());
    assert!(m.finished_at.is_some());
    // Arrêt au premier résultat, celui de "/" : aucune page de plus n'est due.
    assert_site_left_alone(&site, 1, 0).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn unreachable_site_fails() {
    // Port libre puis relâché : personne n'écoute.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let root = tempfile::tempdir().unwrap();
    let store = CrawlStore::new(root.path().to_path_buf());
    let start = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    let loc = store
        .create_crawl(start.host_str().unwrap(), chrono::Local::now())
        .unwrap();
    let meta = run_crawl(
        fast_settings(),
        start,
        store.clone(),
        loc.clone(),
        Arc::new(NoEvents),
        CancellationToken::new(),
    )
    .await;
    let expected = CrawlStatus::Failed {
        message: "Aucune page n'a pu être chargée. Vérifiez l'adresse ou la connexion.".to_string(),
    };
    assert_eq!(meta.status, expected);
    let (m, _) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, expected);
}

async fn assert_throttled_stops(workers: u32) {
    let settings = CrawlSettings {
        workers,
        ..fast_settings()
    };
    let threshold = settings.limit_threshold as usize;
    let (site, store, loc, meta, _root) =
        run_with(Variant::Throttled, settings, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Throttled, "workers={workers}");
    let (m, _) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, CrawlStatus::Throttled, "workers={workers}");
    assert!(threshold < support::LINKED_PAGES);
    // Seuil atteint après `threshold` pages en 429, sur LINKED_PAGES liées depuis "/".
    assert_site_left_alone(&site, workers, threshold).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn throttled_site_stops_crawl() {
    assert_throttled_stops(1).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn throttled_site_stops_crawl_with_two_workers() {
    assert_throttled_stops(2).await;
}

fn path_of(url: &str) -> String {
    Url::parse(url).unwrap().path().to_string()
}

fn has(r: &CrawlResult, code: FindingCode, path: &str) -> bool {
    r.findings
        .iter()
        .any(|f| f.code == code && path_of(&f.url) == path)
}

fn has_with_related(r: &CrawlResult, code: FindingCode, path: &str, related: &[&str]) -> bool {
    r.findings.iter().any(|f| {
        f.code == code
            && path_of(&f.url) == path
            && f.related.iter().map(|u| path_of(u)).collect::<Vec<_>>() == related
    })
}

fn row<'a>(r: &'a CrawlResult, path: &str) -> &'a UrlRow {
    r.rows.iter().find(|x| path_of(&x.url) == path).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn load_result_on_fixture_site() {
    let (_site, store, loc, _meta, _root) = run(Variant::Standard, |_| Arc::new(NoEvents)).await;
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::CanonicalToRedirect, "/b"));
    assert!(has(&r, FindingCode::OrphanPage, "/orphan"));
    assert!(has_with_related(
        &r,
        FindingCode::BrokenInternalLink,
        "/missing",
        &["/"]
    ));
    assert!(has(&r, FindingCode::ImageTooHeavy, "/img/big.jpg"));
    assert!(has(&r, FindingCode::JsonLdInvalid, "/b"));
    assert!(has(&r, FindingCode::TemporaryRedirect, "/temp"));
    assert!(has(&r, FindingCode::DuplicateContent, "/dup1"));
    assert!(has(&r, FindingCode::SitemapUrlError, "/gone"));
    assert_eq!(r.rows.len(), r.pages.len());
    assert_eq!(row(&r, "/b").depth, Some(1));
}

fn fixture_page(
    url: &str,
    status: i32,
    canonical: Option<&str>,
) -> arpenteur_core::model::PageRecord {
    use arpenteur_core::model::{PageFacts, PageRecord};
    PageRecord {
        url: url.to_string(),
        status,
        content_type: "html".to_string(),
        size: None,
        elapsed_ms: 0,
        headers: Default::default(),
        facts: Some(PageFacts {
            title: None,
            meta_description: None,
            h1: vec![],
            canonical: canonical.map(str::to_string),
            meta_robots: None,
            links: vec![],
            images: vec![],
            json_ld: vec![],
            word_count: 0,
            text_hash: String::new(),
            lang: None,
            headings: vec![],
            unlinked_phones: vec![],
        }),
    }
}

fn probe_client() -> reqwest::Client {
    arpenteur_core::canonical_probe::probe_client("Arpenteur-test").unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn unlinked_canonical_target_is_probed() {
    let (site, store, loc, meta, _root) =
        run(Variant::CanonicalOrphanTarget, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::CanonicalToRedirect, "/x"));
    assert_eq!(site.count("/canon-target"), 1);
    let target = r
        .pages
        .iter()
        .find(|p| path_of(&p.url) == "/canon-target")
        .unwrap();
    assert_eq!(target.status, 301);
    assert_eq!(target.content_type, "redirect");
    assert!(target.facts.is_none());
    assert_eq!(
        target.headers.get("location").map(String::as_str),
        Some("/x")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn probe_respects_budget_and_cancel() {
    let site = start_fixture_site(Variant::CanonicalOrphanTarget).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let pages = vec![fixture_page(
        &format!("{}/x", site.base),
        200,
        Some(&format!("{}/canon-target", site.base)),
    )];
    let live = CancellationToken::new();
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        0,
        Duration::ZERO,
        &live,
    )
    .await;
    assert!(added.is_empty());

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &cancelled,
    )
    .await;
    assert!(added.is_empty());
    assert_eq!(site.count("/canon-target"), 0);

    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &live,
    )
    .await;
    assert_eq!(added.len(), 1);
    assert_eq!(site.count("/canon-target"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn external_canonical_is_not_probed() {
    let site = start_fixture_site(Variant::CanonicalOrphanTarget).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    // `localhost` n'est pas le même site que `127.0.0.1`.
    let external = site.base.replace("127.0.0.1", "localhost");
    let pages = vec![fixture_page(
        &format!("{}/x", site.base),
        200,
        Some(&format!("{external}/canon-target")),
    )];
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert!(added.is_empty());
    assert_eq!(site.count("/canon-target"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn crawled_or_duplicate_canonical_targets_are_probed_once_or_never() {
    let site = start_fixture_site(Variant::CanonicalOrphanTarget).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let target = format!("{}/canon-target", site.base);
    let pages = vec![
        fixture_page(&format!("{}/x", site.base), 200, Some(&target)),
        fixture_page(
            &format!("{}/y", site.base),
            200,
            Some(&format!("{target}#frag")),
        ),
        fixture_page(
            &format!("{}/z", site.base),
            200,
            Some(&format!("{}/x", site.base)),
        ),
    ];
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(added.len(), 1);
    assert_eq!(site.count("/canon-target"), 1);
    assert_eq!(site.count("/x"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn brotli_observed_on_start_page() {
    let (_site, store, loc, meta, _root) =
        run(Variant::BrotliOnProbe, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(data.start_content_encoding.as_deref(), Some("br"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(
        !r.findings
            .iter()
            .any(|f| f.code == FindingCode::ModernCompressionMissing)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_brotli_is_reported_on_start_url() {
    let (site, store, loc, _meta, _root) = run(Variant::Standard, |_| Arc::new(NoEvents)).await;
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(data.start_content_encoding.as_deref(), Some(""));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    let found: Vec<_> = r
        .findings
        .iter()
        .filter(|f| f.code == FindingCode::ModernCompressionMissing)
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].url, format!("{}/", site.base));
}

#[tokio::test(flavor = "multi_thread")]
async fn content_encoding_probe_needs_a_200_and_a_live_token() {
    let site = start_fixture_site(Variant::BrotliOnProbe).await;
    let live = CancellationToken::new();
    let url = |p: &str| Url::parse(&format!("{}{p}", site.base)).unwrap();
    let probe = arpenteur_core::canonical_probe::probe_content_encoding;

    assert_eq!(
        probe(&probe_client(), &url("/"), &live).await.as_deref(),
        Some("br")
    );
    assert_eq!(
        probe(&probe_client(), &url("/a"), &live).await.as_deref(),
        Some("")
    );
    assert_eq!(probe(&probe_client(), &url("/old"), &live).await, None);
    assert_eq!(probe(&probe_client(), &url("/missing"), &live).await, None);

    let before = site.requests.lock().unwrap().len();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(probe(&probe_client(), &url("/"), &cancelled).await, None);
    assert_eq!(site.requests.lock().unwrap().len(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn post_crawl_probes_share_one_budget() {
    let site = start_fixture_site(Variant::CanonicalOrphanTarget).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let pages = vec![
        fixture_page(&format!("{}/", site.base), 200, None),
        fixture_page(
            &format!("{}/x", site.base),
            200,
            Some(&format!("{}/canon-target", site.base)),
        ),
    ];
    let run = |max: usize, cancel: CancellationToken| {
        let (pages, start) = (pages.clone(), start.clone());
        async move {
            arpenteur_core::canonical_probe::run_post_crawl_probes(
                &probe_client(),
                &pages,
                &start,
                max,
                Duration::ZERO,
                &cancel,
            )
            .await
        }
    };

    // Budget nul (limite déjà atteinte) ou jeton annulé : aucune requête.
    let r = run(2, CancellationToken::new()).await;
    assert!(r.content_encoding.is_none() && r.pages.is_empty());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let r = run(10, cancelled).await;
    assert!(r.content_encoding.is_none() && r.pages.is_empty());
    assert_eq!(site.requests.lock().unwrap().len(), 0);

    // Un seul slot : la compression passe d'abord, la cible n'est pas demandée.
    let r = run(3, CancellationToken::new()).await;
    assert_eq!(r.content_encoding.as_deref(), Some(""));
    assert!(r.pages.is_empty());
    assert_eq!(site.count("/"), 1);
    assert_eq!(site.count("/canon-target"), 0);

    let r = run(4, CancellationToken::new()).await;
    assert_eq!(r.pages.len(), 1);
    assert_eq!(site.count("/canon-target"), 1);
}

async fn sampled_encoding(
    site: &support::FixtureSite,
    pages: Vec<arpenteur_core::model::PageRecord>,
) -> Option<String> {
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    arpenteur_core::canonical_probe::run_post_crawl_probes(
        &probe_client(),
        &pages,
        &start,
        100,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await
    .content_encoding
}

#[tokio::test(flavor = "multi_thread")]
async fn compression_is_sampled_on_the_start_url_first() {
    let site = start_fixture_site(Variant::BrotliOnProbe).await;
    let at = |p: &str| format!("{}{p}", site.base);
    let pages = vec![
        fixture_page(&at("/a"), 200, None),
        fixture_page(&at("/"), 200, None),
    ];
    assert_eq!(sampled_encoding(&site, pages).await.as_deref(), Some("br"));
    assert_eq!(site.count("/"), 1);
    assert_eq!(site.count("/a"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn compression_falls_back_to_first_internal_html_page() {
    let site = start_fixture_site(Variant::BrotliOnProbe).await;
    let at = |p: &str| format!("{}{p}", site.base);
    let mut redirected_start = fixture_page(&at("/"), 301, None);
    redirected_start.content_type = "redirect".to_string();
    let pages = vec![
        fixture_page(&at("/b"), 404, None),
        redirected_start,
        fixture_page(&at("/a"), 200, None),
        fixture_page(&at("/dup1"), 200, None),
    ];
    assert_eq!(sampled_encoding(&site, pages).await.as_deref(), Some(""));
    assert_eq!(site.count("/a"), 1);
    assert_eq!(site.count("/"), 0);
    assert_eq!(site.count("/dup1"), 0);

    let pages = vec![fixture_page(&at("/dup1"), 200, None)];
    assert_eq!(sampled_encoding(&site, pages).await.as_deref(), Some(""));
    assert_eq!(site.count("/dup1"), 1);
}

fn short_timeout_client() -> reqwest::Client {
    arpenteur_core::canonical_probe::probe_client_with_timeout(
        "Arpenteur-test",
        Duration::from_millis(300),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn hanging_canonical_target_times_out_without_record() {
    let site = start_fixture_site(Variant::CanonicalTargetHangs).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let pages = vec![fixture_page(
        &format!("{}/x", site.base),
        200,
        Some(&format!("{}/canon-target", site.base)),
    )];
    let began = Instant::now();
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &short_timeout_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert!(began.elapsed() < Duration::from_secs(5));
    assert!(added.is_empty());
    assert_eq!(site.count("/canon-target"), 1);

    let r = arpenteur_core::checks::assemble(
        arpenteur_core::model::CrawlMeta {
            id: "i".into(),
            domain: "d".into(),
            start_url: start.to_string(),
            started_at: String::new(),
            finished_at: None,
            status: CrawlStatus::Completed,
            settings: fast_settings(),
            pages: 1,
        },
        arpenteur_core::model::CollectedData {
            pages: [pages, added].concat(),
            ..Default::default()
        },
        None,
    );
    assert!(
        !r.findings
            .iter()
            .any(|f| f.code == FindingCode::CanonicalToError)
    );
    assert!(
        r.findings
            .iter()
            .any(|f| f.code == FindingCode::CanonicalToOtherUrl)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn closed_port_canonical_target_leaves_no_record() {
    let site = start_fixture_site(Variant::CanonicalOrphanTarget).await;
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let pages = vec![fixture_page(
        &format!("{}/x", site.base),
        200,
        Some(&format!("http://127.0.0.1:{closed}/ailleurs")),
    )];
    let added = arpenteur_core::canonical_probe::probe_canonical_targets(
        &probe_client(),
        &pages,
        &start,
        10,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert!(added.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn compression_probe_is_none_on_timeout_and_network_error() {
    let site = start_fixture_site(Variant::CanonicalTargetHangs).await;
    let hanging = Url::parse(&format!("{}/canon-target", site.base)).unwrap();
    let probe = arpenteur_core::canonical_probe::probe_content_encoding;
    let live = CancellationToken::new();
    assert_eq!(probe(&short_timeout_client(), &hanging, &live).await, None);
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let refused = Url::parse(&format!("http://127.0.0.1:{closed}/")).unwrap();
    assert_eq!(probe(&probe_client(), &refused, &live).await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn compression_probe_waits_min_interval_and_is_cancel_aware() {
    let site = start_fixture_site(Variant::BrotliOnProbe).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let pages = vec![fixture_page(&format!("{}/", site.base), 200, None)];
    let run = |interval: Duration, cancel: CancellationToken| {
        let (pages, start) = (pages.clone(), start.clone());
        async move {
            arpenteur_core::canonical_probe::run_post_crawl_probes(
                &probe_client(),
                &pages,
                &start,
                100,
                interval,
                &cancel,
            )
            .await
        }
    };

    let began = Instant::now();
    let r = run(Duration::from_millis(400), CancellationToken::new()).await;
    assert!(began.elapsed() >= Duration::from_millis(400));
    assert_eq!(r.content_encoding.as_deref(), Some("br"));

    let before = site.requests.lock().unwrap().len();
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        canceller.cancel();
    });
    let began = Instant::now();
    let r = run(Duration::from_secs(30), cancel).await;
    assert!(began.elapsed() < Duration::from_secs(5));
    assert!(r.content_encoding.is_none());
    assert_eq!(site.requests.lock().unwrap().len(), before);
}
