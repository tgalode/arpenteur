mod support;

use std::time::{Duration, Instant};

use std::sync::Arc;

use arpenteur_core::canonical_probe::{
    PostCrawl, ProbeEnv, probe_canonical_targets, run_post_crawl_probes,
};
use arpenteur_core::collector::CrawlEvents;
use arpenteur_core::engine::{CrawlTarget, run_crawl};
use arpenteur_core::external_links::{
    MAX_REDIRECTS, check_external_links, classify_status, external_client_with_timeout,
    external_targets,
};
use arpenteur_core::live::LiveCrawl;
use arpenteur_core::model::{
    CrawlMeta, CrawlMode, CrawlResult, CrawlSettings, CrawlStatus, ExternalOutcome, FindingCode,
    Indexability, Progress, UrlRow,
};
use arpenteur_core::settings::{SkipReason, UrlFilter};
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

/// Garde les progressions reçues ; annule au `cancel_at`-ième lien externe vérifié.
struct ExternalWatcher {
    seen: std::sync::Mutex<Vec<Progress>>,
    cancel: CancellationToken,
    cancel_at: Option<u32>,
}

impl ExternalWatcher {
    fn new(cancel: &CancellationToken, cancel_at: Option<u32>) -> Arc<ExternalWatcher> {
        Arc::new(ExternalWatcher {
            seen: Default::default(),
            cancel: cancel.clone(),
            cancel_at,
        })
    }

    fn external(&self) -> Vec<(u32, u32)> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter_map(|p| p.external.map(|e| (e.done, e.total)))
            .collect()
    }
}

impl CrawlEvents for ExternalWatcher {
    fn progress(&self, p: &Progress) {
        self.seen.lock().unwrap().push(p.clone());
        if p.external.is_some_and(|e| Some(e.done) == self.cancel_at) {
            self.cancel.cancel();
        }
    }
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
    run_with_live(variant, settings, LiveCrawl::new(), events).await
}

