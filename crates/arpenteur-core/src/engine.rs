//! Enregistrement des analyseurs de siteone-crawler.
//! Manager::run() future is Send: tokio::spawn is used.
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
