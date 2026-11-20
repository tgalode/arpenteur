//! Lancement d'un crawl et enregistrement des analyseurs de siteone-crawler.
//! Il exige un runtime multi-thread (DnsAnalyzer appelle block_in_place).

use siteone_crawler::analysis::accessibility_analyzer::AccessibilityAnalyzer;
use siteone_crawler::analysis::best_practice_analyzer::BestPracticeAnalyzer;
use siteone_crawler::analysis::browser_console_analyzer::BrowserConsoleAnalyzer;
use siteone_crawler::analysis::caching_analyzer::CachingAnalyzer;
use siteone_crawler::analysis::content_type_analyzer::ContentTypeAnalyzer;
use siteone_crawler::analysis::dns_analyzer::DnsAnalyzer;
use siteone_crawler::analysis::external_links_analyzer::ExternalLinksAnalyzer;
use siteone_crawler::analysis::fastest_analyzer::FastestAnalyzer;
use siteone_crawler::analysis::headers_analyzer::HeadersAnalyzer;
use siteone_crawler::analysis::manager::AnalysisManager;
use siteone_crawler::analysis::page404_analyzer::Page404Analyzer;
use siteone_crawler::analysis::redirects_analyzer::RedirectsAnalyzer;
use siteone_crawler::analysis::security_analyzer::SecurityAnalyzer;
use siteone_crawler::analysis::seo_opengraph_analyzer::SeoAndOpenGraphAnalyzer;
use siteone_crawler::analysis::skipped_urls_analyzer::SkippedUrlsAnalyzer;
use siteone_crawler::analysis::slowest_analyzer::SlowestAnalyzer;
use siteone_crawler::analysis::source_domains_analyzer::SourceDomainsAnalyzer;
use siteone_crawler::analysis::ssl_tls::SslTlsAnalyzer;
use siteone_crawler::options::core_options::CoreOptions;

/// Recopie de `Initiator::register_analyzers` de siteone-crawler 2.5.1
/// (privée, et `Initiator::new` appelle `std::process::exit`).
pub fn register_builtin_analyzers(am: &mut AnalysisManager, options: &CoreOptions) {
    am.register_analyzer(Box::new(AccessibilityAnalyzer::new()));
    am.register_analyzer(Box::new(BestPracticeAnalyzer::new()));

    let mut browser_console = BrowserConsoleAnalyzer::new();
    browser_console.set_activated(options.browser_enabled);
    am.register_analyzer(Box::new(browser_console));

    am.register_analyzer(Box::new(CachingAnalyzer::new()));
    am.register_analyzer(Box::new(ContentTypeAnalyzer::new()));
    am.register_analyzer(Box::new(DnsAnalyzer::new()));
    am.register_analyzer(Box::new(ExternalLinksAnalyzer::new()));

    let mut fastest = FastestAnalyzer::new();
    fastest.set_config(options.fastest_top_limit as usize, options.fastest_max_time);
    am.register_analyzer(Box::new(fastest));

    am.register_analyzer(Box::new(HeadersAnalyzer::new()));
    am.register_analyzer(Box::new(Page404Analyzer::new()));
    am.register_analyzer(Box::new(RedirectsAnalyzer::new()));
    am.register_analyzer(Box::new(SecurityAnalyzer::new()));

    let mut seo = SeoAndOpenGraphAnalyzer::new();
    seo.set_config(options.max_heading_level as i32);
    am.register_analyzer(Box::new(seo));

    am.register_analyzer(Box::new(SkippedUrlsAnalyzer::new()));

    let mut slowest = SlowestAnalyzer::new();
    slowest.set_config(
        options.slowest_top_limit as usize,
        options.slowest_min_time,
        options.slowest_max_time,
    );
    am.register_analyzer(Box::new(slowest));

    am.register_analyzer(Box::new(SourceDomainsAnalyzer::new()));

    let mut ssl_tls = SslTlsAnalyzer::new();
    ssl_tls.set_config(options.accept_invalid_certs);
    am.register_analyzer(Box::new(ssl_tls));
}

use std::sync::Arc;
use std::time::Duration;

use siteone_crawler::engine::manager::Manager;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::canonical_probe::{probe_client, run_post_crawl_probes};
use crate::collector::{CollectorHandle, CrawlEvents, PageCollector};
use crate::model::{
    CollectedData, CrawlLocation, CrawlMeta, CrawlSettings, CrawlStatus, FACTS_VERSION,
    SitemapResult,
};
use crate::settings::{CrawlError, build_core_options, crawl_seeds, plain_user_agent};
use crate::sitemap::discover;
use crate::storage::CrawlStore;

/// Code de sortie de `Manager::run` (manager.rs, `run_post_crawl`) quand aucune
/// réponse HTTP n'a été reçue : adresse erronée, site injoignable.
const NO_PAGE_LOADED: i32 = 3;
const NO_PAGE_LOADED_MESSAGE: &str =
    "Aucune page n'a pu être chargée. Vérifiez l'adresse ou la connexion.";

fn http_client(settings: &CrawlSettings) -> Result<reqwest::Client, CrawlError> {
    reqwest::Client::builder()
        .user_agent(plain_user_agent(settings))
        .build()
        .map_err(|e| CrawlError::Options(e.to_string()))
}

