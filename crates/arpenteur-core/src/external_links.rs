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
            // Des identifiants dans le lien partiraient en Authorization vers un site tiers.
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
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
    let mut messages = Vec::new();
    let mut source = error.source();
    while let Some(cause) = source {
        messages.push(cause.to_string());
        source = cause.source();
    }
    dns_chain(messages.into_iter())
}

fn dns_chain(messages: impl Iterator<Item = String>) -> bool {
    let mut dns = false;
    for message in messages {
        let message = message.to_lowercase();
        if message == "dns error" {
            dns = true;
        }
        // 11002 : WSATRY_AGAIN sous Windows, dont le texte dit « temporary error ».
        if message.contains("temporary")
            || message.contains("try again")
            || message.contains("11002")
        {
            return false;
        }
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
/// Une cible dont les deux essais échouent sur le DNS est mise de côté : elle est « cassée »
/// seulement si une autre cible a reçu une réponse HTTP pendant ce run (preuve que les noms
/// externes se résolvent), sinon « inconnue ».
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
    let mut answered = false;
    let mut vanished = Vec::new();
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
                vanished.push(checks.len());
            }
        }
        checks.push(match result {
            Attempt::Answered(status, last) => {
                answered = true;
                check_of(url, classify_status(status), Some(status), Some(last))
            }
            Attempt::DnsFailure | Attempt::Failed => {
                check_of(url, ExternalOutcome::Unknown, None, None)
            }
            Attempt::Cancelled => break,
        });
    }
    if answered {
        for index in vanished {
            checks[index].outcome = ExternalOutcome::Broken;
        }
    }
    (checks, used)
}

#[cfg(test)]
mod tests {
    use super::dns_chain;

    fn chain(messages: &[&str]) -> bool {
        dns_chain(messages.iter().map(|m| m.to_string()))
    }

    #[test]
    fn dns_chain_needs_a_dns_error_cause() {
        assert!(chain(&[
            "error sending request",
            "client error (Connect)",
            "dns error",
            "error resolving DNS"
        ]));
        assert!(!chain(&[
            "error sending request",
            "client error (Connect)",
            "tcp connect error"
        ]));
        assert!(!chain(&["operation timed out"]));
    }

    #[test]
    fn temporary_resolver_failures_are_not_dns_failures() {
        assert!(!chain(&[
            "dns error",
            "failed to lookup address information: Temporary failure in name resolution"
        ]));
        assert!(!chain(&["dns error", "Try again later"]));
        assert!(!chain(&[
            "dns error",
            "This is usually a temporary error during hostname resolution (os error 11002)"
        ]));
        assert!(!chain(&["dns error", "lookup failed (os error 11002)"]));
    }
}
