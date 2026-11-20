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
        message: "Aucune page n'a pu être chargée. Vérifiez l'adresse ou la connexion."
            .to_string(),
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
    assert!(has_with_related(&r, FindingCode::BrokenInternalLink, "/missing", &["/"]));
    assert!(has(&r, FindingCode::ImageTooHeavy, "/img/big.jpg"));
    assert!(has(&r, FindingCode::JsonLdInvalid, "/b"));
    assert!(has(&r, FindingCode::TemporaryRedirect, "/temp"));
    assert!(has(&r, FindingCode::DuplicateContent, "/dup1"));
    assert!(has(&r, FindingCode::SitemapUrlError, "/gone"));
    assert_eq!(r.rows.len(), r.pages.len());
    assert_eq!(row(&r, "/b").depth, Some(1));
}
