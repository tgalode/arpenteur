#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Standard,
    Throttled,
    NoSitemapLine,
    NoSitemap,
    HugeSitemap,
    TruncatedSitemap,
    RobotsDisallow,
}

pub struct FixtureSite {
    pub base: String,
    pub requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureSite {
    pub fn requested(&self, path: &str) -> bool {
        self.requests.lock().unwrap().iter().any(|p| p == path)
    }
}

#[derive(Clone)]
struct AppState {
    base: String,
    variant: Variant,
    requests: Arc<Mutex<Vec<String>>>,
}

pub async fn start_fixture_site(variant: Variant) -> FixtureSite {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let state = AppState {
        base: base.clone(),
        variant,
        requests: requests.clone(),
    };
    let app = Router::new().fallback(handle).with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    FixtureSite { base, requests }
}

fn html(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

fn text(content_type: &'static str, body: String) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

fn redirect(status: StatusCode, to: &str) -> Response {
    (status, [(header::LOCATION, to.to_string())]).into_response()
}

fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("mot{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn urlset(base: &str, paths: &[&str]) -> String {
    let locs: String = paths
        .iter()
        .map(|p| format!("<url><loc>{base}{p}</loc></url>"))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{locs}</urlset>"#
    )
}

fn huge_urlset(base: &str, prefix: &str) -> String {
    let locs: String = (0..30_000)
        .map(|i| format!("<url><loc>{base}/{prefix}/{i}</loc></url>"))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{locs}</urlset>"#
    )
}

async fn handle(State(st): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().to_string();
    st.requests.lock().unwrap().push(path.clone());
    let base = &st.base;

    if st.variant == Variant::Throttled && path != "/" && path != "/robots.txt" {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    match path.as_str() {
        "/robots.txt" if st.variant == Variant::RobotsDisallow => text(
            "text/plain",
            format!("User-agent: *\nDisallow: /private\nSitemap: {base}/sitemap.xml\n"),
        ),
        "/robots.txt" => {
            let sitemap_line = match st.variant {
                Variant::NoSitemapLine | Variant::NoSitemap => String::new(),
                _ => format!("Sitemap: {base}/sitemap.xml\n"),
            };
            text(
                "text/plain",
                format!("User-agent: *\nAllow: /\n{sitemap_line}"),
            )
        }
        "/sitemap.xml" => match st.variant {
            Variant::NoSitemap => StatusCode::NOT_FOUND.into_response(),
            Variant::TruncatedSitemap => text(
                "application/xml",
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{base}/a</loc></url><url><loc>{base}/b</loc></url><url><loc>{base}/c"#
                ),
            ),
            Variant::RobotsDisallow => text(
                "application/xml",
                urlset(base, &["/", "/private/secret", "/doc.pdf", "/orphan"]),
            ),
            Variant::HugeSitemap => text(
                "application/xml",
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><sitemap><loc>{base}/s1.xml</loc></sitemap><sitemap><loc>{base}/s2.xml</loc></sitemap></sitemapindex>"#
                ),
            ),
            _ => text(
                "application/xml",
                urlset(base, &["/", "/a", "/b", "/orphan", "/gone"]),
            ),
        },
        "/s1.xml" if st.variant == Variant::HugeSitemap => {
            text("application/xml", huge_urlset(base, "p1"))
        }
        "/s2.xml" if st.variant == Variant::HugeSitemap => {
            text("application/xml", huge_urlset(base, "p2"))
        }
        "/" => html(format!(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil témoin</title>
<meta name="description" content="Page d'accueil du site témoin">
<link rel="stylesheet" href="/style.css">
<script type="application/ld+json">{{"@context":"https://schema.org","@type":"Organization","name":"Témoin","url":"{base}/"}}</script>
<script src="/app.js"></script></head>
<body><h1>Accueil témoin</h1>
<a href="/a">A</a> <a href="/b">B</a> <a href="/old">Old</a> <a href="/temp">Temp</a> <a href="/missing">Missing</a>
</body></html>"#
        )),
        "/a" => html(format!(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Page A</title>
<link rel="canonical" href="{base}/a"></head>
<body><h1>Page A</h1><p>{}</p><a href="/dup1">d1</a> <a href="/dup2">d2</a></body></html>"#,
            words(250)
        )),
        "/b" => html(format!(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Page B</title>
<link rel="canonical" href="{base}/old">
<script type="application/ld+json">{{</script></head>
<body><h1>Page B</h1><p>{}</p><img src="/img/big.jpg"></body></html>"#,
            words(50)
        )),
        "/old" => redirect(StatusCode::MOVED_PERMANENTLY, "/a"),
        "/temp" => redirect(StatusCode::FOUND, "/b"),
        "/missing" | "/gone" => StatusCode::NOT_FOUND.into_response(),
        "/orphan" => html(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Orpheline</title></head><body><h1>Orpheline</h1><p>Personne ne me lie.</p></body></html>"#
                .to_string(),
        ),
        "/dup1" => html(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Doublon un</title></head><body><h1>Doublon</h1><p>Texte identique sur les deux pages.</p></body></html>"#
                .to_string(),
        ),
        "/dup2" => html(
            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Doublon deux</title></head><body><main><div><h1>Doublon</h1><p>Texte identique sur les deux pages.</p></div></main></body></html>"#
                .to_string(),
        ),
        "/img/big.jpg" => (
            [(header::CONTENT_TYPE, "image/jpeg")],
            vec![0u8; 250_000],
        )
            .into_response(),
        "/app.js" => text("application/javascript", "console.log('x');".to_string()),
        "/style.css" => text("text/css", "body{margin:0}".to_string()),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
