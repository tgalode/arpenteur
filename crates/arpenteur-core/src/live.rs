//! Instantané d'un crawl en cours : lignes et constats des pages déjà collectées,
//! calculés par le même code qu'à la fin.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::checks::{CheckInput, build_rows, run_checks};
use crate::collector::CollectorHandle;
use crate::links::analyze_links;
use crate::model::{CrawlMeta, CrawlMode, Finding, SitemapResult, UrlRow};

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
        let (handle, sitemap, mut meta) = {
            let slot = self.lock();
            let c = slot.as_ref()?;
            (c.handle.clone(), c.sitemap.clone(), c.meta.clone())
        };
        // Seule la copie des pages se fait sous le verrou du collecteur.
        let pages = handle.pages();
        let links = analyze_links(&pages, &meta.start_url);
        let findings = run_checks(&CheckInput {
            list: (meta.mode == CrawlMode::List).then_some(&meta.list_urls[..]),
            partial: true,
            ..CheckInput::new(&pages, &links, &sitemap, &meta.start_url)
        });
        let rows = build_rows(&pages, &links, meta.mode, &meta.list_urls);
        meta.pages = pages.len() as u32;
        Some(LiveView {
            meta,
            rows,
            findings,
        })
    }
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        *self.0.lock() = None;
    }
}
