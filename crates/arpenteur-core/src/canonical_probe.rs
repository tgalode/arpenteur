//! Requêtes propres de l'application après un crawl terminé : cibles de canonical
//! jamais demandées par SiteOne, et compression observée sur une page du site.

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT_ENCODING, CONTENT_ENCODING};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::model::PageRecord;
use crate::urls::{is_same_site, normalize_url};

pub struct PostCrawl {
    pub content_encoding: Option<String>,
    pub pages: Vec<PageRecord>,
}

/// Client des sondes : ni redirection suivie, ni décompression automatique
/// (sinon reqwest retire `Content-Encoding` de la réponse).
pub fn probe_client(user_agent: &str) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(user_agent)
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
}

/// `Content-Encoding` tel que reçu (`""` s'il est absent) pour une réponse 200.
/// Tout autre cas (statut, erreur réseau, annulation) donne `None` : on ne sait pas.
pub async fn probe_content_encoding(
    client: &reqwest::Client,
    url: &Url,
    cancel: &CancellationToken,
) -> Option<String> {
    if cancel.is_cancelled() {
        return None;
    }
    let request = client
        .get(url.clone())
        .header(ACCEPT_ENCODING, "gzip, deflate, br")
        .send();
    let response = tokio::select! {
        r = request => r.ok()?,
        _ = cancel.cancelled() => return None,
    };
    if response.status() != reqwest::StatusCode::OK {
        return None;
    }
    let value = response
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Some(value.trim().to_string())
}

fn probe_targets(pages: &[PageRecord], start: &Url) -> Vec<Url> {
    let crawled: HashSet<String> = pages
        .iter()
        .map(|p| normalize_url(&p.url).unwrap_or_else(|| p.url.clone()))
        .collect();
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for canonical in pages
        .iter()
        .filter_map(|p| p.facts.as_ref()?.canonical.as_deref())
    {
        let Some(normalized) = normalize_url(canonical) else {
            continue;
        };
        let Ok(url) = Url::parse(&normalized) else {
            continue;
        };
        let web = matches!(url.scheme(), "http" | "https");
        if web
            && is_same_site(start, &url)
            && !crawled.contains(&normalized)
            && seen.insert(normalized)
        {
            targets.push(url);
        }
    }
    targets
}

async fn probe_one(client: &reqwest::Client, url: &Url) -> PageRecord {
    let began = Instant::now();
    let result = client.get(url.clone()).send().await;
    let elapsed_ms = began.elapsed().as_millis() as u64;
    let (status, headers) = match result {
        Ok(r) => {
            let headers: BTreeMap<String, String> = r
                .headers()
                .iter()
                .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect();
            (i32::from(r.status().as_u16()), headers)
        }
        Err(_) => (-1, BTreeMap::new()),
    };
    let content_type = if (300..400).contains(&status) {
        "redirect"
    } else {
        "html"
    };
    PageRecord {
        url: url.to_string(),
        status,
        content_type: content_type.to_string(),
        size: None,
        elapsed_ms,
        headers,
        facts: None,
    }
}

/// Interroge les cibles de canonical internes que le crawl n'a pas demandées,
/// au plus `budget` requêtes, espacées de `min_interval`.
pub async fn probe_canonical_targets(
    client: &reqwest::Client,
    pages: &[PageRecord],
    start: &Url,
    budget: usize,
    min_interval: Duration,
    cancel: &CancellationToken,
) -> Vec<PageRecord> {
    let mut added = Vec::new();
    for url in probe_targets(pages, start).into_iter().take(budget) {
        tokio::select! {
            _ = tokio::time::sleep(min_interval) => {}
            _ = cancel.cancelled() => break,
        }
        let record = tokio::select! {
            r = probe_one(client, &url) => r,
            _ = cancel.cancelled() => break,
        };
        added.push(record);
    }
    added
}

/// Compression d'abord (un constat de site), puis cibles de canonical, dans le
/// budget restant `max_requests - pages crawlées`.
pub async fn run_post_crawl_probes(
    client: &reqwest::Client,
    pages: &[PageRecord],
    start: &Url,
    max_requests: usize,
    min_interval: Duration,
    cancel: &CancellationToken,
) -> PostCrawl {
    let mut budget = max_requests.saturating_sub(pages.len());
    let mut content_encoding = None;
    if budget > 0 && !cancel.is_cancelled() {
        let sample = pages.iter().find(|p| {
            p.is_html()
                && p.status == 200
                && Url::parse(&p.url).is_ok_and(|u| is_same_site(start, &u))
        });
        if let Some(url) = sample.and_then(|p| Url::parse(&p.url).ok()) {
            budget -= 1;
            content_encoding = probe_content_encoding(client, &url, cancel).await;
        }
    }
    let pages = probe_canonical_targets(client, pages, start, budget, min_interval, cancel).await;
    PostCrawl {
        content_encoding,
        pages,
    }
}
