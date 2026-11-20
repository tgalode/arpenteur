use std::collections::{HashSet, VecDeque};
use std::io::Read;
use std::time::Duration;

use quick_xml::Reader;
use quick_xml::events::Event;
use url::Url;

use crate::model::SitemapResult;
use crate::urls::{is_same_site, normalize_url};

const MAX_DEPTH: usize = 2;
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(PartialEq, Eq)]
enum Kind {
    UrlSet,
    Index,
}

struct Fetcher<'a> {
    client: &'a reqwest::Client,
    min_interval: Duration,
    first: bool,
}

enum FetchError {
    NotFound,
    Other(String),
}

impl Fetcher<'_> {
    async fn get(&mut self, url: &Url) -> Result<Vec<u8>, FetchError> {
        if self.first {
            self.first = false;
        } else {
            tokio::time::sleep(self.min_interval).await;
        }
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| FetchError::Other(format!("{url} : {e}")))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(FetchError::NotFound);
        }
        if !status.is_success() {
            return Err(FetchError::Other(format!("{url} : statut {status}")));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| FetchError::Other(format!("{url} : {e}")))?;
        if url.path().ends_with(".gz") {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(&bytes[..])
                .take(MAX_FILE_BYTES)
                .read_to_end(&mut out)
                .map_err(|e| FetchError::Other(format!("{url} : décompression : {e}")))?;
            Ok(out)
        } else {
            Ok(bytes.to_vec())
        }
    }
}

pub async fn discover(
    client: &reqwest::Client,
    start: &Url,
    min_interval: Duration,
    max_urls: usize,
) -> SitemapResult {
    let mut result = SitemapResult {
        found: false,
        sources: Vec::new(),
        urls: Vec::new(),
        truncated: false,
        errors: Vec::new(),
    };
    let mut fetcher = Fetcher {
        client,
        min_interval,
        first: true,
    };
    let origin = start.origin().ascii_serialization();

    let mut declared: Vec<Url> = Vec::new();
    match Url::parse(&format!("{origin}/robots.txt")) {
        Ok(robots_url) => match fetcher.get(&robots_url).await {
            Ok(bytes) => {
                for line in String::from_utf8_lossy(&bytes).lines() {
                    if let Some(value) = sitemap_directive(line) {
                        match robots_url.join(value) {
                            Ok(u) => declared.push(u),
                            Err(e) => result
                                .errors
                                .push(format!("Sitemap invalide {value} : {e}")),
                        }
                    }
                }
            }
            Err(FetchError::NotFound) => {}
            Err(FetchError::Other(e)) => result.errors.push(e),
        },
        Err(e) => result.errors.push(format!("robots.txt : {e}")),
    }

    let fallback = declared.is_empty();
    if fallback && let Ok(u) = Url::parse(&format!("{origin}/sitemap.xml")) {
        declared.push(u);
    }

    let mut seen_sitemaps: HashSet<String> = HashSet::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(Url, usize)> = VecDeque::new();
    for u in declared {
        if !is_same_site(start, &u) {
            result
                .errors
                .push(format!("Sitemap hors du site ignoré : {u}"));
        } else if seen_sitemaps.insert(u.as_str().to_string()) {
            queue.push_back((u, 0));
        }
    }

    while let Some((sitemap_url, depth)) = queue.pop_front() {
        if result.urls.len() >= max_urls {
            result.truncated = true;
            break;
        }
        let bytes = match fetcher.get(&sitemap_url).await {
            Ok(b) => b,
            Err(FetchError::NotFound) if fallback => continue,
            Err(FetchError::NotFound) => {
                result.errors.push(format!("{sitemap_url} : introuvable"));
                continue;
            }
            Err(FetchError::Other(e)) => {
                result.errors.push(e);
                continue;
            }
        };
        let mut children: Vec<String> = Vec::new();
        let mut capped = false;
        let parsed = parse_locs(&bytes, |kind, loc| match kind {
            Kind::Index => {
                children.push(loc);
                true
            }
            Kind::UrlSet => {
                let Some(normalized) = normalize_url(&loc) else {
                    return true;
                };
                let Ok(parsed) = Url::parse(&normalized) else {
                    return true;
                };
                if !is_same_site(start, &parsed) || seen_urls.contains(&normalized) {
                    return true;
                }
                if result.urls.len() >= max_urls {
                    capped = true;
                    return false;
                }
                seen_urls.insert(normalized.clone());
                result.urls.push(normalized);
                true
            }
        });
        match parsed {
            Ok(()) => {
                result.found = true;
                result.sources.push(sitemap_url.to_string());
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("{sitemap_url} : XML invalide : {e}"));
                continue;
            }
        }
        if capped {
            result.truncated = true;
            break;
        }
        if depth < MAX_DEPTH {
            for child in children {
                match sitemap_url.join(&child) {
                    Ok(u) if !is_same_site(start, &u) => {
                        result
                            .errors
                            .push(format!("Sitemap hors du site ignoré : {u}"));
                    }
                    Ok(u) => {
                        if seen_sitemaps.insert(u.as_str().to_string()) {
                            queue.push_back((u, depth + 1));
                        }
                    }
                    Err(e) => result
                        .errors
                        .push(format!("Sitemap invalide {child} : {e}")),
                }
            }
        }
    }
    result
}

fn sitemap_directive(line: &str) -> Option<&str> {
    let (name, value) = line.split_once(':')?;
    if name.trim().eq_ignore_ascii_case("sitemap") {
        let value = value.trim();
        (!value.is_empty()).then_some(value)
    } else {
        None
    }
}

/// Appelle `on_loc` pour chaque `<loc>`; s'arrête quand elle renvoie false.
fn parse_locs(xml: &[u8], mut on_loc: impl FnMut(Kind, String) -> bool) -> Result<(), String> {
    let text = String::from_utf8_lossy(xml);
    let mut reader = Reader::from_str(&text);
    let mut kind: Option<Kind> = None;
    let mut in_loc = false;
    let mut buf = String::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"urlset" if kind.is_none() => kind = Some(Kind::UrlSet),
                    b"sitemapindex" if kind.is_none() => kind = Some(Kind::Index),
                    b"loc" => {
                        in_loc = true;
                        buf.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(t) if in_loc => {
                buf.push_str(&t.decode().map_err(|e| e.to_string())?);
            }
            Event::CData(t) if in_loc => {
                buf.push_str(&t.decode().map_err(|e| e.to_string())?);
            }
            Event::GeneralRef(r) if in_loc => {
                if let Some(c) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                    buf.push(c);
                } else {
                    match r.as_ref() {
                        b"amp" => buf.push('&'),
                        b"lt" => buf.push('<'),
                        b"gt" => buf.push('>'),
                        b"quot" => buf.push('"'),
                        b"apos" => buf.push('\''),
                        _ => {}
                    }
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"loc" => {
                in_loc = false;
                let k = match kind {
                    Some(Kind::UrlSet) => Kind::UrlSet,
                    Some(Kind::Index) => Kind::Index,
                    None => return Err("racine inconnue".to_string()),
                };
                let loc = buf.trim().to_string();
                if !loc.is_empty() && !on_loc(k, loc) {
                    return Ok(());
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if kind.is_none() {
        return Err("ni urlset ni sitemapindex".to_string());
    }
    Ok(())
}
