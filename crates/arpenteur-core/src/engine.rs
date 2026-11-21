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

use crate::canonical_probe::{ProbeEnv, probe_client, run_post_crawl_probes};
use crate::collector::{CollectorHandle, CrawlEvents, PageCollector};
use crate::external_links::external_client;
use crate::model::{
    CollectedData, CrawlLocation, CrawlMeta, CrawlMode, CrawlSettings, CrawlStatus, FACTS_VERSION,
    SitemapResult, SkippedUrl,
};
use crate::settings::{
    CrawlError, UrlFilter, build_core_options, build_list_core_options, crawl_seeds,
    plain_user_agent,
};
use crate::sitemap::{discover, read_robots_txt};
use crate::storage::CrawlStore;

/// Code de sortie de `Manager::run` (manager.rs, `run_post_crawl`) quand aucune
/// réponse HTTP n'a été reçue : adresse erronée, site injoignable.
const NO_PAGE_LOADED: i32 = 3;
const NO_PAGE_LOADED_MESSAGE: &str =
    "Aucune page n'a pu être chargée. Vérifiez l'adresse ou la connexion.";
const EMPTY_LIST_MESSAGE: &str = "La liste est vide.";
const ALL_SKIPPED_MESSAGE: &str =
    "Toutes les adresses de la liste sont interdites par robots.txt ou exclues par les motifs.";

/// Ce que le crawl explore : un site à partir d'une adresse, ou une liste d'adresses
/// d'un même site, sans suivre les liens ni lire le sitemap.
pub enum CrawlTarget {
    Site(Url),
    List(Vec<Url>),
}

impl CrawlTarget {
    /// L'adresse qui nomme le crawl : la première de la liste telle que donnée.
    fn first(&self) -> Option<&Url> {
        match self {
            CrawlTarget::Site(start) => Some(start),
            CrawlTarget::List(urls) => urls.first(),
        }
    }
}

struct Executed {
    status: CrawlStatus,
    handle: Option<CollectorHandle>,
    sitemap: SitemapResult,
    list_skipped: Vec<SkippedUrl>,
}

impl Executed {
    fn new(status: CrawlStatus) -> Executed {
        Executed {
            status,
            handle: None,
            sitemap: SitemapResult::default(),
            list_skipped: Vec::new(),
        }
    }

    fn failed(message: String) -> Executed {
        Executed::new(CrawlStatus::Failed { message })
    }
}

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

