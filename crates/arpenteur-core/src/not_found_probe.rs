//! Sonde de la page d'erreur : une requête vers une adresse qui ne peut pas exister montre ce
//! que le site répond à une page absente.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::CONTENT_TYPE;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::html::extract_page;
use crate::model::NotFoundProbe;

/// Taille maximale du corps lu.
pub const NOT_FOUND_BODY_LIMIT: usize = 1_048_576;

/// 16 caractères hexadécimaux, différents à chaque appel.
pub fn random_token() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    format!("{:016x}", hasher.finish())
}

/// `<origine>/arpenteur-introuvable-<jeton>`.
pub fn not_found_url(start: &Url, token: &str) -> Url {
    let origin = start.origin().ascii_serialization();
    Url::parse(&format!("{origin}/arpenteur-introuvable-{token}"))
        .expect("origine et jeton hexadécimal forment une adresse valide")
}

/// `None` sur une erreur réseau, un délai dépassé, trop de redirections ou une annulation :
/// une requête ratée ne dit rien de ce que le site répond.
/// `client` suit les redirections (cinq au plus) et décompresse lui-même les corps.
pub async fn probe_not_found(
    client: &reqwest::Client,
    url: &Url,
    cancel: &CancellationToken,
) -> Option<NotFoundProbe> {
    if cancel.is_cancelled() {
        return None;
    }
    let read = async {
        let mut response = client.get(url.clone()).send().await.ok()?;
        let status = response.status().as_u16();
        let final_url = response.url().clone();
        let html = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.to_ascii_lowercase().contains("text/html"));
        let mut body: Vec<u8> = Vec::new();
        while body.len() < NOT_FOUND_BODY_LIMIT {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    let room = NOT_FOUND_BODY_LIMIT - body.len();
                    body.extend_from_slice(&chunk[..chunk.len().min(room)]);
                }
                Ok(None) => break,
                Err(_) => return None,
            }
        }
        Some((status, final_url, html, body))
    };
    let (status, final_url, is_html, body) = tokio::select! {
        r = read => r?,
        _ = cancel.cancelled() => return None,
    };
    let (mut title, mut h1, mut text_hash) = (None, None, String::new());
    if is_html {
        let facts = extract_page(&String::from_utf8_lossy(&body), &final_url);
        title = facts.title;
        h1 = facts.h1.into_iter().next();
        if facts.word_count > 0 {
            text_hash = facts.text_hash;
        }
    }
    Some(NotFoundProbe {
        status,
        redirected: final_url != *url,
        final_url: final_url.to_string(),
        title,
        h1,
        text_hash,
    })
}
