# Arpenteur, plan 1 : moteur et coquille (jalons 0 à 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** une app desktop Tauri qui crawle un site avec le moteur SiteOne, y branche nos analyses, montre la progression, permet l'arrêt, enregistre chaque crawl sur le disque et affiche un tableau des URL enrichi (profondeur, liens entrants, canonical, indexabilité).

**Architecture:** un crate Rust `arpenteur-core`, sans Tauri, porte tout le métier : il construit les options de SiteOne, enregistre les analyses intégrées de SiteOne plus notre `PageCollector`, lance `Manager::run()` et range les résultats. Les liens, la profondeur et nos contrôles sont des fonctions pures, recalculées à l'ouverture d'un crawl. L'app Tauri (`src-tauri`) n'est qu'une coquille : commandes, état, événements. L'interface React lit un `CrawlResult` déjà assemblé.

**Tech Stack:** Rust édition 2024 (rustc ≥ 1.94), `siteone-crawler =2.5.1`, Tauri 2, React 19 + TypeScript + Vite, `scraper 0.27`, `quick-xml 0.40`, `reqwest 0.13`, `tokio`, `tokio-util` (`CancellationToken`), `chrono 0.4`, `sha2`, `flate2`, `axum 0.8` (tests), Vitest + Testing Library, `@tanstack/react-virtual`.

**Spec:** `docs/superpowers/specs/2026-11-20-arpenteur-design.md`

