#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
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
    /// Chaque page lente (~300 ms) et liée à `LINKED_PAGES` pages.
    Slow,
    /// Comme `Slow`, avec un sitemap qui déclare `/` et `/p0`.
    SlowWithSitemap,
    /// `/x` a un canonical vers `/canon-target`, qui redirige en 301 et n'est liée nulle part.
    CanonicalOrphanTarget,
    /// `/` répond `content-encoding: br` à la requête de sonde seulement.
    BrotliOnProbe,
    /// Comme `CanonicalOrphanTarget`, mais `/canon-target` accepte la connexion et ne répond jamais.
    CanonicalTargetHangs,
    /// `/x` a un canonical vers `/prive/cible`, que robots.txt interdit.
    CanonicalToDisallowed,
    /// `/x` a un canonical vers `/tag/cible`, qui répond 200 (à exclure par un motif).
    CanonicalToTag,
    /// `/x` a un canonical vers `/canon-target`, qui répond 200 en `application/pdf`.
    CanonicalToPdf,
    /// `/` lie `/p2` et dix routes `/ext-*` sur `localhost` (donc externes : le site écoute sur
    /// `127.0.0.1`) ; `/p2` relie `/ext-404`.
    ExternalLinks,
    /// Les adresses inexistantes répondent 200 avec la page d'erreur du site.
    UnknownAnswers200,
    /// Les adresses inexistantes sont redirigées en 302 vers l'accueil.
    UnknownRedirectsHome,
    /// Les adresses inexistantes sont redirigées vers `/connexion?state=abc#x` (200).
    UnknownRedirectsToLogin,
    /// Vraie 404 pour les adresses inexistantes, mais `/fantome` (200) a le title et le H1 de la
    /// page d'erreur ; `/article-404` parle des erreurs 404 sans les partager.
    GhostErrorPage,
    /// `/robots.txt` répond 404, 410 ou 500 ; le reste est le site standard.
    Robots404,
    Robots410,
    Robots500,
    /// robots.txt interdit l'adresse que la sonde d'adresse inexistante demande.
    RobotsBlocksNotFound,
    /// L'adresse inexistante accepte la connexion et ne répond jamais.
    NotFoundHangs,
    /// L'adresse inexistante répond 200 avec un corps de 3 Mo ; title et H1 sont dans le premier Ko.
    HugeNotFoundBody,
}

/// Début de l'adresse inexistante demandée après le crawl.
pub const NOT_FOUND_PREFIX: &str = "/arpenteur-introuvable-";

const ERROR_PAGE: &str = r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Page introuvable - Témoin</title></head><body><h1>Oups, page introuvable</h1><p>Cette page n'existe pas.</p></body></html>"#;

/// `Accept-Encoding` envoyé par la sonde de compression.
pub const PROBE_ACCEPT_ENCODING: &str = "gzip, deflate, br";

/// Pages liées depuis "/" par `Throttled` (toutes en 429) et par chaque page de `Slow`.
pub const LINKED_PAGES: usize = 25;

pub struct FixtureSite {
    pub base: String,
    pub requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureSite {
    pub fn requested(&self, path: &str) -> bool {
        self.count(path) > 0
    }

    /// Requêtes dont le chemin commence par `prefix`.
    pub fn count_prefix(&self, prefix: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with(prefix))
            .count()
    }

    pub fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == path)
            .count()
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

