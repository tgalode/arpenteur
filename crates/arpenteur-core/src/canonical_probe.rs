//! Requêtes propres de l'application après un crawl terminé : cibles de canonical
//! jamais demandées par SiteOne, et compression observée sur une page du site.

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::checks::reference_page;
use crate::external_links::{check_external_links, external_targets};
use crate::model::{ExternalCheck, PageRecord};
use crate::settings::UrlFilter;
use crate::urls::{is_same_site, normalize_url};

/// Délai de SiteOne pour ses propres requêtes.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Ce que les sondes partagent : client, site, règles de filtrage, débit et annulation.
pub struct ProbeEnv<'a> {
    pub client: &'a reqwest::Client,
    pub start: &'a Url,
    pub filter: &'a UrlFilter,
    pub min_interval: Duration,
    pub cancel: &'a CancellationToken,
}

pub struct PostCrawl {
    pub content_encoding: Option<String>,
    pub pages: Vec<PageRecord>,
    pub external: Vec<ExternalCheck>,
    /// Adresses externes non demandées : budget épuisé ou annulation.
    pub external_unchecked: u32,
}

/// Client des sondes : ni redirection suivie, ni décompression automatique
/// (sinon reqwest retire `Content-Encoding` de la réponse).
pub fn probe_client(user_agent: &str) -> Result<reqwest::Client, reqwest::Error> {
    probe_client_with_timeout(user_agent, PROBE_TIMEOUT)
}

pub fn probe_client_with_timeout(
    user_agent: &str,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
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

fn probe_targets(pages: &[PageRecord], start: &Url, filter: &UrlFilter) -> Vec<Url> {
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
            && filter.check(&normalized).is_ok()
            && seen.insert(normalized)
        {
            targets.push(url);
        }
    }
    targets
}

/// Type d'une réponse, avec le vocabulaire des pages crawlées. La correspondance de
/// SiteOne (`get_content_type_id_by_header`) est privée : on en reprend les cas utiles.
fn content_type_name(status: i32, header: Option<&str>) -> &'static str {
    if (300..400).contains(&status) {
        return "redirect";
    }
    let header = header.unwrap_or("").to_lowercase();
    if header.contains("text/html") {
        "html"
    } else if header.contains("image/") {
        "image"
    } else if header.contains("application/pdf") {
        "document"
    } else {
        "other"
    }
}

/// `None` sur une erreur réseau (délai dépassé compris) : une requête ratée ne prouve rien
/// sur la cible, on n'en tire aucun constat.
async fn probe_one(client: &reqwest::Client, url: &Url) -> Option<PageRecord> {
    let began = Instant::now();
    let response = client.get(url.clone()).send().await.ok()?;
    let elapsed_ms = began.elapsed().as_millis() as u64;
    let headers: BTreeMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();
    let status = i32::from(response.status().as_u16());
    let content_type = content_type_name(
        status,
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
    );
    Some(PageRecord {
        url: url.to_string(),
        status,
        content_type: content_type.to_string(),
        size: None,
        elapsed_ms,
        headers,
        facts: None,
        probed: true,
    })
}

/// Interroge les cibles de canonical internes que le crawl n'a pas demandées et que le
/// filtre autorise, au plus `budget` requêtes, espacées de `env.min_interval`.
/// Renvoie les lignes ajoutées et le nombre de requêtes faites.
pub async fn probe_canonical_targets(
    env: &ProbeEnv<'_>,
    pages: &[PageRecord],
    budget: usize,
) -> (Vec<PageRecord>, usize) {
    let mut added = Vec::new();
    let mut requests = 0;
    for url in probe_targets(pages, env.start, env.filter)
        .into_iter()
        .take(budget)
    {
        tokio::select! {
            _ = tokio::time::sleep(env.min_interval) => {}
            _ = env.cancel.cancelled() => break,
        }
        requests += 1;
        let record = tokio::select! {
            r = probe_one(env.client, &url) => r,
            _ = env.cancel.cancelled() => break,
        };
        added.extend(record);
    }
    (added, requests)
}

/// Compression d'abord (un constat de site), puis cibles de canonical, puis liens
/// externes (`external_client` à `None` : réglage décoché), dans le budget restant
/// `max_requests - pages crawlées`.
pub async fn run_post_crawl_probes(
    env: &ProbeEnv<'_>,
    pages: &[PageRecord],
    max_requests: usize,
    external_client: Option<&reqwest::Client>,
) -> PostCrawl {
    let mut budget = max_requests.saturating_sub(pages.len());
    let mut content_encoding = None;
    if budget > 0 && !env.cancel.is_cancelled() {
        let sample = reference_page(pages, env.start.as_str());
        if let Some(url) = sample.and_then(|p| Url::parse(&p.url).ok()) {
            budget -= 1;
            tokio::select! {
                _ = tokio::time::sleep(env.min_interval) => {
                    content_encoding = probe_content_encoding(env.client, &url, env.cancel).await;
                }
                _ = env.cancel.cancelled() => {}
            }
        }
    }
    let (probed, made) = probe_canonical_targets(env, pages, budget).await;
    budget = budget.saturating_sub(made);

    let (mut external, mut external_unchecked) = (Vec::new(), 0);
    if let Some(client) = external_client {
        let targets = external_targets(pages, env.start);
        (external, _) = check_external_links(
            client,
            env.start,
            &targets,
            budget,
            env.min_interval,
            env.cancel,
        )
        .await;
        external_unchecked = (targets.len() - external.len()) as u32;
    }
    PostCrawl {
        content_encoding,
        pages: probed,
        external,
        external_unchecked,
    }
}
