//! Vérification, après le crawl, des liens `<a href>` qui sortent du site : une requête
//! par adresse distincte, le robots.txt des sites tiers n'est pas lu.

use std::collections::HashSet;
use std::error::Error as _;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use url::Url;

use crate::model::{ExternalCheck, ExternalOutcome, PageRecord};
use crate::urls::{is_same_site, normalize_url};

/// Redirections suivies avant de renoncer (le lien est alors « inconnu »).
pub const MAX_REDIRECTS: usize = 5;

const EXTERNAL_TIMEOUT: Duration = Duration::from_secs(5);

/// Adresses http(s) distinctes (normalisées) des liens des pages internes qui sortent du site,
/// dans l'ordre de rencontre.
pub fn external_targets(pages: &[PageRecord], start: &Url) -> Vec<Url> {
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for page in pages {
        let Some(facts) = page.facts.as_ref() else {
            continue;
        };
        if !Url::parse(&page.url).is_ok_and(|u| is_same_site(start, &u)) {
            continue;
        }
        for link in &facts.links {
            let Some(normalized) = normalize_url(&link.href) else {
                continue;
            };
            let Ok(url) = Url::parse(&normalized) else {
                continue;
            };
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && !is_same_site(start, &url)
                && seen.insert(normalized)
            {
                targets.push(url);
            }
        }
    }
    targets
}

pub fn external_client(user_agent: &str) -> Result<reqwest::Client, reqwest::Error> {
    external_client_with_timeout(user_agent, EXTERNAL_TIMEOUT)
}

pub fn external_client_with_timeout(
    user_agent: &str,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .user_agent(user_agent)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
}

/// Cassé : 404 et 410 seulement. 401, 403, 429, 999 et les 5xx sont des refus ou des pannes
/// passagères, pas la preuve que la page n'existe plus.
pub fn classify_status(status: u16) -> ExternalOutcome {
    match status {
        404 | 410 => ExternalOutcome::Broken,
        s if s < 400 => ExternalOutcome::Ok,
        _ => ExternalOutcome::Unknown,
    }
}

/// Échec de résolution du nom de domaine : hyper-util le signale par « dns error » dans la
/// chaîne des causes. Une résolution temporairement indisponible (EAI_AGAIN) ne prouve rien.
fn is_dns_failure(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    let mut dns = false;
    while let Some(cause) = source {
        let message = cause.to_string().to_lowercase();
        if message == "dns error" {
            dns = true;
        }
        if message.contains("temporary failure") || message.contains("try again") {
            return false;
        }
        source = cause.source();
    }
    dns
}

enum Attempt {
    Answered(u16, Url),
    DnsFailure,
    Failed,
    Cancelled,
}

async fn attempt(client: &reqwest::Client, url: &Url, cancel: &CancellationToken) -> Attempt {
    // Le corps n'est pas lu : la réponse est lâchée dès les en-têtes reçus.
    let result = tokio::select! {
        r = client.get(url.clone()).send() => r,
        _ = cancel.cancelled() => return Attempt::Cancelled,
    };
    match result {
        Ok(response) => Attempt::Answered(response.status().as_u16(), response.url().clone()),
        Err(e) if is_dns_failure(&e) => Attempt::DnsFailure,
        Err(_) => Attempt::Failed,
    }
}

/// `false` si le crawl a été annulé pendant l'attente.
async fn wait_turn(interval: Duration, cancel: &CancellationToken) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(interval) => true,
        _ = cancel.cancelled() => false,
    }
}

fn check_of(
    url: &Url,
    outcome: ExternalOutcome,
    status: Option<u16>,
    last: Option<Url>,
) -> ExternalCheck {
    ExternalCheck {
        url: url.to_string(),
        outcome,
        status,
        final_url: last.filter(|l| l != url).map(|l| l.to_string()),
    }
}

/// Renvoie les vérifications faites et le nombre de requêtes consommées (au plus `budget`).
/// Une cible absente du résultat n'a pas été vérifiée (budget épuisé ou annulation).
pub async fn check_external_links(
    client: &reqwest::Client,
    targets: &[Url],
    budget: usize,
    min_interval: Duration,
    cancel: &CancellationToken,
) -> (Vec<ExternalCheck>, usize) {
    let mut checks = Vec::new();
    let mut used = 0;
    for url in targets {
        if used >= budget || !wait_turn(min_interval, cancel).await {
            break;
        }
        used += 1;
        let mut result = attempt(client, url, cancel).await;
        if matches!(result, Attempt::DnsFailure) && used < budget {
            if !wait_turn(min_interval, cancel).await {
                break;
            }
            used += 1;
            result = attempt(client, url, cancel).await;
            if matches!(result, Attempt::DnsFailure) {
                checks.push(check_of(url, ExternalOutcome::Broken, None, None));
                continue;
            }
        }
        checks.push(match result {
            Attempt::Answered(status, last) => {
                check_of(url, classify_status(status), Some(status), Some(last))
            }
            Attempt::DnsFailure | Attempt::Failed => {
                check_of(url, ExternalOutcome::Unknown, None, None)
            }
            Attempt::Cancelled => break,
        });
    }
    (checks, used)
}
