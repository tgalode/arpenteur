//! Analyseur Arpenteur branché sur SiteOne : un `PageRecord` par URL visitée.
//! `analyze_visited_url` s'exécute sous les verrous du crawler : rien de bloquant ici.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use siteone_crawler::analysis::analyzer::Analyzer;
use siteone_crawler::analysis::result::url_analysis_result::UrlAnalysisResult;
use siteone_crawler::output::output::Output;
use siteone_crawler::result::status::Status;
use siteone_crawler::result::visited_url::VisitedUrl;
use siteone_crawler::types::ContentTypeId;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::html::extract_page;
use crate::limit_guard::LimitGuard;
use crate::model::{PageRecord, Progress};

pub trait CrawlEvents: Send + Sync + 'static {
    /// Appelée sous les verrous de SiteOne : doit être rapide et non bloquante.
    fn progress(&self, p: &Progress);
}

pub struct PageCollector {
    events: Arc<dyn CrawlEvents>,
    guard: LimitGuard,
    cancel: CancellationToken,
    pages: Arc<Mutex<Vec<PageRecord>>>,
    throttled: Arc<AtomicBool>,
    started: Instant,
    times: HashMap<String, f64>,
    counts: HashMap<String, usize>,
}

#[derive(Clone)]
pub struct CollectorHandle {
    pages: Arc<Mutex<Vec<PageRecord>>>,
    throttled: Arc<AtomicBool>,
}

impl CollectorHandle {
    pub fn pages(&self) -> Vec<PageRecord> {
        self.pages.lock().unwrap().clone()
    }

    pub fn was_throttled(&self) -> bool {
        self.throttled.load(Ordering::SeqCst)
    }
}

impl PageCollector {
    pub fn new(
        events: Arc<dyn CrawlEvents>,
        limit_threshold: u32,
        cancel: CancellationToken,
    ) -> (PageCollector, CollectorHandle) {
        let pages = Arc::new(Mutex::new(Vec::new()));
        let throttled = Arc::new(AtomicBool::new(false));
        let handle = CollectorHandle {
            pages: pages.clone(),
            throttled: throttled.clone(),
        };
        let collector = PageCollector {
            events,
            guard: LimitGuard::new(limit_threshold),
            cancel,
            pages,
            throttled,
            started: Instant::now(),
            times: HashMap::new(),
            counts: HashMap::new(),
        };
        (collector, handle)
    }
}

fn content_type_name(id: ContentTypeId) -> &'static str {
    match id {
        ContentTypeId::Html => "html",
        ContentTypeId::Script => "script",
        ContentTypeId::Stylesheet => "stylesheet",
        ContentTypeId::Image => "image",
        ContentTypeId::Video => "video",
        ContentTypeId::Font => "font",
        ContentTypeId::Document => "document",
        ContentTypeId::Json => "json",
        ContentTypeId::Redirect => "redirect",
        ContentTypeId::Other => "other",
        ContentTypeId::Audio => "audio",
        ContentTypeId::Xml => "xml",
    }
}

impl Analyzer for PageCollector {
    fn analyze(&mut self, _status: &Status, _output: &mut dyn Output) {}

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        let headers: BTreeMap<String, String> = headers
            .map(|h| {
                h.iter()
                    .map(|(k, v)| (k.to_lowercase(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let is_html = visited_url.content_type == ContentTypeId::Html;
        let facts = match body {
            Some(html) if is_html && !html.is_empty() => Url::parse(&visited_url.url)
                .ok()
                .map(|page_url| extract_page(html, &page_url)),
            _ => None,
        };

        let record = PageRecord {
            url: visited_url.url.clone(),
            status: visited_url.status_code,
            content_type: content_type_name(visited_url.content_type).to_string(),
            size: visited_url.size,
            elapsed_ms: (visited_url.request_time * 1000.0).round() as u64,
            headers,
            facts,
        };

        let done = {
            let mut pages = self.pages.lock().unwrap();
            pages.push(record);
            pages.len() as u32
        };
        self.events.progress(&Progress {
            done,
            last_url: visited_url.url.clone(),
            last_status: visited_url.status_code,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
        });

        if self.guard.observe(visited_url.status_code) {
            self.throttled.store(true, Ordering::SeqCst);
            self.cancel.cancel();
        }
        None
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        10_000
    }

    fn get_name(&self) -> &str {
        "ArpenteurPageCollector"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        &self.times
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        &self.counts
    }
}