**Hors de ce plan (plan 2, jalons 3 à 5) :** fusion avec le JSON de SiteOne, catalogue des problèmes (brouillon à valider d'abord), panneau Problèmes, export CSV, écran Réglages, écran À propos avec la licence de SiteOne, installeurs et notice d'installation. Le plan 2 s'écrit après la validation du catalogue.

## Global Constraints

- Dépendance : `siteone-crawler = { version = "=2.5.1", default-features = false }` (pas de `browser`, pas de rendu JavaScript).
- Pas de fork de SiteOne. Jamais `Initiator::new`, jamais `--help` ni `--version` dans les arguments : ces chemins appellent `std::process::exit`. Les options passent par `siteone_crawler::options::core_options::parse_argv`.
- Réglages par défaut : 1 connexion, 2 requêtes par seconde, 10 000 requêtes au plus, robots.txt respecté, User-Agent `Arpenteur/<version>!` (le `!` retire la signature SiteOne), seuil de limitation 5.
- Seuls les pages HTML et les images sont téléchargées. JS, CSS, polices et fichiers sont exclus par `--ignore-regex`, jamais par les options `--disable-*`, qui retouchent le HTML.
- La file d'attente de SiteOne (`--max-queue-length`) est égale à la limite de requêtes. Le cache HTTP de SiteOne est désactivé (`--http-cache-dir=off`) : chaque crawl reflète le site du moment.
- Stockage : `<app_data_dir>/sites/<domaine>/<AAAA-MM-JJ_HH-MM-SS>/` avec `crawl.json`, `arpenteur.json`, `siteone.json`, `siteone-report.html`. Pas de base de données.
- Seuils : image lourde au-delà de 204 800 octets (200 Ko) ; contenu court sous 200 mots ; doublon = même empreinte du texte visible normalisé.
- Un seul crawl à la fois.
- Cibles : macOS Apple Silicon et Windows x64.
- Interface en français, tous les textes dans `src/i18n/fr.ts`.
- Les types sérialisés vers l'interface utilisent `#[serde(rename_all = "camelCase")]`.

## Review Focus

1. **URL saisie sans schéma ou avec des espaces** (`exemple.fr`, ` https://exemple.fr/x `) : l'app ajoute `https://` et retire les espaces. Une saisie qui n'est pas une URL (`pas une url`, `ftp://x.fr`) est refusée avec un message, sans crawl. Test dans la tâche 8.
2. **Un accueil qui redirige vers un autre hôte** (`exemple.fr` vers `www.exemple.fr`, le cas de crocweb.fr) : les deux hôtes sont le même site. Un canonical vers l'hôte sans `www` qui redirige donne `canonicalToRedirect`, pas `canonicalToOtherHost`. Test dans la tâche 10.
3. **HTML piégeux** (canonical relatif, `<base href>`, `REL="Canonical"`, deux `<title>`, page en Latin-1) : extraction sans panique, canonical résolu en URL absolue. Tests dans la tâche 3.
4. **Sitemap énorme** (index de 60 000 URL) : la découverte s'arrête à la limite de requêtes, le signale, et respecte le débit. Test dans la tâche 6.
5. **Double clic sur « Lancer »** ou lancement pendant un crawl : le second est refusé avec un message, le premier continue. Test dans la tâche 11.

## Fichiers

```
arpenteur/
  Cargo.toml                         workspace : crates/arpenteur-core, src-tauri
  crates/arpenteur-core/
    src/lib.rs                       déclare et réexporte les modules
    src/model.rs                     tous les types de données partagés
    src/urls.rs                      normalisation et « même site »
    src/html.rs                      HTML brut → PageFacts
    src/limit_guard.rs               détection d'un site qui limite
    src/collector.rs                 PageCollector (Analyzer SiteOne) et CrawlEvents
    src/sitemap.rs                   découverte des URL du sitemap
    src/storage.rs                   CrawlStore : dossiers et fichiers JSON
    src/settings.rs                  URL de départ, argv et CoreOptions
    src/engine.rs                    analyses intégrées et run_crawl
    src/links.rs                     liens entrants, profondeur, chaînes de redirection
    src/checks.rs                    nos contrôles, lignes du tableau, assemblage
    tests/support/mod.rs             site témoin axum
    tests/smoke.rs                   vérifications du jalon 0
    tests/crawl.rs                   crawls de bout en bout
  src-tauri/src/{main.rs, lib.rs, state.rs, events.rs, commands.rs}
  src/{main.tsx, App.tsx}
  src/lib/{types.ts, api.ts, table.ts, table.test.ts}
  src/i18n/{fr.ts, fr.test.ts}
  src/screens/{Home.tsx, Home.test.tsx, Crawl.tsx, Crawl.test.tsx}
  src/components/{UrlTable.tsx, UrlDetail.tsx}
  .github/workflows/ci.yml
```

---

### Task 1 : workspace, site témoin et vérifications du jalon 0

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `crates/arpenteur-core/Cargo.toml`, `crates/arpenteur-core/src/lib.rs`, `crates/arpenteur-core/src/engine.rs`, `crates/arpenteur-core/src/settings.rs`
- Test: `crates/arpenteur-core/tests/support/mod.rs`, `crates/arpenteur-core/tests/smoke.rs`

**Interfaces:**
- Produces : `engine::register_builtin_analyzers(am: &mut AnalysisManager, options: &CoreOptions)`, `settings::ASSET_IGNORE_REGEX: &str`, et dans `tests/support` : `start_fixture_site(variant: Variant) -> FixtureSite` avec `FixtureSite { base: String, requests: Arc<Mutex<Vec<String>>> }`, `FixtureSite::requested(&self, path: &str) -> bool`, `enum Variant { Standard, Throttled, NoSitemapLine, NoSitemap, HugeSitemap }`.

- [ ] **Step 1 : initialiser le dépôt**

Run: `cd ~/Documents/Repositories/arpenteur && git init -b main`. `.gitignore` : `target/`, `node_modules/`, `dist/`, `src-tauri/gen/`.

- [ ] **Step 2 : créer le workspace et le crate**

Workspace avec `members = ["crates/arpenteur-core"]` (`src-tauri` s'ajoutera à la tâche 11). Dépendances du crate : celles du Tech Stack. En dev-dependencies : `axum`, `tokio` (features `full`).

- [ ] **Step 3 : écrire le site témoin `tests/support/mod.rs`**

Un serveur axum sur `127.0.0.1:0` qui journalise chaque chemin demandé dans `requests`. Variante `Standard` :

| Chemin | Réponse |
|---|---|
| `/robots.txt` | `User-agent: *`, `Allow: /`, `Sitemap: {base}/sitemap.xml` |
| `/sitemap.xml` | urlset : `/`, `/a`, `/b`, `/orphan`, `/gone` |
| `/` | title « Accueil témoin », un JSON-LD `Organization` valide, liens vers `/a`, `/b`, `/old`, `/temp`, `/missing`, plus `<script src="/app.js">` et `<link rel="stylesheet" href="/style.css">` |
| `/a` | canonical vers lui-même, 250 mots, liens vers `/dup1` et `/dup2` |
| `/b` | canonical vers `/old`, JSON-LD `{` invalide, `<img src="/img/big.jpg">` sans alt, 50 mots |
| `/old` | 301 vers `/a` |
| `/temp` | 302 vers `/b` |
| `/missing`, `/gone` | 404 |
| `/orphan` | 200, aucune page n'y mène |
| `/dup1`, `/dup2` | même texte visible, balisage différent |
| `/img/big.jpg` | 250 000 octets, `image/jpeg` |
| `/app.js`, `/style.css` | 200 (ne doivent jamais être demandés) |

Autres variantes :
- `Throttled` : `/` et `/robots.txt` comme `Standard`, tout le reste en 429.
- `NoSitemapLine` : `robots.txt` sans ligne `Sitemap`, `/sitemap.xml` toujours servi.
- `NoSitemap` : `robots.txt` sans ligne `Sitemap`, `/sitemap.xml` en 404.
- `HugeSitemap` : `/sitemap.xml` est un index vers `/s1.xml` et `/s2.xml`, de 30 000 `<loc>` chacun, générés à la volée.

- [ ] **Step 4 : écrire les tests du jalon 0 dans `tests/smoke.rs`**

Chaque test construit l'argv à la main (`--url={base}/`, `--workers=2`, `--max-reqs-per-sec=50`, `--http-cache-dir=off`, `--output-json-file={tmp}/siteone.json`, `--output-html-report=`, `--output-text-file=`, `--no-color`), appelle `parse_argv`, construit un `AnalysisManager` avec `register_builtin_analyzers`, ajoute une analyse de test `BodyProbe` qui garde `(url, body)`, appelle `auto_activate_analyzers()`, puis lance `Manager::new(options, am)?.run()`.

```rust
#[tokio::test] async fn body_given_to_analyzers_is_raw_html() {
    // body de "/" capté par BodyProbe
    assert!(body.contains("<title>Accueil témoin</title>"));
    assert!(body.contains(r#"<script type="application/ld+json">"#));
}
#[tokio::test] async fn asset_regex_prevents_js_and_css_requests() {
    // argv + format!("--ignore-regex={}", ASSET_IGNORE_REGEX)
    assert!(!site.requested("/app.js"));
    assert!(!site.requested("/style.css"));
    assert!(site.requested("/img/big.jpg"));
}
#[tokio::test] async fn url_list_seeds_unlinked_pages() {
    // options.url_list_urls = vec![format!("{}/orphan", site.base)] après parse_argv
    assert!(site.requested("/orphan"));
}
```

- [ ] **Step 5 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core --test smoke`
Expected : erreur de compilation, `register_builtin_analyzers` et `ASSET_IGNORE_REGEX` n'existent pas.

- [ ] **Step 6 : implémenter `register_builtin_analyzers` et `ASSET_IGNORE_REGEX`**

`register_builtin_analyzers` recopie exactement `Initiator::register_analyzers` de la 2.5.1 (`src/engine/initiator.rs`, de `AccessibilityAnalyzer` à `SslTlsAnalyzer`, avec leurs `set_config`). Le commentaire de tête de la fonction indique la version recopiée.

```rust
pub const ASSET_IGNORE_REGEX: &str =
    r"(?i)\.(js|mjs|css|woff2?|ttf|otf|eot|pdf|zip|rar|7z|docx?|xlsx?|pptx?|mp3|mp4|webm)(\?|#|$)";
```

- [ ] **Step 7 : vérifier que les tests passent**

Run: `cargo test -p arpenteur-core --test smoke`
Expected : 3 tests PASS. Si `asset_regex_prevents_js_and_css_requests` échoue parce que SiteOne n'applique pas `--ignore-regex` aux ressources, s'arrêter et le signaler : la règle D8 est à revoir avant la suite.

- [ ] **Step 8 : décider comment lancer le moteur**

Ajouter `fn assert_send<T: Send>(_: &T) {}` dans un test et l'appeler sur le futur de `manager.run()`. S'il compile, `run_crawl` (tâche 8) fera `tokio::spawn`. Sinon, elle lancera le moteur dans un `std::thread` avec un runtime `new_current_thread`. Consigner le résultat dans le commentaire de tête de `engine.rs`.

- [ ] **Step 9 : commit**

```bash
git add -A && git commit -m "feat(core): workspace, site témoin et vérifications du jalon 0"
```

### Task 2 : CI sur macOS et Windows

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1 : demander à l'utilisateur de créer le dépôt GitHub privé `arpenteur`**, ou son accord pour `gh repo create arpenteur --private --source . --remote origin`. Ne rien créer ni pousser sans son accord.

- [ ] **Step 2 : écrire le workflow**

Sur `push` et `pull_request`, matrice `os: [macos-latest, windows-latest]` : `actions/checkout`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, puis `cargo test -p arpenteur-core`.

- [ ] **Step 3 : pousser avec l'accord de l'utilisateur et vérifier**

Run: `git push -u origin main`, puis `gh run watch`
Expected : les deux jobs sont verts. Un échec sous Windows est un constat du jalon 0 : le signaler avant de continuer.

- [ ] **Step 4 : commit**

```bash
git add .github/workflows/ci.yml && git commit -m "ci: tests du cœur sur macOS et Windows"
```

### Task 3 : types partagés, URL et extraction HTML

**Files:**
- Create: `crates/arpenteur-core/src/model.rs`, `src/urls.rs`, `src/html.rs`
- Test: tests unitaires dans `src/urls.rs` et `src/html.rs`

**Interfaces:**
- Produces (dans `model.rs`, tous `Serialize + Deserialize + Clone + Debug`, en camelCase ; `CrawlStatus` et `FindingCode` aussi `PartialEq + Eq`) :
  - `CrawlSettings { workers: u32, max_reqs_per_sec: f64, max_requests: u32, respect_robots: bool, user_agent: Option<String>, include_regex: Vec<String>, ignore_regex: Vec<String>, limit_threshold: u32 }`, avec `#[serde(default)]` et un `Default` qui porte les valeurs des Global Constraints.
  - `CrawlStatus`, en `#[serde(tag = "kind", rename_all = "camelCase")]` : `Running | Completed | Stopped | Throttled | Failed { message: String }`.
  - `Progress { done: u32, last_url: String, last_status: i32, elapsed_ms: u64 }`.
  - `PageRecord { url: String, status: i32, content_type: String, size: Option<i64>, elapsed_ms: u64, headers: BTreeMap<String, String>, facts: Option<PageFacts> }`, avec `location(&self) -> Option<&str>`, `x_robots_tag(&self) -> Option<&str>` et `is_html(&self) -> bool`. Les en-têtes sont en minuscules.
  - `PageFacts { title: Option<String>, meta_description: Option<String>, h1: Vec<String>, canonical: Option<String>, meta_robots: Option<String>, links: Vec<Link>, images: Vec<Image>, json_ld: Vec<JsonLd>, word_count: u32, text_hash: String }`.
  - `Link { href: String, anchor: String, nofollow: bool }`, `Image { src: String, alt: Option<String> }`, `JsonLd { valid: bool, types: Vec<String> }`.
  - `SitemapResult { found: bool, sources: Vec<String>, urls: Vec<String>, truncated: bool, errors: Vec<String> }`.
  - `InLink { from: String, anchor: String, nofollow: bool }`, `LinkReport { inlinks: BTreeMap<String, Vec<InLink>>, depth: BTreeMap<String, u32>, redirect_chains: BTreeMap<String, Vec<String>> }`.
  - `FindingCode`, variantes Rust en PascalCase (`FindingCode::CanonicalToRedirect`) sérialisées en camelCase : `canonicalMissing, canonicalToOtherUrl, canonicalToRedirect, canonicalToError, canonicalToOtherHost, temporaryRedirect, redirectChain, brokenInternalLink, imageTooHeavy, jsonLdMissing, jsonLdInvalid, orphanPage, notInSitemap, sitemapUrlError, sitemapUrlNonIndexable, sitemapNotFound, duplicateContent, thinContent, noindex`.
  - `Finding { code: FindingCode, url: String, related: Vec<String> }`.
  - `UrlRow { url: String, status: i32, content_type: String, title: Option<String>, meta_description: Option<String>, h1: Option<String>, h1_count: u32, canonical: Option<String>, indexable: bool, depth: Option<u32>, inlinks: u32, word_count: Option<u32>, size: Option<i64> }`.
  - `CrawlLocation { domain: String, id: String, dir: PathBuf }`, `CrawlMeta { id, domain, start_url: String, started_at: String, finished_at: Option<String>, status: CrawlStatus, settings: CrawlSettings, pages: u32 }` (dates en RFC 3339).
  - `CollectedData { pages: Vec<PageRecord>, sitemap: SitemapResult }`, `SiteSummary { domain: String, crawls: Vec<CrawlMeta> }`.
  - `CrawlResult { meta: CrawlMeta, rows: Vec<UrlRow>, findings: Vec<Finding>, pages: Vec<PageRecord>, links: LinkReport, sitemap: SitemapResult }`.
- Produces (`urls.rs`) : `normalize_url(raw: &str) -> Option<String>` (sans fragment, hôte en minuscules, port par défaut retiré, chemin vide devenu `/`) et `is_same_site(a: &Url, b: &Url) -> bool` (hôtes égaux une fois le `www.` de tête retiré).
- Produces (`html.rs`) : `extract_page(html: &str, page_url: &Url) -> PageFacts`.

- [ ] **Step 1 : écrire les tests de `urls.rs`**

```rust
#[test] fn normalize_drops_fragment_and_default_port() {
    assert_eq!(normalize_url("HTTPS://X.fr:443/a#top").as_deref(), Some("https://x.fr/a"));
    assert_eq!(normalize_url("https://x.fr").as_deref(), Some("https://x.fr/"));
}
#[test] fn www_and_apex_are_same_site() { /* x.fr ~ www.x.fr : vrai ; x.fr ~ y.fr : faux ; sub.x.fr ~ x.fr : faux */ }
```

- [ ] **Step 2 : écrire les tests de `html.rs`**, page de base `https://x.fr/dir/page`

```rust
#[test] fn extracts_title_meta_h1_and_robots()      // title, meta_description, h1 == ["Un", "Deux"], meta_robots == Some("noindex, follow")
#[test] fn first_title_wins_when_duplicated()       // deux <title> → le premier
#[test] fn resolves_relative_canonical()            // href="../a" → Some("https://x.fr/a")
#[test] fn respects_base_href()                     // <base href="https://cdn.x.fr/"> + href="a" → "https://cdn.x.fr/a"
#[test] fn rel_canonical_is_case_insensitive()      // REL="Canonical" trouvé ; rel="alternate" ignoré
#[test] fn no_canonical_gives_none()
#[test] fn json_ld_valid_with_types()               // {"@type":"Organization"} → valid, ["Organization"] ;
                                                    // @graph [{"@type":"A"},{"@type":["B","C"]}] → ["A","B","C"]
#[test] fn json_ld_invalid()                        // "{" → valid == false, types vide
#[test] fn links_resolved_with_anchor_and_nofollow() // <a href="/b#x" rel="nofollow">Voir B</a> → href "https://x.fr/b", anchor "Voir B", nofollow
#[test] fn ignores_non_http_links()                 // mailto:, tel:, javascript:, "#ancre" seul → absents
#[test] fn image_alt_none_versus_empty()            // <img src="a.jpg"> → None ; alt="" → Some("")
#[test] fn word_count_ignores_script_style_noscript() // « un deux trois » + script → 3
#[test] fn text_hash_ignores_markup_and_whitespace()  // même texte, balises et espaces différents → même empreinte ; autre texte → différente
#[test] fn latin1_page_does_not_panic()             // String::from_utf8_lossy(octets Latin-1) → title présent
```

- [ ] **Step 3 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core --lib`
Expected : FAIL, fonctions absentes.

- [ ] **Step 4 : implémenter `model.rs`, `urls.rs` et `extract_page`**

`scraper` pour le DOM. L'empreinte est un SHA-256 en hexadécimal du texte visible : mots séparés par un espace, en minuscules. Le texte de `script`, `style`, `noscript` et `template` est exclu.

- [ ] **Step 5 : vérifier que les tests passent**

Run: `cargo test -p arpenteur-core --lib`
Expected : PASS.

- [ ] **Step 6 : commit**

```bash
git add -A && git commit -m "feat(core): types partagés et extraction des pages HTML"
```

### Task 4 : détection d'un site qui limite

**Files:**
- Create: `crates/arpenteur-core/src/limit_guard.rs` (tests dans le fichier)

**Interfaces:**
- Produces : `LimitGuard::new(threshold: u32) -> LimitGuard` et `LimitGuard::observe(&mut self, status: i32) -> bool`, qui renvoie `true` quand le seuil est atteint.

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn trips_after_five_consecutive_429()   // 200, puis 429 ×4 → false, 5e → true
#[test] fn status_503_counts_like_429()
#[test] fn success_resets_the_count()           // 429 ×4, 200, 429 ×4 → jamais true
#[test] fn wave_of_403_after_200_trips()        // 200, 403 ×5 → true au 5e
#[test] fn forbidden_without_prior_success_does_not_trip() // 403 ×10 → false
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core limit_guard`
Expected : FAIL.

- [ ] **Step 3 : implémenter `LimitGuard`**

Un compteur de 429/503 consécutifs, un compteur de 403 consécutifs actif seulement après au moins un 2xx. Tout 2xx remet les deux à zéro. Les autres statuts ne changent rien.

- [ ] **Step 4 : vérifier que les tests passent**, puis **commit** `feat(core): détection d'un site qui limite les requêtes`.

### Task 5 : PageCollector, l'analyse branchée sur SiteOne

**Files:**
- Create: `crates/arpenteur-core/src/collector.rs`
- Test: `crates/arpenteur-core/tests/smoke.rs` (ajout)

**Interfaces:**
- Consumes : `extract_page` (tâche 3), `LimitGuard` (tâche 4), `register_builtin_analyzers` (tâche 1).
- Produces :
  - `trait CrawlEvents: Send + Sync + 'static { fn progress(&self, p: &Progress); }`
  - `PageCollector::new(events: Arc<dyn CrawlEvents>, limit_threshold: u32, cancel: CancellationToken) -> (PageCollector, CollectorHandle)`
  - `CollectorHandle::pages(&self) -> Vec<PageRecord>` et `CollectorHandle::was_throttled(&self) -> bool`
  - `impl siteone_crawler::analysis::analyzer::Analyzer for PageCollector`, avec `get_name() == "ArpenteurPageCollector"`, `should_be_activated() == true` et `analyze()` vide.

- [ ] **Step 1 : écrire le test dans `tests/smoke.rs`**

Même harnais que la tâche 1, avec `PageCollector` à la place de `BodyProbe` et un `CrawlEvents` de test qui garde les `Progress`.

```rust
#[tokio::test] async fn collector_records_pages_and_progress() {
    let home = page(&pages, "/");
    assert_eq!(home.status, 200);
    assert_eq!(home.facts.as_ref().unwrap().title.as_deref(), Some("Accueil témoin"));
    let old = page(&pages, "/old");
    assert_eq!(old.status, 301);
    assert!(old.location().unwrap().ends_with("/a"));
    let img = page(&pages, "/img/big.jpg");
    assert!(img.facts.is_none() && img.size.unwrap() >= 250_000);
    assert_eq!(progress.len(), pages.len());
    assert_eq!(progress.last().unwrap().done as usize, pages.len());
}
```

- [ ] **Step 2 : vérifier que le test échoue**

Run: `cargo test -p arpenteur-core --test smoke collector`
Expected : FAIL, `PageCollector` absent.

- [ ] **Step 3 : implémenter `PageCollector`**

Dans `analyze_visited_url`, construire un `PageRecord` à partir de `VisitedUrl` (`url`, `status_code`, `size`, `request_time` en ms) et des en-têtes. `content_type` reprend le nom de `ContentTypeId` en minuscules. `extract_page` n'est appelé que si le type est HTML et le corps présent. Ensuite : pousser le record dans l'état partagé, émettre un `Progress` et passer le statut à `LimitGuard`. Si le garde se déclenche, marquer `throttled` puis appeler `cancel.cancel()`. La méthode renvoie `None`.

- [ ] **Step 4 : vérifier que le test passe**, puis **commit** `feat(core): collecte des pages et progression`.

### Task 6 : découverte du sitemap

**Files:**
- Create: `crates/arpenteur-core/src/sitemap.rs`
- Test: `crates/arpenteur-core/tests/crawl.rs` (création)

**Interfaces:**
- Consumes : `is_same_site`, `normalize_url` (tâche 3), `SitemapResult` (tâche 3).
- Produces : `async fn discover(client: &reqwest::Client, start: &Url, min_interval: Duration, max_urls: usize) -> SitemapResult`.

- [ ] **Step 1 : écrire les tests**

```rust
#[tokio::test] async fn reads_sitemap_declared_in_robots()   // Standard : found, 5 urls, sources == [base/sitemap.xml]
#[tokio::test] async fn falls_back_to_sitemap_xml()          // NoSitemapLine : found, 5 urls
#[tokio::test] async fn missing_sitemap_is_not_found()       // NoSitemap : !found, urls vide, pas d'erreur bloquante
#[tokio::test] async fn follows_sitemap_index_and_caps()     // HugeSitemap, max_urls 10_000 : urls.len() == 10_000, truncated
#[tokio::test] async fn respects_min_interval()              // Standard, intervalle 200 ms : durée totale ≥ 400 ms (robots + sitemap)
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core --test crawl sitemap`
Expected : FAIL.

- [ ] **Step 3 : implémenter `discover`**

Lire `{origin}/robots.txt` et ses lignes `Sitemap:` (sans tenir compte de la casse), sinon prendre `{origin}/sitemap.xml`. Lire les `<urlset>` et les `<sitemapindex>` avec `quick-xml`, sur trois niveaux au plus, sans jamais relire un sitemap déjà vu. Les `.gz` sont décompressés avec `flate2`. Une pause de `min_interval` précède chaque requête après la première. Seules les URL du même site sont gardées, normalisées et dédoublonnées. On s'arrête à `max_urls` avec `truncated = true`. Une erreur réseau ou XML va dans `errors`, sans faire échouer la découverte.

- [ ] **Step 4 : vérifier que les tests passent**, puis **commit** `feat(core): découverte des URL du sitemap`.

### Task 7 : stockage des crawls

**Files:**
- Create: `crates/arpenteur-core/src/storage.rs` (tests dans le fichier, avec un dossier temporaire)

**Interfaces:**
- Produces : `CrawlStore` (`Clone`) avec :
  - `new(root: PathBuf) -> CrawlStore`
  - `create_crawl(&self, domain: &str, started_at: DateTime<Local>) -> io::Result<CrawlLocation>`
  - `write_meta(&self, loc: &CrawlLocation, meta: &CrawlMeta) -> io::Result<()>` (`crawl.json`)
  - `write_data(&self, loc: &CrawlLocation, data: &CollectedData) -> io::Result<()>` (`arpenteur.json`)
  - `list_sites(&self) -> io::Result<Vec<SiteSummary>>`
  - `read_crawl(&self, domain: &str, id: &str) -> io::Result<(CrawlMeta, CollectedData)>`
  - `pub fn domain_key(host: &str) -> String`

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn creates_dated_dir()             // id == "2026-11-20_14-03-07", dir == root/sites/exemple.fr/<id>
#[test] fn same_second_gets_suffix()       // 2e création à la même seconde → id "2026-11-20_14-03-07-2"
#[test] fn domain_key_is_sanitized()       // "Exemple.FR" → "exemple.fr" ; "localhost:8080" → "localhost_8080"
#[test] fn list_sites_sorted()             // sites par ordre alphabétique, crawls du plus récent au plus ancien ; dossier sans crawl.json ignoré
#[test] fn roundtrip_meta_and_data()
#[test] fn writes_are_atomic()             // écriture dans un .tmp puis rename : aucun .tmp ne reste
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**, **Step 4 : vérifier qu'ils passent**

Run: `cargo test -p arpenteur-core storage`

- [ ] **Step 5 : commit** `feat(core): stockage des crawls par site`.

### Task 8 : réglages et lancement d'un crawl

**Files:**
- Modify: `crates/arpenteur-core/src/settings.rs`, `crates/arpenteur-core/src/engine.rs`
- Test: tests unitaires dans `settings.rs`, et `tests/crawl.rs` (ajout)

**Interfaces:**
- Consumes : `PageCollector` (tâche 5), `discover` (tâche 6), `CrawlStore` (tâche 7).
- Produces :
  - `settings::normalize_start_url(input: &str) -> Result<Url, CrawlError>`
  - `settings::default_user_agent() -> String` (`format!("Arpenteur/{}!", env!("CARGO_PKG_VERSION"))`)
  - `settings::build_argv(settings: &CrawlSettings, start: &Url, crawl_dir: &Path) -> Vec<String>`
  - `settings::build_core_options(settings: &CrawlSettings, start: &Url, crawl_dir: &Path, seeds: Vec<String>) -> Result<CoreOptions, CrawlError>`
  - `enum CrawlError { InvalidUrl(String), Options(String) }`, qui implémente `Display` en français
  - `engine::run_crawl(settings: CrawlSettings, start: Url, store: CrawlStore, location: CrawlLocation, events: Arc<dyn CrawlEvents>, cancel: CancellationToken) -> CrawlMeta` (async, ne renvoie jamais d'erreur : un échec devient `CrawlStatus::Failed`)

- [ ] **Step 1 : écrire les tests de `settings.rs`**

```rust
#[test] fn normalize_adds_scheme_and_trims() {
    assert_eq!(normalize_start_url("exemple.fr").unwrap().as_str(), "https://exemple.fr/");
    assert_eq!(normalize_start_url("  https://exemple.fr/x ").unwrap().as_str(), "https://exemple.fr/x");
}
#[test] fn normalize_rejects_non_urls() {
    assert!(normalize_start_url("pas une url").is_err());
    assert!(normalize_start_url("ftp://exemple.fr").is_err());
}
#[test] fn argv_carries_defaults() {
    let argv = build_argv(&CrawlSettings::default(), &url, dir);
    for expected in ["--workers=1", "--max-reqs-per-sec=2", "--max-visited-urls=10000",
                     "--max-queue-length=10000", "--http-cache-dir=off", "--no-color",
                     "--output-text-file="] {
        assert!(argv.iter().any(|a| a == expected), "{expected}");
    }
    assert!(argv.contains(&format!("--ignore-regex={ASSET_IGNORE_REGEX}")));
    assert!(argv.contains(&format!("--user-agent={}", default_user_agent())));
    assert!(argv.contains(&format!("--output-json-file={}", dir.join("siteone.json").display())));
    assert!(argv.contains(&format!("--output-html-report={}", dir.join("siteone-report.html").display())));
    assert!(!argv.iter().any(|a| a == "--ignore-robots-txt" || a == "--help" || a == "--version"));
}
#[test] fn argv_maps_custom_settings()  // respect_robots false → --ignore-robots-txt ; include/ignore regex répétées ; user_agent fourni tel quel
```

- [ ] **Step 2 : écrire les tests de bout en bout dans `tests/crawl.rs`**

Réglages rapides pour les tests : `workers 2`, `max_reqs_per_sec 50.0`.

```rust
#[tokio::test] async fn crawl_completes_and_writes_files() {
    assert_eq!(meta.status, CrawlStatus::Completed);
    for f in ["crawl.json", "arpenteur.json", "siteone.json"] { assert!(loc.dir.join(f).exists()); }
    // pages contient /, /a, /b, /orphan (venue du sitemap), /img/big.jpg
    assert!(!site.requested("/app.js") && !site.requested("/style.css"));
}
#[tokio::test] async fn stop_keeps_collected_pages() {
    // CrawlEvents de test qui annule le jeton au premier Progress
    assert_eq!(meta.status, CrawlStatus::Stopped);
    let (m, data) = store.read_crawl(&loc.domain, &loc.id).unwrap();
    assert_eq!(m.status, CrawlStatus::Stopped);
    assert!(!data.pages.is_empty());
    assert!(m.finished_at.is_some());
}
#[tokio::test] async fn throttled_site_stops_crawl() {   // Variant::Throttled
    assert_eq!(meta.status, CrawlStatus::Throttled);
}
```

- [ ] **Step 3 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core settings && cargo test -p arpenteur-core --test crawl crawl_`
Expected : FAIL.

- [ ] **Step 4 : implémenter `settings.rs`**

`build_core_options` appelle `parse_argv(&build_argv(...))`, puis remplit `options.url_list_urls = seeds`. Une erreur de `parse_argv` devient `CrawlError::Options`.

- [ ] **Step 5 : implémenter `run_crawl`**

Dans l'ordre :
1. Écrire `crawl.json` avec le statut `Running`.
2. Lancer `discover`, avec un client `reqwest` qui porte le User-Agent, `min_interval = 1 / max_reqs_per_sec` et `max_urls = max_requests`.
3. Construire les options avec les URL du sitemap comme liste de départ.
4. Préparer l'`AnalysisManager` : `register_builtin_analyzers`, puis `PageCollector`, puis `auto_activate_analyzers()`.
5. Faire un `tokio::select!` entre `manager.run()` (`Completed`, ou `Failed` sur erreur) et `cancel.cancelled()` (`Throttled` si `was_throttled()`, sinon `Stopped`). Le lancement suit la décision du Step 8 de la tâche 1.
6. Toujours écrire `arpenteur.json`, puis `crawl.json` avec `finished_at` et `pages`.

Une annulation pendant la découverte du sitemap donne aussi `Stopped`.

- [ ] **Step 6 : vérifier que les tests passent**

Run: `cargo test -p arpenteur-core`
Expected : PASS.

- [ ] **Step 7 : commit** `feat(core): lancement, arrêt et enregistrement d'un crawl`.

### Task 9 : liens, profondeur et chaînes de redirection

**Files:**
- Create: `crates/arpenteur-core/src/links.rs` (tests dans le fichier, avec des `PageRecord` construits à la main par un petit constructeur de test)

**Interfaces:**
- Consumes : `PageRecord`, `LinkReport`, `normalize_url`, `is_same_site`.
- Produces : `analyze_links(pages: &[PageRecord], start_url: &str) -> LinkReport`. Toutes les clés sont des URL normalisées.

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn counts_inlinks_from_distinct_pages_excluding_self()
#[test] fn depth_from_start_with_transparent_redirects()
    // / → /old (301 → /a), /a → /b : depth / = 0, /old = 1, /a = 1, /b = 2
#[test] fn unreached_page_has_no_depth()        // /orphan absent de depth
#[test] fn detects_chain_of_two_redirects()     // /r1 → /r2 → /final : redirect_chains["…/r1"] == ["…/r2", "…/final"]
#[test] fn single_redirect_is_not_a_chain()
#[test] fn redirect_loop_terminates()           // /x → /y → /x : pas de boucle infinie, chaîne enregistrée
#[test] fn www_and_apex_links_are_internal()    // départ https://x.fr/, lien vers https://www.x.fr/a compté
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**, **Step 4 : vérifier qu'ils passent**

Lien interne : cible du même site que l'URL de départ. Profondeur : parcours en largeur depuis l'URL de départ, sur les liens des pages HTML. Une redirection transmet la profondeur de sa source à sa cible, sans niveau supplémentaire.

Run: `cargo test -p arpenteur-core links`

- [ ] **Step 5 : commit** `feat(core): liens entrants, profondeur et chaînes de redirection`.

### Task 10 : nos contrôles, les lignes du tableau et l'ouverture d'un crawl

**Files:**
- Create: `crates/arpenteur-core/src/checks.rs` (tests dans le fichier)
- Modify: `crates/arpenteur-core/src/storage.rs` (ajout de `load_result`)
- Test: `tests/crawl.rs` (ajout)

**Interfaces:**
- Consumes : `analyze_links` (tâche 9), `CrawlStore::read_crawl` (tâche 7).
- Produces :
  - `pub const IMAGE_HEAVY_BYTES: i64 = 204_800;` et `pub const THIN_CONTENT_WORDS: u32 = 200;`
  - `run_checks(pages: &[PageRecord], links: &LinkReport, sitemap: &SitemapResult, start_url: &str) -> Vec<Finding>`
  - `build_rows(pages: &[PageRecord], links: &LinkReport) -> Vec<UrlRow>` (une ligne par page collectée, dans l'ordre du crawl)
  - `is_indexable(page: &PageRecord) -> bool` : statut 200, page HTML, pas de `noindex` (meta robots ou `X-Robots-Tag`), canonical absent ou égal à l'URL elle-même
  - `assemble(meta: CrawlMeta, data: CollectedData) -> CrawlResult`
  - `CrawlStore::load_result(&self, domain: &str, id: &str) -> io::Result<CrawlResult>` = `read_crawl` puis `assemble`

- [ ] **Step 1 : écrire les tests unitaires**

Chaque code a son cas positif et son cas négatif (règle D5).

```rust
#[test] fn canonical_missing_only_on_html_200()
#[test] fn self_canonical_is_not_flagged()
#[test] fn canonical_to_other_url_is_reported()
#[test] fn canonical_to_redirect()
#[test] fn canonical_to_error()
#[test] fn canonical_to_other_host()
#[test] fn apex_canonical_that_redirects_is_redirect_not_other_host()
    // pages sur www.x.fr, canonical vers https://x.fr/a qui redirige en 301 vers https://www.x.fr/a :
    // canonicalToRedirect présent, canonicalToOtherHost absent
#[test] fn temporary_redirect_on_internal_url_only()  // 302, 303, 307 internes signalés ; 301 et 308 non
#[test] fn redirect_chain_reported_with_hops()
#[test] fn broken_internal_link_lists_all_sources()   // cible en 404 ou statut < 0, related == toutes les pages sources
#[test] fn image_too_heavy_threshold()                // 204_800 non signalé, 204_801 signalé ; related == pages qui l'affichent
#[test] fn json_ld_missing_and_invalid()             // page HTML 200 sans bloc → missing ; bloc invalide → invalid ; valide → rien
#[test] fn orphan_page_from_sitemap()                 // URL du sitemap sans lien entrant → orphanPage ; liée → rien
#[test] fn not_in_sitemap_only_when_sitemap_found()
#[test] fn sitemap_url_error_and_non_indexable()
#[test] fn sitemap_not_found_reported_once()
#[test] fn duplicate_content_groups_pages()           // même text_hash sur 2 pages HTML 200 : chacune signalée, related == les autres
#[test] fn thin_content_threshold()                   // 199 mots signalé, 200 non
#[test] fn noindex_from_meta_or_header()
```

- [ ] **Step 2 : écrire le test d'ouverture dans `tests/crawl.rs`** (`has`, `has_with_related` et `row` sont de petits helpers de test qui comparent les chemins d'URL)

```rust
#[tokio::test] async fn load_result_on_fixture_site() {
    let r = store.load_result(&loc.domain, &loc.id).unwrap();
    assert!(has(&r, FindingCode::CanonicalToRedirect, "/b"));
    assert!(has(&r, FindingCode::OrphanPage, "/orphan"));
    assert!(has_with_related(&r, FindingCode::BrokenInternalLink, "/missing", &["/"]));
    assert!(has(&r, FindingCode::ImageTooHeavy, "/img/big.jpg"));
    assert!(has(&r, FindingCode::JsonLdInvalid, "/b"));
    assert!(has(&r, FindingCode::TemporaryRedirect, "/temp"));
    assert!(has(&r, FindingCode::DuplicateContent, "/dup1"));
    assert!(has(&r, FindingCode::SitemapUrlError, "/gone"));
    assert_eq!(r.rows.len(), r.pages.len());
    assert_eq!(row(&r, "/b").depth, Some(1));
}
```

- [ ] **Step 3 : vérifier que les tests échouent**, **Step 4 : implémenter**, **Step 5 : vérifier qu'ils passent**

Run: `cargo test -p arpenteur-core`
Expected : PASS.

- [ ] **Step 6 : commit** `feat(core): nos contrôles SEO et l'assemblage d'un crawl`.

### Task 11 : la coquille Tauri

**Files:**
- Create: `package.json`, `vite.config.ts`, `index.html`, `tsconfig*.json`, `src-tauri/` (`Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`, `src/main.rs`, `src/lib.rs`, `src/state.rs`, `src/events.rs`, `src/commands.rs`)
- Modify: `Cargo.toml` (ajout de `src-tauri` au workspace), `.github/workflows/ci.yml`

**Interfaces:**
- Consumes : `normalize_start_url`, `run_crawl`, `CrawlStore`, `CrawlEvents`.
- Produces :
  - `AppState { store: CrawlStore, running: Mutex<Option<CancellationToken>> }`, avec `try_begin(&self) -> Result<CancellationToken, AlreadyRunning>`, `stop(&self) -> bool` et `finish(&self)`
  - les commandes `start_crawl(url: String, settings: Option<CrawlSettings>) -> Result<StartedCrawl, String>` (`StartedCrawl { domain: String, id: String }`), `stop_crawl() -> bool`, `list_sites() -> Result<Vec<SiteSummary>, String>` et `load_crawl(domain: String, id: String) -> Result<CrawlResult, String>`
  - les événements `crawl-progress` (charge `Progress`) et `crawl-finished` (charge `CrawlMeta`)

- [ ] **Step 1 : générer la coquille**

Dans un dossier temporaire, lancer `npm create tauri-app@latest` (template React + TypeScript, gestionnaire npm, identifiant `app.arpenteur.desktop`, nom de fenêtre « Arpenteur »). Déplacer dans le dépôt `src-tauri/`, `src/`, `index.html`, `package.json`, `vite.config.ts` et `tsconfig*.json`. Ajouter `src-tauri` au workspace, et `arpenteur-core` (chemin) aux dépendances de `src-tauri`.

- [ ] **Step 2 : écrire les tests de `state.rs`**

```rust
#[test] fn second_begin_is_refused()   // try_begin OK puis try_begin → Err(AlreadyRunning)
#[test] fn finish_allows_a_new_crawl()
#[test] fn stop_cancels_the_running_token() // stop() == true et jeton annulé ; sans crawl, stop() == false
```

- [ ] **Step 3 : vérifier que les tests échouent**, **Step 4 : implémenter `state.rs`**, **Step 5 : vérifier qu'ils passent**

Run: `cargo test -p arpenteur`

- [ ] **Step 6 : implémenter `events.rs` et `commands.rs`**

`TauriEvents` implémente `CrawlEvents` et émet `crawl-progress` au plus toutes les 100 ms. `start_crawl` enchaîne :
1. `normalize_start_url`, une erreur donnant le message « Adresse invalide : {saisie} » ;
2. `try_begin`, un refus donnant « Un crawl est déjà en cours. » ;
3. `create_crawl` ;
4. `tauri::async_runtime::spawn` de `run_crawl`, puis `finish()` et l'émission de `crawl-finished`.

Le dossier racine est `app.path().app_data_dir()`.

- [ ] **Step 7 : compléter la CI**

Ajouter un job sur la même matrice : `actions/setup-node` (Node 24), `npm ci`, `npm test`, puis `npx tauri build --debug --no-bundle`. L'exécutable Windows est publié en artefact.

- [ ] **Step 8 : vérifier**

Run: `cargo test -p arpenteur && npm run tauri dev`
Expected : tests PASS, la fenêtre « Arpenteur » s'ouvre (encore avec l'écran du modèle). Après le push, avec l'accord de l'utilisateur, le job Windows est vert.

- [ ] **Step 9 : commit** `feat(app): coquille Tauri, commandes et événements de crawl`.

### Task 12 : couche front et écran d'accueil

**Files:**
- Create: `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/table.ts`, `src/lib/table.test.ts`, `src/i18n/fr.ts`, `src/i18n/fr.test.ts`, `src/screens/Home.tsx`, `src/screens/Home.test.tsx`
- Modify: `src/App.tsx`, `package.json` (Vitest, Testing Library, jsdom)

**Interfaces:**
- Consumes : les commandes et événements de la tâche 11.
- Produces :
  - `types.ts` : le miroir TypeScript des types de `model.rs` (`CrawlStatus` en union sur `kind`)
  - `api.ts` : `startCrawl(url: string, settings?: Partial<CrawlSettings>): Promise<StartedCrawl>`, `stopCrawl(): Promise<boolean>`, `listSites(): Promise<SiteSummary[]>`, `loadCrawl(domain: string, id: string): Promise<CrawlResult>`, `onProgress(cb: (p: Progress) => void): Promise<UnlistenFn>`, `onFinished(cb: (m: CrawlMeta) => void): Promise<UnlistenFn>`
  - `table.ts` : `sortRows(rows: UrlRow[], key: keyof UrlRow, dir: 'asc' | 'desc'): UrlRow[]` et `filterRows(rows: UrlRow[], f: { query: string; statusClass: 'all' | '2xx' | '3xx' | '4xx' | '5xx' | 'error' }): UrlRow[]`
  - `fr.ts` : `fr` (objet de textes) et `t(key: keyof typeof fr, vars?: Record<string, string | number>): string`
  - `App.tsx` : la navigation entre `Home` et `Crawl` (`{ domain, id, running }`)

- [ ] **Step 1 : écrire les tests**

```ts
test('sortRows met les valeurs nulles en dernier', ...)          // depth [2, null, 0] asc → [0, 2, null]
test('sortRows trie le texte à la française', ...)               // « école » entre « eau » et « fable »
test('filterRows cherche dans url, title et h1', ...)
test('filterRows par classe de statut', ...)                     // 'error' = status < 0
test('t remplace les variables', ...)                            // t('progress', { done: 3, elapsed: '4 s' }) === '3 URL traitées · 4 s'
test('Accueil affiche le message d’erreur de lancement', ...)    // startCrawl rejette « Adresse invalide : x » → texte visible
test('Accueil liste les crawls du plus récent au plus ancien', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `npm test`
Expected : FAIL.

- [ ] **Step 3 : implémenter**

`Home` contient le champ URL et le bouton « Lancer », le message d'erreur, et la liste des sites. Chaque crawl de la liste montre sa date, son statut traduit et son nombre de pages, et s'ouvre au clic. Les textes viennent de `fr.ts`, y compris les libellés de statut : `completed` « Terminé », `stopped` « Arrêté », `throttled` « Interrompu : le site limite les requêtes », `failed` « Échec », `running` « En cours ».

- [ ] **Step 4 : vérifier que les tests passent**, puis **commit** `feat(ui): accueil, appels au backend et outils de tableau`.

### Task 13 : écran du crawl, tableau et détail d'une URL

**Files:**
- Create: `src/screens/Crawl.tsx`, `src/screens/Crawl.test.tsx`, `src/components/UrlTable.tsx`, `src/components/UrlDetail.tsx`
- Modify: `src/i18n/fr.ts`, `package.json` (`@tanstack/react-virtual`)

**Interfaces:**
- Consumes : `api.ts`, `table.ts`, `fr.ts` (tâche 12).
- Produces : `Crawl({ domain, id, running }: { domain: string; id: string; running: boolean })`.

- [ ] **Step 1 : écrire les tests** (API simulée)

```ts
test('la progression met à jour le compteur', ...)        // 2 événements → « 2 URL traitées »
test('Arrêter appelle stopCrawl', ...)
test('à la fin, le résultat est chargé et affiché', ...)  // onFinished → loadCrawl → lignes visibles
test('un clic sur un en-tête trie le tableau', ...)
test('un clic sur une ligne ouvre le détail avec les liens entrants', ...)
test('un crawl arrêté affiche son bandeau', ...)          // « Crawl arrêté avant la fin. »
test('un crawl interrompu conseille de baisser le débit', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `npm test`
Expected : FAIL.

- [ ] **Step 3 : implémenter**

- **Pendant le crawl** : une ligne de progression (`t('progress', …)`) et le bouton « Arrêter ».
- **À la fin** : un bandeau selon le statut, puis `UrlTable`.
- **`UrlTable`** : tableau virtualisé avec les colonnes URL, Statut, Type, Title, H1, Indexable, Profondeur, Liens entrants, Mots. Un clic sur un en-tête trie, un champ de recherche et une liste de classes de statut filtrent.
- **`UrlDetail`**, en bas : canonical, meta description, les H1, les liens entrants (page et ancre), les liens sortants, les en-têtes, les types JSON-LD.

- [ ] **Step 4 : vérifier que les tests passent**

Run: `npm test`
Expected : PASS.

- [ ] **Step 5 : vérifier dans l'app**

Run: `npm run tauri dev`, puis crawler `https://www.crocweb.fr/` avec les réglages par défaut.
Expected :
- environ 15 pages et leurs images en une à deux minutes ;
- `/tarifs` à la profondeur 1, avec des liens entrants dans son détail ;
- un deuxième crawl arrêté en cours de route affiche le bandeau « arrêté » et reste dans la liste de l'accueil.

- [ ] **Step 6 : commit** `feat(ui): écran du crawl, tableau des URL et détail`.