async fn run_with_live(
    variant: Variant,
    settings: CrawlSettings,
    live: LiveCrawl,
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
    let meta = run_crawl(
        settings,
        CrawlTarget::Site(start),
        store.clone(),
        loc.clone(),
        events,
        cancel,
        live,
    )
    .await;
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
        CrawlTarget::Site(start),
        store.clone(),
        loc.clone(),
        Arc::new(NoEvents),
        CancellationToken::new(),
        LiveCrawl::new(),
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
        probed: false,
        facts: Some(PageFacts {
            title: None,
            meta_description: None,
            h1: vec![],
            canonical: canonical.map(str::to_string),
            canonicals: vec![],
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

async fn probe_targets_with(
    client: &reqwest::Client,
    start: &Url,
    pages: &[arpenteur_core::model::PageRecord],
    budget: usize,
    cancel: &CancellationToken,
) -> Vec<arpenteur_core::model::PageRecord> {
    let filter = UrlFilter::new(&CrawlSettings::default(), None);
    let env = ProbeEnv {
        client,
        start,
        filter: &filter,
        min_interval: Duration::ZERO,
        cancel,
        on_external_progress: &|_, _| {},
    };
    probe_canonical_targets(&env, pages, budget).await.0
}

fn follow_client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    arpenteur_core::external_links::external_client("Arpenteur-test").unwrap()
}

async fn post_probes(
    client: &reqwest::Client,
    start: &Url,
    pages: &[arpenteur_core::model::PageRecord],
    max_requests: usize,
    min_interval: Duration,
    cancel: &CancellationToken,
) -> PostCrawl {
    post_probes_with(
        client,
        Some(&follow_client()),
        false,
        None,
        start,
        pages,
        max_requests,
        min_interval,
        cancel,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn post_probes_with(
    client: &reqwest::Client,
    follow: Option<&reqwest::Client>,
    check_external: bool,
    robots_txt: Option<&str>,
    start: &Url,
    pages: &[arpenteur_core::model::PageRecord],
    max_requests: usize,
    min_interval: Duration,
    cancel: &CancellationToken,
) -> PostCrawl {
    let filter = UrlFilter::new(&CrawlSettings::default(), robots_txt);
    let env = ProbeEnv {
        client,
        start,
        filter: &filter,
        min_interval,
        cancel,
        on_external_progress: &|_, _| {},
    };
    run_post_crawl_probes(&env, pages, max_requests, follow, check_external).await
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
    let added = probe_targets_with(&probe_client(), &start, &pages, 0, &live).await;
    assert!(added.is_empty());

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let added = probe_targets_with(&probe_client(), &start, &pages, 10, &cancelled).await;
    assert!(added.is_empty());
    assert_eq!(site.count("/canon-target"), 0);

    let added = probe_targets_with(&probe_client(), &start, &pages, 10, &live).await;
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
    let added = probe_targets_with(
        &probe_client(),
        &start,
        &pages,
        10,
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
    let added = probe_targets_with(
        &probe_client(),
        &start,
        &pages,
        10,
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
            post_probes(
                &probe_client(),
                &start,
                &pages,
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
    assert!(r.pages.is_empty() && r.not_found.is_none());
    assert_eq!(site.count("/"), 1);
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 0);
    assert_eq!(site.count("/canon-target"), 0);

    // Deux slots : l'adresse inexistante passe avant la cible de canonical.
    let r = run(4, CancellationToken::new()).await;
    assert!(r.not_found.is_some() && r.pages.is_empty());
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 1);
    assert_eq!(site.count("/canon-target"), 0);

    let r = run(5, CancellationToken::new()).await;
    assert_eq!(r.pages.len(), 1);
    assert_eq!(site.count("/canon-target"), 1);
}

async fn sampled_encoding(
    site: &support::FixtureSite,
    pages: Vec<arpenteur_core::model::PageRecord>,
) -> Option<String> {
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    post_probes(
        &probe_client(),
        &start,
        &pages,
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
    let added = probe_targets_with(
        &short_timeout_client(),
        &start,
        &pages,
        10,
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
            mode: CrawlMode::Site,
            list_urls: Vec::new(),
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
    let added = probe_targets_with(
        &probe_client(),
        &start,
        &pages,
        10,
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
        async move { post_probes(&probe_client(), &start, &pages, 100, interval, &cancel).await }
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

#[tokio::test(flavor = "multi_thread")]
async fn disallowed_canonical_target_is_not_probed() {
    let (site, store, loc, meta, _root) =
        run(Variant::CanonicalToDisallowed, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(!site.requested("/prive/cible"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::CanonicalToOtherUrl, "/x"));
    assert!(r.pages.iter().all(|p| !p.probed));
}

#[tokio::test(flavor = "multi_thread")]
async fn pattern_excluded_canonical_target_is_not_probed() {
    let settings = CrawlSettings {
        ignore_regex: vec!["/tag/".to_string()],
        ..fast_settings()
    };
    let (site, store, loc, _meta, _root) =
        run_with(Variant::CanonicalToTag, settings, |_| Arc::new(NoEvents)).await;
    assert!(!site.requested("/tag/cible"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::CanonicalToOtherUrl, "/x"));

    // Sans le motif, la même cible est sondée.
    let (site, _store, _loc, _meta, _root) =
        run(Variant::CanonicalToTag, |_| Arc::new(NoEvents)).await;
    assert_eq!(site.count("/tag/cible"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn probed_row_is_marked_and_typed() {
    let (site, store, loc, _meta, _root) =
        run(Variant::CanonicalToPdf, |_| Arc::new(NoEvents)).await;
    assert_eq!(site.count("/canon-target"), 1);
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    let target = r
        .pages
        .iter()
        .find(|p| path_of(&p.url) == "/canon-target")
        .unwrap();
    assert!(target.probed);
    assert_eq!(target.status, 200);
    assert_eq!(target.content_type, "document");
    assert_eq!(
        row(&r, "/canon-target").indexable,
        Indexability::NotAnalyzed
    );
    assert_eq!(row(&r, "/x").indexable, Indexability::No);
    assert_eq!(row(&r, "/").indexable, Indexability::Yes);
    assert!(
        !r.findings
            .iter()
            .any(|f| f.code == FindingCode::SitemapUrlNonIndexable)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn probed_rows_are_not_counted_in_meta_pages() {
    let (_site, store, loc, meta, _root) =
        run(Variant::CanonicalToPdf, |_| Arc::new(NoEvents)).await;
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    let probed = data.pages.iter().filter(|p| p.probed).count();
    assert_eq!(probed, 1);
    assert_eq!(meta.pages as usize, data.pages.len() - probed);
    assert_eq!(m.pages, meta.pages);
}

// Liens externes

fn page_linking(url: &str, hrefs: &[&str]) -> arpenteur_core::model::PageRecord {
    let mut page = fixture_page(url, 200, None);
    page.facts.as_mut().unwrap().links = hrefs
        .iter()
        .map(|h| arpenteur_core::model::Link {
            href: h.to_string(),
            anchor: String::new(),
            nofollow: false,
        })
        .collect();
    page
}

fn ext_client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    external_client_with_timeout("Arpenteur-test", Duration::from_millis(500)).unwrap()
}

fn ext_url(site: &support::FixtureSite, path: &str) -> Url {
    let port = site.base.rsplit(':').next().unwrap();
    Url::parse(&format!("http://localhost:{port}{path}")).unwrap()
}

async fn check(
    targets: &[Url],
    budget: usize,
    interval: Duration,
    cancel: &CancellationToken,
) -> (Vec<arpenteur_core::model::ExternalCheck>, usize) {
    check_external_links(&ext_client(), targets, budget, interval, cancel, &|_, _| {}).await
}

fn outcome_of(
    checks: &[arpenteur_core::model::ExternalCheck],
    url: &Url,
) -> arpenteur_core::model::ExternalCheck {
    checks
        .iter()
        .find(|c| c.url == url.as_str())
        .unwrap_or_else(|| panic!("pas de vérification pour {url}"))
        .clone()
}

#[test]
fn classify_status_boundaries() {
    for s in [200, 301, 399] {
        assert_eq!(classify_status(s), ExternalOutcome::Ok, "{s}");
    }
    for s in [404, 410] {
        assert_eq!(classify_status(s), ExternalOutcome::Broken, "{s}");
    }
    for s in [400, 401, 403, 429, 500, 503, 999] {
        assert_eq!(classify_status(s), ExternalOutcome::Unknown, "{s}");
    }
    assert_eq!(MAX_REDIRECTS, 5);
}

#[test]
fn external_targets_are_distinct_http_links_off_site() {
    let start = Url::parse("https://exemple.fr/").unwrap();
    let pages = vec![
        page_linking(
            "https://exemple.fr/",
            &[
                "https://autre.fr/a#haut",
                "https://autre.fr/a",
                "mailto:contact@exemple.fr",
                "tel:+33123456789",
                "https://exemple.fr/interne",
                "https://www.exemple.fr/www",
                "http://tiers.fr/b",
                "ftp://tiers.fr/c",
            ],
        ),
        page_linking(
            "https://exemple.fr/p2",
            &["https://autre.fr/a", "https://z.fr/"],
        ),
        // Une page d'un autre site ne contribue pas : constats sur les pages internes seulement.
        page_linking("https://etranger.fr/", &["https://jamais.fr/"]),
    ];
    let targets: Vec<String> = external_targets(&pages, &start)
        .iter()
        .map(Url::to_string)
        .collect();
    assert_eq!(
        targets,
        ["https://autre.fr/a", "http://tiers.fr/b", "https://z.fr/"]
    );
}

#[test]
fn links_with_credentials_are_not_targets() {
    let start = Url::parse("https://exemple.fr/").unwrap();
    let pages = vec![page_linking(
        "https://exemple.fr/",
        &[
            "https://user:pass@autre.fr/x",
            "https://user@autre.fr/x",
            "https://:pass@autre.fr/x",
            "https://autre.fr/x",
        ],
    )];
    let targets: Vec<String> = external_targets(&pages, &start)
        .iter()
        .map(Url::to_string)
        .collect();
    assert_eq!(targets, ["https://autre.fr/x"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn broken_only_on_404_410() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let u = |p: &str| ext_url(&site, p);
    let all = [
        "/ext-ok",
        "/ext-404",
        "/ext-410",
        "/ext-403",
        "/ext-429",
        "/ext-999",
        "/ext-500",
        "/ext-redirect",
        "/ext-loop",
        "/ext-hang",
    ]
    .map(&u);
    let (checks, used) = check(&all, 100, Duration::ZERO, &CancellationToken::new()).await;
    assert_eq!(checks.len(), all.len());
    assert_eq!(used, all.len());

    let ok = outcome_of(&checks, &u("/ext-ok"));
    assert_eq!(
        (ok.outcome, ok.status, ok.final_url),
        (ExternalOutcome::Ok, Some(200), None)
    );
    for (path, status) in [("/ext-404", 404), ("/ext-410", 410)] {
        let c = outcome_of(&checks, &u(path));
        assert_eq!(
            (c.outcome, c.status),
            (ExternalOutcome::Broken, Some(status)),
            "{path}"
        );
    }
    let redirected = outcome_of(&checks, &u("/ext-redirect"));
    assert_eq!(redirected.outcome, ExternalOutcome::Broken);
    assert_eq!(redirected.status, Some(404));
    assert_eq!(redirected.final_url, Some(u("/ext-404").to_string()));

    for (path, status) in [
        ("/ext-403", Some(403)),
        ("/ext-429", Some(429)),
        ("/ext-999", Some(999)),
        ("/ext-500", Some(500)),
        ("/ext-loop", None),
        ("/ext-hang", None),
    ] {
        let c = outcome_of(&checks, &u(path));
        assert_eq!(
            (c.outcome, c.status),
            (ExternalOutcome::Unknown, status),
            "{path}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn each_external_url_is_requested_once() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let start = Url::parse("https://exemple.fr/").unwrap();
    let link = |p: &str| ext_url(&site, p).to_string();
    let pages: Vec<_> = (0..50)
        .map(|i| {
            page_linking(
                &format!("https://exemple.fr/p{i}"),
                &[&link("/ext-404"), &link("/ext-ok")],
            )
        })
        .collect();
    let targets = external_targets(&pages, &start);
    assert_eq!(targets.len(), 2);
    let (checks, used) = check(&targets, 100, Duration::ZERO, &CancellationToken::new()).await;
    assert_eq!((checks.len(), used), (2, 2));
    assert_eq!(site.count("/ext-404"), 1);
    assert_eq!(site.count("/ext-ok"), 1);
    assert_eq!(site.count("/robots.txt"), 0);
}

fn gone(name: &str) -> Url {
    Url::parse(&format!("https://{name}.invalid/")).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn vanished_domain_is_unknown_when_nothing_else_answered() {
    let targets = [gone("domaine-inexistant"), gone("autre-domaine")];
    let (checks, used) = check(&targets, 10, Duration::ZERO, &CancellationToken::new()).await;
    assert_eq!((checks.len(), used), (2, 4));
    for c in &checks {
        assert_eq!(
            (c.outcome, c.status, c.final_url.clone()),
            (ExternalOutcome::Unknown, None, None),
            "{}",
            c.url
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn vanished_domain_is_broken_when_another_target_answered() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let dead = gone("domaine-inexistant");
    let answering = ext_url(&site, "/ext-404");
    // Ni l'ordre ni le statut de la cible qui répond ne comptent.
    for targets in [
        vec![dead.clone(), answering.clone()],
        vec![answering.clone(), dead.clone()],
        vec![dead.clone(), ext_url(&site, "/ext-403")],
    ] {
        let (checks, used) = check(&targets, 10, Duration::ZERO, &CancellationToken::new()).await;
        assert_eq!((checks.len(), used), (2, 3));
        let c = outcome_of(&checks, &dead);
        assert_eq!(
            (c.outcome, c.status, c.final_url),
            (ExternalOutcome::Broken, None, None)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn single_dns_failure_is_never_broken() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let dead = gone("domaine-inexistant");
    // Budget de 2 : un seul essai pour la cible morte, même si l'autre répond.
    let targets = [ext_url(&site, "/ext-ok"), dead.clone()];
    let (checks, used) = check(&targets, 2, Duration::ZERO, &CancellationToken::new()).await;
    assert_eq!(used, 2);
    assert_eq!(outcome_of(&checks, &dead).outcome, ExternalOutcome::Unknown);
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_connection_is_unknown() {
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let url = Url::parse(&format!("http://localhost:{closed}/")).unwrap();
    let (checks, used) = check(
        std::slice::from_ref(&url),
        10,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(used, 1);
    let c = outcome_of(&checks, &url);
    assert_eq!((c.outcome, c.status), (ExternalOutcome::Unknown, None));
}

#[tokio::test(flavor = "multi_thread")]
async fn external_checks_share_the_budget() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let targets: Vec<Url> = ["/ext-ok", "/ext-404", "/ext-410", "/ext-403", "/ext-429"]
        .iter()
        .map(|p| ext_url(&site, p))
        .collect();
    let (checks, used) = check(&targets, 3, Duration::ZERO, &CancellationToken::new()).await;
    assert_eq!((checks.len(), used), (3, 3));
    assert_eq!(targets.len() - checks.len(), 2);
    assert_eq!(site.requests.lock().unwrap().len(), 3);

    let before = site.requests.lock().unwrap().len();
    let (checks, used) = check(&targets, 0, Duration::ZERO, &CancellationToken::new()).await;
    assert!(checks.is_empty());
    assert_eq!(used, 0);
    assert_eq!(site.requests.lock().unwrap().len(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_stops_external_checks() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let targets: Vec<Url> = ["/ext-ok", "/ext-404", "/ext-410"]
        .iter()
        .map(|p| ext_url(&site, p))
        .collect();

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let began = Instant::now();
    let (checks, used) = check(&targets, 10, Duration::from_secs(30), &cancelled).await;
    assert!(began.elapsed() < Duration::from_secs(5));
    assert!((checks.len(), used) == (0, 0));
    assert_eq!(site.requests.lock().unwrap().len(), 0);

    // Annulé pendant l'attente du débit : la première vérification est gardée.
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        canceller.cancel();
    });
    let began = Instant::now();
    let (checks, used) = check(&targets, 10, Duration::from_millis(400), &cancel).await;
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!((checks.len(), used), (1, 1));
    assert_eq!(checks[0].outcome, ExternalOutcome::Ok);
}

#[tokio::test(flavor = "multi_thread")]
async fn external_checks_wait_min_interval() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let targets: Vec<Url> = ["/ext-ok", "/ext-404"]
        .iter()
        .map(|p| ext_url(&site, p))
        .collect();
    let began = Instant::now();
    let (checks, _) = check(
        &targets,
        10,
        Duration::from_millis(300),
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(checks.len(), 2);
    assert!(began.elapsed() >= Duration::from_millis(600));
}

async fn crawl_external(settings: CrawlSettings, stop_early: bool) -> CrawlRun {
    run_with(Variant::ExternalLinks, settings, |cancel| {
        if stop_early {
            Arc::new(CancelOnFirstProgress(cancel.clone()))
        } else {
            Arc::new(NoEvents)
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn completed_crawl_checks_external_links_once_each() {
    let (site, store, loc, meta, _root) = crawl_external(fast_settings(), false).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(data.external_links.len(), 10);
    assert_eq!(data.external_unchecked, 0);
    assert_eq!(site.count("/ext-ok"), 1);
    // /ext-404 est lié sur deux pages et atteint aussi par /ext-redirect : 2 requêtes, pas 3.
    assert_eq!(site.count("/ext-404"), 2);
    assert!(
        data.pages
            .iter()
            .all(|p| !path_of(&p.url).starts_with("/ext-"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn crawl_budget_leaves_links_unchecked() {
    // 2 pages crawlées + 1 compression + 1 adresse inexistante + 0 canonical :
    // 4 requêtes restent pour 10 liens.
    let settings = CrawlSettings {
        max_requests: 8,
        ..fast_settings()
    };
    let (site, store, loc, meta, _root) = crawl_external(settings, false).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(data.external_links.len(), 4);
    assert_eq!(data.external_unchecked, 6);
    assert_eq!(site.count("/ext-hang"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn external_checks_report_done_and_total() {
    let watcher = std::sync::Mutex::new(None);
    let (_site, _store, _loc, meta, _root) =
        run_with(Variant::ExternalLinks, fast_settings(), |cancel| {
            let w = ExternalWatcher::new(cancel, None);
            *watcher.lock().unwrap() = Some(w.clone());
            w
        })
        .await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let w = watcher.lock().unwrap().take().unwrap();
    let expected: Vec<(u32, u32)> = (0..=10).map(|d| (d, 10)).collect();
    assert_eq!(w.external(), expected);
    // Les progressions du crawl lui-même n'ont pas ce champ.
    let seen = w.seen.lock().unwrap();
    assert!(seen.iter().take(2).all(|p| p.external.is_none()));
    assert!(seen.iter().all(|p| p.external.is_none() || p.done == 2));
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_during_external_checks_is_a_stopped_crawl() {
    let watcher = std::sync::Mutex::new(None);
    let (site, store, loc, meta, _root) =
        run_with(Variant::ExternalLinks, fast_settings(), |cancel| {
            let w = ExternalWatcher::new(cancel, Some(3));
            *watcher.lock().unwrap() = Some(w.clone());
            w
        })
        .await;
    assert_eq!(meta.status, CrawlStatus::Stopped);
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, CrawlStatus::Stopped);
    assert_eq!(m.pages, 2);
    assert_eq!(data.external_links.len(), 3);
    assert_eq!(data.external_unchecked, 0);
    assert!(!site.requested("/ext-hang"));
}

#[tokio::test(flavor = "multi_thread")]
async fn setting_off_makes_no_external_request() {
    let settings = CrawlSettings {
        check_external_links: false,
        ..fast_settings()
    };
    let (site, store, loc, meta, _root) = crawl_external(settings, false).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert!(data.external_links.is_empty());
    assert_eq!(data.external_unchecked, 0);
    assert!(
        !site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.starts_with("/ext-"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stopped_crawl_checks_no_external_link() {
    let settings = CrawlSettings {
        workers: 1,
        ..fast_settings()
    };
    let (site, store, loc, meta, _root) = crawl_external(settings, true).await;
    assert_eq!(meta.status, CrawlStatus::Stopped);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert!(data.external_links.is_empty());
    assert_eq!(data.external_unchecked, 0);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.starts_with("/ext-"))
    );
}

async fn run_list(variant: Variant, settings: CrawlSettings, paths: &[&str]) -> CrawlRun {
    let site = start_fixture_site(variant).await;
    let root = tempfile::tempdir().unwrap();
    let store = CrawlStore::new(root.path().to_path_buf());
    let urls: Vec<Url> = paths
        .iter()
        .map(|p| Url::parse(&format!("{}{}", site.base, p)).unwrap())
        .collect();
    let loc = store
        .create_crawl(urls[0].host_str().unwrap(), chrono::Local::now())
        .unwrap();
    let meta = run_crawl(
        settings,
        CrawlTarget::List(urls),
        store.clone(),
        loc.clone(),
        Arc::new(NoEvents),
        CancellationToken::new(),
        LiveCrawl::new(),
    )
    .await;
    (site, store, loc, meta, root)
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_requests_only_listed_urls() {
    let (site, store, loc, meta, _root) =
        run_list(Variant::Standard, fast_settings(), &["/", "/a"]).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.mode, CrawlMode::List);
    assert_eq!(meta.start_url, format!("{}/", site.base));
    assert_eq!(
        meta.list_urls,
        [format!("{}/", site.base), format!("{}/a", site.base)]
    );
    assert_eq!(meta.pages, 2);

    // "/" lie /a, /b, /old, /temp, /missing : aucun n'est suivi, sauf /a qui est listé.
    for path in ["/b", "/old", "/temp", "/missing", "/dup1", "/orphan"] {
        assert!(!site.requested(path), "{path}");
    }
    assert!(!site.requested("/sitemap.xml"));
    assert!(!site.requested("/app.js") && !site.requested("/style.css"));
    assert_eq!(site.count("/a"), 1);

    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    let paths: Vec<String> = r.rows.iter().map(|x| path_of(&x.url)).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"/".to_string()) && paths.contains(&"/a".to_string()));
    for x in &r.rows {
        assert_eq!((x.depth, x.inlinks, x.off_list), (None, None, false));
    }
    assert!(r.list_skipped.is_empty());
    // Les liens de la page sont relevés quand même.
    assert!(
        r.pages
            .iter()
            .find(|p| path_of(&p.url) == "/")
            .and_then(|p| p.facts.as_ref())
            .is_some_and(|f| f.links.len() == 5)
    );
    for code in [
        FindingCode::OrphanPage,
        FindingCode::NotInSitemap,
        FindingCode::SitemapNotFound,
        FindingCode::SitemapUrlError,
        FindingCode::BrokenInternalLink,
    ] {
        assert!(!r.findings.iter().any(|f| f.code == code), "{code:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_follows_redirects_and_marks_them_off_list() {
    // /old redirige en 301 vers /a, que la liste ne contient pas.
    let (site, store, loc, meta, _root) =
        run_list(Variant::Standard, fast_settings(), &["/old", "/orphan"]).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(site.requested("/a"));
    assert!(!site.requested("/b") && !site.requested("/dup1"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert_eq!(r.rows.len(), 3);
    assert!(row(&r, "/a").off_list);
    assert!(!row(&r, "/old").off_list);
    assert!(!row(&r, "/orphan").off_list);
    assert_eq!(row(&r, "/old").status, 301);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_skips_disallowed_and_excluded_urls() {
    let settings = CrawlSettings {
        ignore_regex: vec!["/tag/".to_string()],
        ..fast_settings()
    };
    let (site, store, loc, meta, _root) = run_list(
        Variant::RobotsDisallow,
        settings,
        &["/private/secret", "/tag/x", "/orphan", "/"],
    )
    .await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(!site.requested("/private/secret") && !site.requested("/tag/x"));
    assert!(site.requested("/orphan") && site.requested("/"));
    // Le nom du crawl reste la première adresse donnée, même écartée.
    assert_eq!(meta.start_url, format!("{}/private/secret", site.base));
    assert_eq!(meta.list_urls.len(), 4);
    assert_eq!(meta.pages, 2);

    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    let skipped: Vec<(String, SkipReason)> = r
        .list_skipped
        .iter()
        .map(|s| (path_of(&s.url), s.reason))
        .collect();
    assert_eq!(
        skipped,
        [
            ("/private/secret".to_string(), SkipReason::Robots),
            ("/tag/x".to_string(), SkipReason::Pattern),
        ]
    );
    assert_eq!(r.rows.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_fails_when_everything_is_skipped() {
    let (site, store, loc, meta, _root) = run_list(
        Variant::RobotsDisallow,
        fast_settings(),
        &["/private/secret"],
    )
    .await;
    let expected = CrawlStatus::Failed {
        message: "Toutes les adresses de la liste sont interdites par robots.txt ou exclues par les motifs."
            .to_string(),
    };
    assert_eq!(meta.status, expected);
    assert!(!site.requested("/private/secret"));
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, expected);
    assert_eq!(data.list_skipped.len(), 1);
    assert!(data.pages.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_runs_post_crawl_probes() {
    let (site, store, loc, meta, _root) =
        run_list(Variant::ExternalLinks, fast_settings(), &["/"]).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(!site.requested("/p2"));
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    // Compression (deuxième requête sur "/") et liens externes, une requête par adresse.
    assert_eq!(data.start_content_encoding.as_deref(), Some(""));
    assert_eq!(site.count("/"), 2);
    assert_eq!(data.external_links.len(), 10);
    assert_eq!(site.count("/ext-ok"), 1);
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has_with_related(
        &r,
        FindingCode::BrokenExternalLink,
        "/ext-404",
        &["/"]
    ));
    assert!(has(&r, FindingCode::ModernCompressionMissing, "/"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_image_of_a_listed_page_is_not_off_list() {
    let (site, store, loc, meta, _root) =
        run_list(Variant::Standard, fast_settings(), &["/b"]).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert!(site.requested("/img/big.jpg"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(!row(&r, "/img/big.jpg").off_list);
    assert!(!row(&r, "/b").off_list);
}

// Fausses pages d'erreur et robots.txt

fn detail_of(r: &CrawlResult, code: FindingCode, path: &str) -> Option<String> {
    r.findings
        .iter()
        .find(|f| f.code == code && path_of(&f.url) == path)
        .and_then(|f| f.detail.clone())
}

fn count_of(r: &CrawlResult, code: FindingCode) -> usize {
    r.findings.iter().filter(|f| f.code == code).count()
}

#[tokio::test(flavor = "multi_thread")]
async fn site_answering_200_to_unknown_urls() {
    let (site, store, loc, meta, _root) =
        run(Variant::UnknownAnswers200, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 1);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    let probe = data.not_found_probe.unwrap();
    assert_eq!((probe.status, probe.redirected), (200, false));
    assert_eq!(probe.title.as_deref(), Some("Page introuvable - Témoin"));
    assert_eq!(probe.h1.as_deref(), Some("Oups, page introuvable"));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::Soft404Site, "/"));
    assert_eq!(
        detail_of(&r, FindingCode::Soft404Site, "/").as_deref(),
        Some("répond 200")
    );
    assert_eq!(count_of(&r, FindingCode::Soft404), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn site_redirecting_unknown_urls_to_home() {
    let (site, store, loc, meta, _root) =
        run(Variant::UnknownRedirectsHome, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 1);
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert_eq!(
        detail_of(&r, FindingCode::Soft404Site, "/"),
        Some(format!("redirige vers {}/", site.base))
    );
    assert_eq!(count_of(&r, FindingCode::Soft404), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn probe_final_address_is_stored_without_query_or_fragment() {
    let (site, store, loc, meta, _root) =
        run(Variant::UnknownRedirectsToLogin, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    let probe = data.not_found_probe.unwrap();
    assert!(probe.redirected);
    assert_eq!(probe.final_url, format!("{}/connexion", site.base));
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    let detail = detail_of(&r, FindingCode::Soft404Site, "/").unwrap();
    assert_eq!(detail, format!("redirige vers {}/connexion", site.base));
    assert!(!detail.contains("state"));
}

#[tokio::test(flavor = "multi_thread")]
async fn page_with_the_error_page_signature_is_a_soft_404() {
    let (_site, store, loc, meta, _root) =
        run(Variant::GhostErrorPage, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(data.not_found_probe.unwrap().status, 404);
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::Soft404, "/fantome"));
    // Même title que la page d'erreur mais un autre H1 : un article, pas une fausse erreur.
    assert!(!has(&r, FindingCode::Soft404, "/article-404"));
    assert_eq!(count_of(&r, FindingCode::Soft404), 1);
    assert_eq!(count_of(&r, FindingCode::Soft404Site), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn not_found_probe_shares_the_budget_and_respects_robots() {
    let site = start_fixture_site(Variant::ExternalLinks).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let port = site.base.rsplit(':').next().unwrap().to_string();
    let pages = vec![
        page_linking(
            &format!("{}/", site.base),
            &[&format!("http://localhost:{port}/ext-ok")],
        ),
        fixture_page(
            &format!("{}/x", site.base),
            200,
            Some(&format!("{}/canon-target", site.base)),
        ),
    ];
    let journal = || site.requests.lock().unwrap().clone();
    let all = |max: usize, robots: Option<&'static str>, cancel: CancellationToken| {
        let (pages, start) = (pages.clone(), start.clone());
        async move {
            post_probes_with(
                &probe_client(),
                Some(&follow_client()),
                true,
                robots,
                &start,
                &pages,
                max,
                Duration::ZERO,
                &cancel,
            )
            .await
        }
    };

    // Ordre : compression, adresse inexistante, cible de canonical, liens externes.
    let r = all(100, None, CancellationToken::new()).await;
    assert!(r.not_found.is_some());
    let log = journal();
    assert_eq!(log.len(), 4);
    assert_eq!(log[0], "/");
    assert!(log[1].starts_with(support::NOT_FOUND_PREFIX));
    assert_eq!(log[2], "/canon-target");
    assert_eq!(log[3], "/ext-ok");
    let token = log[1].trim_start_matches(support::NOT_FOUND_PREFIX);
    assert_eq!(token.len(), 16);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

    // Budget d'une requête : la compression seule.
    site.requests.lock().unwrap().clear();
    let r = all(3, None, CancellationToken::new()).await;
    assert!(r.not_found.is_none() && r.content_encoding.is_some());
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 0);

    // Adresse interdite par robots.txt : pas de sonde.
    site.requests.lock().unwrap().clear();
    let robots = Some("User-agent: *\nDisallow: /arpenteur-introuvable\n");
    let r = all(100, robots, CancellationToken::new()).await;
    assert!(r.not_found.is_none());
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 0);
    assert_eq!(site.count("/canon-target"), 1);

    // Crawl arrêté : pas de sonde.
    site.requests.lock().unwrap().clear();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let r = all(100, None, cancelled).await;
    assert!(r.not_found.is_none());
    assert!(journal().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn not_found_probe_is_none_on_timeout_and_cancel() {
    use arpenteur_core::not_found_probe::{not_found_url, probe_not_found};
    let site = start_fixture_site(Variant::NotFoundHangs).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let url = not_found_url(&start, "0123456789abcdef");
    assert_eq!(
        url.as_str(),
        format!("{}/arpenteur-introuvable-0123456789abcdef", site.base)
    );
    let _ = rustls::crypto::ring::default_provider().install_default();
    let short = arpenteur_core::external_links::external_client_with_timeout(
        "Arpenteur-test",
        Duration::from_millis(300),
    )
    .unwrap();
    let live = CancellationToken::new();
    assert_eq!(probe_not_found(&short, &url, &live).await, None);
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 1);

    // Annulé pendant la lecture : rien non plus.
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        canceller.cancel();
    });
    let began = Instant::now();
    assert_eq!(probe_not_found(&follow_client(), &url, &cancel).await, None);
    assert!(began.elapsed() < Duration::from_secs(4));
}

#[tokio::test(flavor = "multi_thread")]
async fn not_found_probe_is_not_sent_when_robots_forbids_it() {
    let (site, store, loc, meta, _root) =
        run(Variant::RobotsBlocksNotFound, |_| Arc::new(NoEvents)).await;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 0);
    let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert!(data.not_found_probe.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn robots_status_is_recorded_and_missing_only_on_404_or_410() {
    for (variant, status, missing) in [
        (Variant::Robots404, 404, true),
        (Variant::Robots410, 410, true),
        (Variant::Robots500, 500, false),
        (Variant::Standard, 200, false),
    ] {
        let (_site, store, loc, meta, _root) = run(variant, |_| Arc::new(NoEvents)).await;
        assert_eq!(meta.status, CrawlStatus::Completed);
        let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
        assert_eq!(data.sitemap.robots_status, Some(status));
        let r = store.load_result(&loc.domain, &loc.id).unwrap();
        assert_eq!(
            has(&r, FindingCode::RobotsTxtMissing, "/"),
            missing,
            "{status}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn list_mode_records_the_robots_status() {
    for (variant, status, missing) in [
        (Variant::Robots404, 404, true),
        (Variant::Robots500, 500, false),
    ] {
        let (_site, store, loc, meta, _root) =
            run_list(variant, fast_settings(), &["/", "/a"]).await;
        assert_eq!(meta.status, CrawlStatus::Completed);
        let (_, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
        assert_eq!(data.sitemap.robots_status, Some(status));
        let r = store.load_result(&loc.domain, &loc.id).unwrap();
        assert_eq!(has(&r, FindingCode::RobotsTxtMissing, "/"), missing);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn not_found_probe_reads_a_bounded_body() {
    use arpenteur_core::not_found_probe::{NOT_FOUND_BODY_LIMIT, not_found_url, probe_not_found};
    let site = start_fixture_site(Variant::HugeNotFoundBody).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let url = not_found_url(&start, "0123456789abcdef");
    let probe = probe_not_found(&follow_client(), &url, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(probe.status, 200);
    assert_eq!(probe.title.as_deref(), Some("Gros corps"));
    assert_eq!(probe.h1.as_deref(), Some("Gros H1"));
    // Le corps servi (3 Mo) dépasse la limite : la lecture s'arrête avant sa fin, mais le nombre
    // d'octets réellement lus n'est pas observable depuis le test (le noyau met en tampon).
    assert!(3_000_000 > NOT_FOUND_BODY_LIMIT);
    assert!(!probe.text_hash.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_follow_client_only_compression_and_canonical_probes_run() {
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
    let r = post_probes_with(
        &probe_client(),
        None,
        true,
        None,
        &start,
        &pages,
        100,
        Duration::ZERO,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(r.content_encoding.as_deref(), Some(""));
    assert_eq!(r.pages.len(), 1);
    assert!(r.not_found.is_none() && r.external.is_empty());
    assert_eq!(site.count_prefix(support::NOT_FOUND_PREFIX), 0);
}

/// Lance un crawl en tâche de fond : le test prend des instantanés pendant qu'il tourne.
struct Background {
    site: support::FixtureSite,
    task: tokio::task::JoinHandle<CrawlMeta>,
    cancel: CancellationToken,
    store: CrawlStore,
    loc: arpenteur_core::model::CrawlLocation,
    _root: tempfile::TempDir,
}

async fn spawn_crawl(variant: Variant, live: &LiveCrawl) -> Background {
    let site = start_fixture_site(variant).await;
    let root = tempfile::tempdir().unwrap();
    let store = CrawlStore::new(root.path().to_path_buf());
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let loc = store
        .create_crawl(start.host_str().unwrap(), chrono::Local::now())
        .unwrap();
    let cancel = CancellationToken::new();
    let task = tokio::spawn(run_crawl(
        fast_settings(),
        CrawlTarget::Site(start),
        store.clone(),
        loc.clone(),
        Arc::new(NoEvents),
        cancel.clone(),
        live.clone(),
    ));
    Background {
        site,
        task,
        cancel,
        store,
        loc,
        _root: root,
    }
}

/// Attend qu'une condition soit vraie, sans compter sur un délai fixe.
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "délai dépassé : {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_during_the_crawl_shows_collected_pages() {
    let live = LiveCrawl::new();
    let run = spawn_crawl(Variant::SlowWithSitemap, &live).await;
    wait_until("première page collectée", || {
        live.snapshot().is_some_and(|v| !v.rows.is_empty())
    })
    .await;
    let view = live.snapshot().unwrap();

    assert_eq!(view.meta.status, CrawlStatus::Running);
    assert_eq!(view.meta.pages as usize, view.rows.len());
    assert!(view.rows.iter().all(|r| r.url.starts_with(&run.site.base)));
    // Le sitemap lu avant le crawl est connu de l'instantané.
    assert!(
        view.findings
            .iter()
            .all(|f| f.code != FindingCode::SitemapNotFound),
        "{:?}",
        view.findings
    );

    let meta = run.task.await.unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.pages as usize, support::LINKED_PAGES + 1);
    assert!(
        view.rows.len() < meta.pages as usize,
        "{} lignes pour {} pages",
        view.rows.len(),
        meta.pages
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_is_none_outside_the_collection() {
    let live = LiveCrawl::new();
    assert!(live.snapshot().is_none());

    let run = spawn_crawl(Variant::NotFoundHangs, &live).await;
    // La sonde d'adresse inexistante est la première des sondes qui ne répond jamais.
    wait_until("sonde d'adresse inexistante", || {
        run.site.count_prefix(support::NOT_FOUND_PREFIX) > 0
    })
    .await;
    assert!(live.snapshot().is_none(), "pendant les sondes de fin");

    run.cancel.cancel();
    let meta = run.task.await.unwrap();
    assert!(live.snapshot().is_none(), "après la fin");
    let (_, data) = run.store.read_crawl(&run.loc.domain, &run.loc.id).unwrap();
    assert_eq!(meta.pages as usize, data.pages.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_is_none_when_the_crawl_fails_before_collecting() {
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
    let live = LiveCrawl::new();
    let meta = run_crawl(
        fast_settings(),
        CrawlTarget::Site(start),
        store,
        loc,
        Arc::new(NoEvents),
        CancellationToken::new(),
        live.clone(),
    )
    .await;
    assert!(matches!(meta.status, CrawlStatus::Failed { .. }));
    assert!(live.snapshot().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_does_not_slow_the_crawl() {
    let live = LiveCrawl::new();
    let run = spawn_crawl(Variant::Slow, &live).await;
    wait_until("début de la collecte", || live.snapshot().is_some()).await;
    for _ in 0..50 {
        live.snapshot();
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    let meta = run.task.await.unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    let (_, data) = run.store.read_crawl(&run.loc.domain, &run.loc.id).unwrap();
    let mut urls: Vec<&str> = data.pages.iter().map(|p| p.url.as_str()).collect();
    urls.sort();
    urls.dedup();
    assert_eq!(urls.len(), support::LINKED_PAGES + 1);
    assert_eq!(data.pages.len(), support::LINKED_PAGES + 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_before_the_first_page_is_empty() {
    let live = LiveCrawl::new();
    let run = spawn_crawl(Variant::Slow, &live).await;
    // La première page met ~300 ms : le premier instantané arrive avant elle.
    let mut first = None;
    wait_until("début de la collecte", || {
        first = live.snapshot();
        first.is_some()
    })
    .await;
    let view = first.unwrap();
    assert!(view.rows.is_empty());
    assert_eq!(view.meta.pages, 0);
    assert_eq!(view.meta.status, CrawlStatus::Running);
    let start = format!("{}/", run.site.base);
    assert!(
        view.findings.iter().all(|f| f.url == start),
        "{:?}",
        view.findings
    );
    run.cancel.cancel();
    run.task.await.unwrap();
}