fn many_links_page() -> Response {
    let links: String = (0..LINKED_PAGES)
        .map(|i| format!(r#"<a href="/p{i}">p{i}</a> "#))
        .collect();
    html(format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Liens</title></head><body><h1>Liens</h1>{links}</body></html>"#
    ))
}

async fn handle(State(st): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let path = uri.path().to_string();
    st.requests.lock().unwrap().push(path.clone());
    let base = &st.base;

    if st.variant == Variant::BrotliOnProbe
        && path == "/"
        && headers
            .get(header::ACCEPT_ENCODING)
            .is_some_and(|v| v == PROBE_ACCEPT_ENCODING)
    {
        return (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CONTENT_ENCODING, "br"),
            ],
            "br",
        )
            .into_response();
    }
    if st.variant == Variant::UnknownRedirectsToLogin && path == "/connexion" {
        return html(ERROR_PAGE.to_string());
    }
    if path.starts_with(NOT_FOUND_PREFIX) {
        match st.variant {
            Variant::UnknownAnswers200 => return html(ERROR_PAGE.to_string()),
            Variant::UnknownRedirectsHome => return redirect(StatusCode::FOUND, "/"),
            Variant::UnknownRedirectsToLogin => {
                return redirect(StatusCode::FOUND, "/connexion?state=abc#x");
            }
            Variant::GhostErrorPage => {
                return (
                    StatusCode::NOT_FOUND,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    ERROR_PAGE,
                )
                    .into_response();
            }
            Variant::HugeNotFoundBody => {
                let head = r#"<!doctype html><html><head><title>Gros corps</title></head><body><h1>Gros H1</h1><p>"#;
                return html(format!(
                    "{head}{}</p></body></html>",
                    "mot ".repeat(750_000)
                ));
            }
            Variant::NotFoundHangs => {
                std::future::pending::<()>().await;
            }
            _ => {}
        }
    }
    if st.variant == Variant::GhostErrorPage {
        return match path.as_str() {
            "/robots.txt" => text("text/plain", "User-agent: *\nAllow: /\n".to_string()),
            "/" => html(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil</title></head><body><h1>Accueil</h1><a href="/fantome">f</a> <a href="/article-404">a</a></body></html>"#
                    .to_string(),
            ),
            "/fantome" => html(ERROR_PAGE.to_string()),
            "/article-404" => html(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Page introuvable - Témoin</title></head><body><h1>Comprendre l'erreur 404</h1><p>Un article sur les pages introuvables.</p></body></html>"#
                    .to_string(),
            ),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if st.variant == Variant::RobotsBlocksNotFound && path == "/robots.txt" {
        return text(
            "text/plain",
            "User-agent: *\nDisallow: /arpenteur-introuvable\n".to_string(),
        );
    }
    if path == "/robots.txt" {
        match st.variant {
            Variant::Robots404 => return StatusCode::NOT_FOUND.into_response(),
            Variant::Robots410 => return StatusCode::GONE.into_response(),
            Variant::Robots500 => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            _ => {}
        }
    }
    if st.variant == Variant::ExternalLinks {
        let port = base.rsplit(':').next().unwrap_or_default().to_string();
        let ext = |name: &str| format!(r#"<a href="http://localhost:{port}/{name}">{name}</a> "#);
        return match path.as_str() {
            "/robots.txt" => text("text/plain", "User-agent: *\nAllow: /\n".to_string()),
            "/" => {
                let links: String = [
                    "ext-ok",
                    "ext-404",
                    "ext-410",
                    "ext-403",
                    "ext-429",
                    "ext-999",
                    "ext-500",
                    "ext-redirect",
                    "ext-loop",
                    "ext-hang",
                ]
                .iter()
                .map(|n| ext(n))
                .collect();
                html(format!(
                    r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil</title></head><body><h1>Accueil</h1><a href="/p2">p2</a> {links}</body></html>"#
                ))
            }
            "/p2" => html(format!(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>P2</title></head><body><h1>P2</h1>{}</body></html>"#,
                ext("ext-404")
            )),
            "/ext-ok" => html("<!doctype html><title>ok</title>".to_string()),
            "/ext-404" => StatusCode::NOT_FOUND.into_response(),
            "/ext-410" => StatusCode::GONE.into_response(),
            "/ext-403" => StatusCode::FORBIDDEN.into_response(),
            "/ext-429" => StatusCode::TOO_MANY_REQUESTS.into_response(),
            "/ext-999" => StatusCode::from_u16(999).unwrap().into_response(),
            "/ext-500" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            "/ext-redirect" => redirect(StatusCode::FOUND, "/ext-404"),
            "/ext-loop" => redirect(StatusCode::FOUND, "/ext-loop"),
            "/ext-hang" => {
                std::future::pending::<()>().await;
                unreachable!()
            }
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if matches!(
        st.variant,
        Variant::CanonicalToDisallowed | Variant::CanonicalToTag | Variant::CanonicalToPdf
    ) {
        let target = match st.variant {
            Variant::CanonicalToDisallowed => "/prive/cible",
            Variant::CanonicalToTag => "/tag/cible",
            _ => "/canon-target",
        };
        return match path.as_str() {
            "/robots.txt" => text(
                "text/plain",
                "User-agent: *\nDisallow: /prive\n".to_string(),
            ),
            "/" => html(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil</title></head><body><h1>Accueil</h1><a href="/x">x</a></body></html>"#
                    .to_string(),
            ),
            "/x" => html(format!(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>X</title><link rel="canonical" href="{base}{target}"></head><body><h1>X</h1></body></html>"#
            )),
            p if p == target && st.variant == Variant::CanonicalToPdf => {
                ([(header::CONTENT_TYPE, "application/pdf")], "%PDF-1.4").into_response()
            }
            p if p == target => html("<!doctype html><title>Cible</title>".to_string()),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    if matches!(
        st.variant,
        Variant::CanonicalOrphanTarget | Variant::CanonicalTargetHangs
    ) {
        if st.variant == Variant::CanonicalTargetHangs && path == "/canon-target" {
            std::future::pending::<()>().await;
        }
        return match path.as_str() {
            "/robots.txt" => text("text/plain", "User-agent: *\nAllow: /\n".to_string()),
            "/" => html(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil</title></head><body><h1>Accueil</h1><a href="/x">x</a></body></html>"#
                    .to_string(),
            ),
            "/x" => html(format!(
                r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>X</title><link rel="canonical" href="{base}/canon-target"></head><body><h1>X</h1></body></html>"#
            )),
            "/canon-target" => redirect(StatusCode::MOVED_PERMANENTLY, "/x"),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }

    if st.variant == Variant::Throttled && path == "/" {
        return many_links_page();
    }
    if st.variant == Variant::Throttled && path != "/robots.txt" {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    if matches!(st.variant, Variant::Slow | Variant::SlowWithSitemap) {
        let with_sitemap = st.variant == Variant::SlowWithSitemap;
        return match path.as_str() {
            "/robots.txt" if with_sitemap => text(
                "text/plain",
                format!("User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n"),
            ),
            "/robots.txt" => text("text/plain", "User-agent: *\nAllow: /\n".to_string()),
            "/sitemap.xml" if with_sitemap => text("application/xml", urlset(base, &["/", "/p0"])),
            "/sitemap.xml" => StatusCode::NOT_FOUND.into_response(),
            _ => {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                many_links_page()
            }
        };
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
