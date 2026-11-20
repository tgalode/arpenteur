mod support;

use std::time::{Duration, Instant};

use arpenteur_core::sitemap::{discover, discover_with_limit};
use support::{Variant, start_fixture_site};
use url::Url;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Arpenteur-test")
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_sitemap_declared_in_robots() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 5);
    assert!(result.urls.contains(&format!("{}/orphan", site.base)));
    assert_eq!(result.sources, vec![format!("{}/sitemap.xml", site.base)]);
    assert!(!result.truncated);
}

#[tokio::test(flavor = "multi_thread")]
async fn falls_back_to_sitemap_xml() {
    let site = start_fixture_site(Variant::NoSitemapLine).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_sitemap_is_not_found() {
    let site = start_fixture_site(Variant::NoSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(!result.found);
    assert!(result.urls.is_empty());
    assert!(result.sources.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn follows_sitemap_index_and_caps() {
    let site = start_fixture_site(Variant::HugeSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(result.found);
    assert_eq!(result.urls.len(), 10_000);
    assert!(result.truncated);
    assert!(site.requested("/s1.xml"));
    assert!(!site.requested("/s2.xml"));
}

#[tokio::test(flavor = "multi_thread")]
async fn respects_min_interval() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let interval = Duration::from_millis(200);
    let began = Instant::now();
    let result = discover(&client(), &start, interval, 10_000).await;
    let elapsed = began.elapsed();
    assert!(result.found);
    let made = site
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|p| *p == "/robots.txt" || p.ends_with(".xml"))
        .count() as u32;
    assert!(made >= 2);
    assert!(
        elapsed >= interval * (made - 1),
        "{elapsed:?} pour {made} requêtes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn oversize_sitemap_records_error() {
    let site = start_fixture_site(Variant::Standard).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover_with_limit(&client(), &start, Duration::ZERO, 10_000, 120).await;
    assert!(result.urls.len() < 5);
    assert!(result.errors.iter().any(|e| e.contains("dépasse 120")));
}

#[tokio::test(flavor = "multi_thread")]
async fn truncated_sitemap_records_error() {
    let site = start_fixture_site(Variant::TruncatedSitemap).await;
    let start = Url::parse(&format!("{}/", site.base)).unwrap();
    let result = discover(&client(), &start, Duration::ZERO, 10_000).await;
    assert!(!result.found);
    assert!(result.errors.iter().any(|e| e.contains("tronqué")));
}
