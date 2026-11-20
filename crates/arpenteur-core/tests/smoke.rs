mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use arpenteur_core::collector::{CrawlEvents, PageCollector};
use arpenteur_core::engine::register_builtin_analyzers;
use arpenteur_core::model::{PageRecord, Progress};
use arpenteur_core::settings::ASSET_IGNORE_REGEX;
use siteone_crawler::analysis::analyzer::Analyzer;
use siteone_crawler::analysis::manager::AnalysisManager;
use siteone_crawler::analysis::result::url_analysis_result::UrlAnalysisResult;
use siteone_crawler::engine::manager::Manager;
use siteone_crawler::options::core_options::{CoreOptions, parse_argv};
use siteone_crawler::result::visited_url::VisitedUrl;
use tokio_util::sync::CancellationToken;
use support::{FixtureSite, Variant, start_fixture_site};

type Captured = Arc<Mutex<Vec<(String, String)>>>;

struct BodyProbe {
    captured: Captured,
    times: HashMap<String, f64>,
    counts: HashMap<String, usize>,
}

impl Analyzer for BodyProbe {
    fn analyze(
        &mut self,
        _status: &siteone_crawler::result::status::Status,
        _output: &mut dyn siteone_crawler::output::output::Output,
    ) {
    }

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        _headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        if let Some(body) = body {
            self.captured
                .lock()
                .unwrap()
                .push((visited_url.url.clone(), body.to_string()));
        }
        None
    }

    fn should_be_activated(&self) -> bool {
        true
    }
    fn get_order(&self) -> i32 {
        1000
    }
    fn get_name(&self) -> &str {
        "BodyProbe"
    }
    fn get_exec_times(&self) -> &HashMap<String, f64> {
        &self.times
    }
    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        &self.counts
    }
}

fn base_argv(site: &FixtureSite, tmp: &std::path::Path) -> Vec<String> {
    [
        format!("--url={}/", site.base),
        "--workers=2".to_string(),
        "--max-reqs-per-sec=50".to_string(),
        "--http-cache-dir=off".to_string(),
        format!("--output-json-file={}/siteone.json", tmp.display()),
        "--output-html-report=".to_string(),
        "--output-text-file=".to_string(),
        "--no-color".to_string(),
    ]
    .to_vec()
}

fn manager_with_probe(options: CoreOptions, captured: Captured) -> Manager {
    let mut am = AnalysisManager::new();
    register_builtin_analyzers(&mut am, &options);
    am.register_analyzer(Box::new(BodyProbe {
        captured,
        times: HashMap::new(),
        counts: HashMap::new(),
    }));
    am.auto_activate_analyzers();
    Manager::new(options, am).expect("manager")
}

fn assert_send<T: Send>(_: &T) {}

async fn crawl(site: &FixtureSite, extra: Vec<String>, seed: Option<String>) -> Captured {
    let tmp = std::env::temp_dir().join(format!("arpenteur-smoke-{}", site.base.replace([':', '/'], "_")));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut argv = base_argv(site, &tmp);
    argv.extend(extra);
    let mut options = parse_argv(&argv).expect("parse_argv");
    if let Some(url) = seed {
        options.url_list_urls = vec![url];
    }
    let captured: Captured = Arc::default();
    let mut manager = manager_with_probe(options, captured.clone());
    let fut = manager.run();
    assert_send(&fut);
    fut.await.expect("crawl");
    captured
}

#[tokio::test(flavor = "multi_thread")]
async fn body_given_to_analyzers_is_raw_html() {
    let site = start_fixture_site(Variant::Standard).await;
    let captured = crawl(&site, vec![], None).await;
    let pages = captured.lock().unwrap();
    let (_, body) = pages
        .iter()
        .find(|(url, _)| url == &format!("{}/", site.base))
        .expect("body of / captured");
    assert!(body.contains("<title>Accueil témoin</title>"));
    assert!(body.contains(r#"<script type="application/ld+json">"#));
}

#[tokio::test(flavor = "multi_thread")]
async fn asset_regex_prevents_js_and_css_requests() {
    let site = start_fixture_site(Variant::Standard).await;
    crawl(&site, vec![format!("--ignore-regex={}", ASSET_IGNORE_REGEX)], None).await;
    assert!(!site.requested("/app.js"));
    assert!(!site.requested("/style.css"));
    assert!(site.requested("/img/big.jpg"));
}

#[tokio::test(flavor = "multi_thread")]
async fn url_list_seeds_unlinked_pages() {
    let site = start_fixture_site(Variant::Standard).await;
    let seed = format!("{}/orphan", site.base);
    crawl(&site, vec![], Some(seed)).await;
    assert!(site.requested("/orphan"));
}

#[derive(Default)]
struct RecordingEvents {
    progress: Mutex<Vec<Progress>>,
}

impl CrawlEvents for RecordingEvents {
    fn progress(&self, p: &Progress) {
        self.progress.lock().unwrap().push(p.clone());
    }
}

fn page<'a>(pages: &'a [PageRecord], path: &str) -> &'a PageRecord {
    pages
        .iter()
        .find(|p| p.url.ends_with(path))
        .unwrap_or_else(|| panic!("page {path} not collected"))
}

#[tokio::test(flavor = "multi_thread")]
async fn collector_records_pages_and_progress() {
    let site = start_fixture_site(Variant::Standard).await;
    let tmp = std::env::temp_dir().join("arpenteur-smoke-collector");
    std::fs::create_dir_all(&tmp).unwrap();
    let options = parse_argv(&base_argv(&site, &tmp)).expect("parse_argv");

    let events = Arc::new(RecordingEvents::default());
    let (collector, handle) = PageCollector::new(events.clone(), 5, CancellationToken::new());
    let mut am = AnalysisManager::new();
    register_builtin_analyzers(&mut am, &options);
    am.register_analyzer(Box::new(collector));
    am.auto_activate_analyzers();
    let mut manager = Manager::new(options, am).expect("manager");
    manager.run().await.expect("crawl");

    let pages = handle.pages();
    let progress = events.progress.lock().unwrap();
    assert!(!handle.was_throttled());

    let home = page(&pages, "/");
    assert_eq!(home.status, 200);
    assert_eq!(home.facts.as_ref().unwrap().title.as_deref(), Some("Accueil témoin"));
    let old = page(&pages, "/old");
    assert_eq!(old.status, 301);
    assert!(old.location().unwrap().ends_with("/a"));
    let img = page(&pages, "/img/big.jpg");
    assert!(img.facts.is_none() && img.size.unwrap() >= 250_000);
    assert_eq!(progress.len(), pages.len());
    assert_eq!(progress.last().unwrap().done as usize, pages.len());
}