/// Préparation (sitemap, ou robots.txt et filtre de la liste) puis crawl SiteOne.
async fn execute(
    settings: &CrawlSettings,
    target: &CrawlTarget,
    location: &CrawlLocation,
    events: Arc<dyn CrawlEvents>,
    cancel: &CancellationToken,
) -> Executed {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let prepared = http_client(settings).and_then(|client| Ok((client, sitemap_budget(settings)?)));
    let (client, (min_interval, max_urls)) = match prepared {
        Ok(p) => p,
        Err(e) => return Executed::failed(e.to_string()),
    };

    let mut executed = Executed::new(CrawlStatus::Running);
    let options = match target {
        CrawlTarget::Site(start) => {
            executed.sitemap = tokio::select! {
                s = discover(&client, start, min_interval, max_urls) => s,
                _ = cancel.cancelled() => return Executed::new(CrawlStatus::Stopped),
            };
            if cancel.is_cancelled() {
                executed.status = CrawlStatus::Stopped;
                return executed;
            }
            // Les URL écartées restent dans `sitemap.urls` : le contenu du sitemap est un fait.
            let seeds = crawl_seeds(settings, &executed.sitemap);
            build_core_options(settings, start, &location.dir, seeds)
        }
        CrawlTarget::List(urls) => {
            let Some(first) = urls.first() else {
                return Executed::failed(EMPTY_LIST_MESSAGE.to_string());
            };
            let robots_txt = tokio::select! {
                r = read_robots_txt(&client, first) => r,
                _ = cancel.cancelled() => return Executed::new(CrawlStatus::Stopped),
            };
            let filter = UrlFilter::new(settings, robots_txt.as_deref());
            executed.sitemap = SitemapResult {
                robots_txt,
                ..SitemapResult::default()
            };
            let mut allowed = Vec::new();
            for url in urls {
                match filter.check(url.as_str()) {
                    Ok(()) => allowed.push(url),
                    Err(reason) => executed.list_skipped.push(SkippedUrl {
                        url: url.to_string(),
                        reason,
                    }),
                }
            }
            // SiteOne demande toujours son URL de départ sans la filtrer : ce sera la première
            // adresse autorisée.
            let Some((start, rest)) = allowed.split_first() else {
                executed.status = CrawlStatus::Failed {
                    message: ALL_SKIPPED_MESSAGE.to_string(),
                };
                return executed;
            };
            let rest = rest.iter().map(|u| u.to_string()).collect();
            build_list_core_options(settings, start, &location.dir, rest)
        }
    };
    let options = match options {
        Ok(o) => o,
        Err(e) => {
            executed.status = CrawlStatus::Failed {
                message: e.to_string(),
            };
            return executed;
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
            executed.status = CrawlStatus::Failed {
                message: e.to_string(),
            };
            executed.handle = Some(handle);
            return executed;
        }
    };

    // Annuler le futur arrête la boucle de SiteOne : une URL = une tâche déjà lancée,
    // donc au plus `workers` requêtes en vol se terminent, aucune nouvelle ne part.
    // `biased` : l'annulation est vue avant que la boucle ne lance une autre tâche.
    executed.status = tokio::select! {
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
    executed.handle = Some(handle);
    executed
}

/// Ne renvoie jamais d'erreur : un échec devient `CrawlStatus::Failed`.
pub async fn run_crawl(
    settings: CrawlSettings,
    target: CrawlTarget,
    store: CrawlStore,
    location: CrawlLocation,
    events: Arc<dyn CrawlEvents>,
    cancel: CancellationToken,
) -> CrawlMeta {
    let (mode, list_urls) = match &target {
        CrawlTarget::Site(_) => (CrawlMode::Site, Vec::new()),
        CrawlTarget::List(urls) => (CrawlMode::List, urls.iter().map(Url::to_string).collect()),
    };
    let mut meta = CrawlMeta {
        id: location.id.clone(),
        domain: location.domain.clone(),
        start_url: target.first().map(Url::to_string).unwrap_or_default(),
        started_at: chrono::Local::now().to_rfc3339(),
        finished_at: None,
        status: CrawlStatus::Running,
        settings: settings.clone(),
        pages: 0,
        mode,
        list_urls,
    };
    let write_failure = |e: std::io::Error| CrawlStatus::Failed {
        message: format!("Écriture impossible : {e}"),
    };
    let initial_write = store.write_meta(&location, &meta);

    let Executed {
        status,
        handle,
        sitemap,
        list_skipped,
    } = match initial_write {
        Err(e) => Executed::new(write_failure(e)),
        Ok(()) => execute(&settings, &target, &location, events, &cancel).await,
    };

    let mut pages = handle.map(|h| h.pages()).unwrap_or_default();
    let mut start_content_encoding = None;
    let (mut external_links, mut external_unchecked) = (Vec::new(), 0);
    if let (CrawlStatus::Completed, Some(start)) = (&status, target.first()) {
        // Après un crawl terminé seulement, au même débit et dans la même limite.
        if let Ok((min_interval, _)) = sitemap_budget(&settings) {
            if let Ok(client) = probe_client(&plain_user_agent(&settings)) {
                let filter = UrlFilter::new(&settings, sitemap.robots_txt.as_deref());
                let env = ProbeEnv {
                    client: &client,
                    start,
                    filter: &filter,
                    min_interval,
                    cancel: &cancel,
                };
                let external = settings
                    .check_external_links
                    .then(|| external_client(&plain_user_agent(&settings)).ok())
                    .flatten();
                let probed = run_post_crawl_probes(
                    &env,
                    &pages,
                    settings.max_requests as usize,
                    external.as_ref(),
                )
                .await;
                start_content_encoding = probed.content_encoding;
                external_links = probed.external;
                external_unchecked = probed.external_unchecked;
                pages.extend(probed.pages);
            }
        }
    }
    meta.pages = pages.iter().filter(|p| !p.probed).count() as u32;
    meta.status = status;
    meta.finished_at = Some(chrono::Local::now().to_rfc3339());

    if let Err(e) = store.write_data(
        &location,
        &CollectedData {
            pages,
            sitemap,
            facts_version: FACTS_VERSION,
            start_content_encoding,
            external_links,
            external_unchecked,
            list_skipped,
        },
    ) {
        meta.status = write_failure(e);
    }
    if let Err(e) = store.write_meta(&location, &meta) {
        meta.status = write_failure(e);
    }
    meta
}