fn sitemap_budget(settings: &CrawlSettings) -> Result<(Duration, usize), CrawlError> {
    if !(settings.max_reqs_per_sec > 0.0 && settings.max_reqs_per_sec.is_finite()) {
        return Err(CrawlError::Options(
            "le débit maximal doit être strictement positif".to_string(),
        ));
    }
    Ok((
        Duration::from_secs_f64(1.0 / settings.max_reqs_per_sec),
        settings.max_requests as usize,
    ))
}

/// Découverte du sitemap puis crawl SiteOne ; renvoie le statut final et les pages collectées.
async fn execute(
    settings: &CrawlSettings,
    start: &Url,
    location: &CrawlLocation,
    events: Arc<dyn CrawlEvents>,
    cancel: &CancellationToken,
) -> (CrawlStatus, Option<CollectorHandle>, SitemapResult) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let prepared = http_client(settings).and_then(|client| Ok((client, sitemap_budget(settings)?)));
    let (client, (min_interval, max_urls)) = match prepared {
        Ok(p) => p,
        Err(e) => {
            return (
                CrawlStatus::Failed {
                    message: e.to_string(),
                },
                None,
                SitemapResult::default(),
            );
        }
    };

    let sitemap = tokio::select! {
        s = discover(&client, start, min_interval, max_urls) => s,
        _ = cancel.cancelled() => return (CrawlStatus::Stopped, None, SitemapResult::default()),
    };
    if cancel.is_cancelled() {
        return (CrawlStatus::Stopped, None, sitemap);
    }

    // Les URL écartées restent dans `sitemap.urls` : le contenu du sitemap est un fait.
    let seeds = crawl_seeds(settings, &sitemap);
    let options = match build_core_options(settings, start, &location.dir, seeds) {
        Ok(o) => o,
        Err(e) => {
            return (
                CrawlStatus::Failed {
                    message: e.to_string(),
                },
                None,
                sitemap,
            );
        }
    };
    let mut am = siteone_crawler::analysis::manager::AnalysisManager::new();
    register_builtin_analyzers(&mut am, &options);
    let (collector, handle) = PageCollector::new(events, settings.limit_threshold, cancel.clone());
    am.register_analyzer(Box::new(collector));
    am.auto_activate_analyzers();
    let mut manager = match Manager::new(options, am) {
        Ok(m) => m,
        Err(e) => {
            return (
                CrawlStatus::Failed {
                    message: e.to_string(),
                },
                Some(handle),
                sitemap,
            );
        }
    };

    // Annuler le futur arrête la boucle de SiteOne : une URL = une tâche déjà lancée,
    // donc au plus `workers` requêtes en vol se terminent, aucune nouvelle ne part.
    // `biased` : l'annulation est vue avant que la boucle ne lance une autre tâche.
    let status = tokio::select! {
        biased;
        _ = cancel.cancelled() => if handle.was_throttled() {
            CrawlStatus::Throttled
        } else {
            CrawlStatus::Stopped
        },
        result = manager.run() => match result {
            // Seuil atteint sur la dernière URL : la boucle a fini dans le même tour.
            Ok(_) if handle.was_throttled() => CrawlStatus::Throttled,
            Ok(NO_PAGE_LOADED) => CrawlStatus::Failed { message: NO_PAGE_LOADED_MESSAGE.to_string() },
            Ok(_) => CrawlStatus::Completed,
            Err(e) => CrawlStatus::Failed { message: e.to_string() },
        },
    };
    (status, Some(handle), sitemap)
}

/// Ne renvoie jamais d'erreur : un échec devient `CrawlStatus::Failed`.
pub async fn run_crawl(
    settings: CrawlSettings,
    start: Url,
    store: CrawlStore,
    location: CrawlLocation,
    events: Arc<dyn CrawlEvents>,
    cancel: CancellationToken,
) -> CrawlMeta {
    let mut meta = CrawlMeta {
        id: location.id.clone(),
        domain: location.domain.clone(),
        start_url: start.as_str().to_string(),
        started_at: chrono::Local::now().to_rfc3339(),
        finished_at: None,
        status: CrawlStatus::Running,
        settings: settings.clone(),
        pages: 0,
    };
    let write_failure = |e: std::io::Error| CrawlStatus::Failed {
        message: format!("Écriture impossible : {e}"),
    };
    let initial_write = store.write_meta(&location, &meta);

    let (status, handle, sitemap) = match initial_write {
        Err(e) => (write_failure(e), None, SitemapResult::default()),
        Ok(()) => execute(&settings, &start, &location, events, &cancel).await,
    };

    let mut pages = handle.map(|h| h.pages()).unwrap_or_default();
    let mut start_content_encoding = None;
    if status == CrawlStatus::Completed {
        // Après un crawl terminé seulement, au même débit et dans la même limite.
        if let Ok((min_interval, _)) = sitemap_budget(&settings) {
            if let Ok(client) = probe_client(&plain_user_agent(&settings)) {
                let probed = run_post_crawl_probes(
                    &client,
                    &pages,
                    &start,
                    settings.max_requests as usize,
                    min_interval,
                    &cancel,
                )
                .await;
                start_content_encoding = probed.content_encoding;
                pages.extend(probed.pages);
            }
        }
    }
    meta.pages = pages.len() as u32;
    meta.status = status;
    meta.finished_at = Some(chrono::Local::now().to_rfc3339());

    if let Err(e) = store.write_data(
        &location,
        &CollectedData {
            pages,
            sitemap,
            facts_version: FACTS_VERSION,
            start_content_encoding,
        },
    ) {
        meta.status = write_failure(e);
    }
    if let Err(e) = store.write_meta(&location, &meta) {
        meta.status = write_failure(e);
    }
    meta
}
