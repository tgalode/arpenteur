# Arpenteur, plan 3 : fiabilité, tableau complet, liens externes et mode liste Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rendre les constats plus fiables (six corrections), exporter 16 colonnes et laisser choisir celles du tableau, signaler les liens externes cassés, et crawler une liste d'URL collée sans suivre les liens.

**Architecture:** le cœur Rust garde son découpage : extraction (`html.rs`), contrôles (`checks.rs`), sondes après crawl (`canonical_probe.rs`, plus un nouveau `external_links.rs`), moteur (`engine.rs`). Deux petites structures arrêtent la croissance des listes de paramètres : `CheckInput` pour les contrôles et `ProbeEnv` pour les sondes. Le filtre robots.txt et motifs, aujourd'hui enfoui dans `crawl_seeds`, devient un `UrlFilter` partagé par le sitemap, la sonde et le mode liste. L'interface reçoit les nouvelles données par `CrawlResult` et garde ses textes dans `fr.ts`.

**Tech Stack:** Rust (`arpenteur-core`, `siteone-crawler =2.5.1`, `reqwest`, `tokio`), Tauri 2, React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-arpenteur-plan-3-design.md` (fait foi). Contexte : `docs/superpowers/plans/2026-11-20-arpenteur-plan-2-bilan.md`, catalogue `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md`.

**Branche :** `feature/plan-3-fiabilite-liens-liste`, créée depuis `main`, sans worktree. Jamais de commit de code sur `main`.

## Global Constraints

- Tout nouveau champ enregistré (`crawl.json`, `arpenteur.json`, `settings.json`) porte `#[serde(default)]`. Un crawl enregistré avant le plan 3 s'ouvre sans erreur et sans nouveau constat.
- Règle D5 : chaque contrôle a un test positif et un test négatif sur sa frontière. Un faux positif coûte plus qu'un contrôle manquant.
- Toute requête ajoutée respecte le débit réglé et compte dans la limite de requêtes. Ordre dans le budget restant : compression, cibles de canonical, liens externes.
- Un lien externe n'est « cassé » que si la réponse finale est 404 ou 410, ou si le nom de domaine n'existe plus après un second essai. 401, 403, 429, 999, les 5xx, un délai dépassé et une connexion refusée ne donnent aucun constat.
- Délai maximal d'une requête de sonde : 5 secondes. Redirections suivies pour un lien externe : 5 au plus.
- Le catalogue validé fait foi. Il passe à 38 codes : 7 en haute, 15 en moyenne, 12 en basse, 4 en information. Les textes de `brokenExternalLink` sont copiés à l'identique depuis la conception.
- Constats sur les pages internes uniquement (même site que l'URL de départ, `is_same_site`).
- Interface : textes dans `fr.ts`, habillage Console (variables de `styles.css`, aucune couleur codée en dur hors `:root`, pas de style en ligne hors virtualisation et `gridTemplateColumns`). Aucun tiret cadratin dans un texte ou un document.
- SiteOne reste en `=2.5.1`. Ne jamais pousser ni créer de dépôt distant sans l'accord de l'utilisateur.

## Review Focus

1. **Un crawl enregistré avant le plan 3** (sans `probed`, `mode`, `listUrls`, `externalLinks`) et un `settings.json` sans `checkExternalLinks` s'ouvrent sans erreur ; le crawl garde ses constats et n'en gagne aucun. Test dans les tâches 3, 5 et 7 (`tests/legacy_crawl.rs`).
2. **Un site tiers qui bloque les robots** (403, 429, 999), qui répond 500 ou qui ne répond pas : aucun « Lien externe cassé ». Test dans la tâche 5.
3. **Le même lien externe présent sur 50 pages** est demandé une seule fois, et le constat cite les 50 pages. Tests dans les tâches 5 et 6.
4. **Une liste collée depuis un tableur** (fins de ligne Windows, espaces, lignes vides, BOM, doublons, une URL d'un autre site) : nettoyée, ou refusée avec le numéro des lignes fautives. Test dans la tâche 7.
5. **Un choix de colonnes enregistré devenu invalide** (clé inconnue, JSON abîmé, stockage indisponible) : le tableau revient aux 9 colonnes par défaut, sans erreur. Test dans la tâche 4.

---

### Task 1 : fiabilité des contrôles de page (crawl limité, noindex, titres hors contenu)

**Files:**
- Modify: `crates/arpenteur-core/src/checks.rs`, `crates/arpenteur-core/src/html.rs`

**Interfaces:**
- Produces :

```rust
pub struct CheckInput<'a> {
    pub pages: &'a [PageRecord],
    pub links: &'a LinkReport,
    pub sitemap: &'a SitemapResult,
    pub start_url: &'a str,
    pub start_content_encoding: Option<&'a str>, // None par défaut
    pub facts_version: u32,                      // FACTS_VERSION par défaut
    pub throttled: bool,                         // false par défaut
}
impl<'a> CheckInput<'a> {
    pub fn new(pages: &'a [PageRecord], links: &'a LinkReport, sitemap: &'a SitemapResult, start_url: &'a str) -> Self;
}
pub fn run_checks(input: &CheckInput) -> Vec<Finding>;
```

  `assemble` construit le `CheckInput` avec `throttled = meta.status == CrawlStatus::Throttled`. Tous les appels existants de `run_checks` (tests compris) passent par `CheckInput`. Les tâches suivantes ajoutent des champs avec une valeur par défaut dans `new`.

- [ ] **Step 1 : écrire les tests**

```rust
// checks.rs
#[test] fn throttled_crawl_ignores_503_and_429()
    // crawl limité : pages internes en 503 et 429, liées et dans le sitemap → ni serverError,
    // ni brokenInternalLink, ni sitemapUrlError, ni canonicalToError ;
    // 500 sur le même crawl → serverError ; 503 sur un crawl non limité → serverError
#[test] fn noindex_page_skips_title_meta_and_h1_checks()
    // page noindex (meta robots) sans title, sans meta description, sans H1 → aucun des 8 codes
    // titleMissing/TooLong/TooShort, metaDescriptionMissing/TooLong/TooShort, h1Missing, h1Multiple ;
    // le constat noindex reste ; imageMissingAlt et htmlLangMissing restent
    // même page sans noindex → titleMissing, metaDescriptionMissing, h1Missing
#[test] fn noindex_by_x_robots_tag_also_skips()
// html.rs
#[test] fn headings_ignore_footer_nav_and_aside()
    // <h1>A</h1><footer><h3>B</h3></footer> → headings [1]
    // <aside><div><h4>C</h4></div></aside> et <nav><h2>D</h2></nav> ignorés
    // <main><h1>A</h1><h3>B</h3></main> → [1, 3]
#[test] fn h1_detection_is_unchanged_inside_nav()
    // <nav><h1>Menu</h1></nav><h1>Titre</h1> → facts.h1 == ["Menu", "Titre"]
```

- [ ] **Step 2 : vérifier que les tests échouent** (`cargo test -p arpenteur-core checks html`)

- [ ] **Step 3 : implémenter**

- `is_error` reçoit `throttled` : sur un crawl limité, 503 et 429 ne sont pas des erreurs. Toutes les règles qui s'appuient sur `is_error` suivent.
- Les huit contrôles de title, meta description et H1 sont sautés quand `is_noindex(page)`.
- `headings` exclut les titres qui ont un ancêtre `footer`, `nav` ou `aside`. `facts.h1` n'est pas touché.

- [ ] **Step 4 : vérifier** (`cargo test -p arpenteur-core`)

- [ ] **Step 5 : commit** `feat(core): constats plus justes sur crawl limité, pages en noindex et titres hors contenu`

### Task 2 : en-têtes de sécurité lus sur la page de référence

**Files:**
- Modify: `crates/arpenteur-core/src/checks.rs`, `crates/arpenteur-core/src/siteone.rs`, `crates/arpenteur-core/src/canonical_probe.rs`

**Interfaces:**
- Consumes : `CheckInput` (tâche 1).
- Produces :

```rust
pub const SECURITY_HEADERS: [&str; 5] = [
    "Strict-Transport-Security", "Content-Security-Policy", "X-Frame-Options",
    "X-Content-Type-Options", "Referrer-Policy",
];
/// La ligne de l'URL de départ si elle est HTML en 200, sinon la première page HTML interne en 200.
pub fn reference_page<'a>(pages: &'a [PageRecord], start_url: &str) -> Option<&'a PageRecord>;
```

  Le constat `SecurityHeadersMissing` : un seul, sur l'URL de départ, `related` = les en-têtes absents dans l'ordre de `SECURITY_HEADERS`, `detail` = `Absents : A, B.`. `siteone.rs` ne produit plus ce code : `SecurityIssue`, `SiteOneSummary.security_issues` et leurs tests disparaissent. `canonical_probe.rs` utilise `reference_page` pour choisir la page de la sonde de compression.

- [ ] **Step 1 : écrire les tests** (`checks.rs`)

```rust
#[test] fn all_security_headers_present_gives_no_finding()
#[test] fn missing_headers_are_listed_in_order()
    // absents : CSP et Referrer-Policy → related == ["Content-Security-Policy", "Referrer-Policy"],
    // detail == "Absents : Content-Security-Policy, Referrer-Policy."
#[test] fn csp_frame_ancestors_counts_as_x_frame_options()
#[test] fn hsts_is_not_expected_on_http()       // site en http:// sans HSTS → HSTS non cité
#[test] fn security_headers_use_reference_page()
    // départ en 301, première page HTML interne en 200 sans X-Content-Type-Options → constat ;
    // aucune page HTML en 200 → rien
#[test] fn header_names_are_case_insensitive()  // les clés enregistrées sont en minuscules
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**, **Step 4 : vérifier** (`cargo test -p arpenteur-core`)

- [ ] **Step 5 : commit** `feat(core): en-têtes de sécurité lus sur la page de départ, en français`

### Task 3 : sonde filtrée et lignes sondées « non analysé »

**Files:**
- Modify: `crates/arpenteur-core/src/model.rs`, `settings.rs`, `canonical_probe.rs`, `engine.rs`, `checks.rs`, `crates/arpenteur-core/tests/crawl.rs`, `tests/support/mod.rs`, `tests/legacy_crawl.rs`
- Modify: `src/lib/types.ts`, `src/components/UrlTable.tsx`, `src/lib/csv.ts`, `src/lib/table.ts`, `src/i18n/fr.ts` et leurs tests

**Interfaces:**
- Produces :

```rust
// settings.rs
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SkipReason { Robots, Pattern }
pub struct UrlFilter { /* motifs compilés, robots.txt */ }
impl UrlFilter {
    /// `robots_txt` est ignoré si `settings.respect_robots` est faux.
    pub fn new(settings: &CrawlSettings, robots_txt: Option<&str>) -> UrlFilter;
    pub fn check(&self, url: &str) -> Result<(), SkipReason>;
}
// canonical_probe.rs
pub struct ProbeEnv<'a> {
    pub client: &'a reqwest::Client,
    pub start: &'a Url,
    pub filter: &'a UrlFilter,
    pub min_interval: Duration,
    pub cancel: &'a CancellationToken,
}
/// Renvoie les lignes ajoutées et le nombre de requêtes faites.
pub async fn probe_canonical_targets(env: &ProbeEnv<'_>, pages: &[PageRecord], budget: usize) -> (Vec<PageRecord>, usize);
pub async fn run_post_crawl_probes(env: &ProbeEnv<'_>, pages: &[PageRecord], max_requests: usize) -> PostCrawl;
// model.rs
PageRecord { #[serde(default)] pub probed: bool, .. }
#[serde(rename_all = "camelCase")]
pub enum Indexability { Yes, No, NotAnalyzed }   // "yes" | "no" | "notAnalyzed"
UrlRow { pub indexable: Indexability, .. }
// checks.rs
pub fn indexability(page: &PageRecord) -> Indexability;  // probed → NotAnalyzed
```

  `crawl_seeds` s'appuie sur `UrlFilter` (même comportement). Une ligne sondée porte `probed = true` et un `content_type` tiré de l'en-tête `Content-Type` de la réponse, avec le vocabulaire des pages crawlées (`html`, `image`, `document`…) : réutiliser la correspondance de SiteOne si elle est publique, sinon `text/html` → `html`, `image/*` → `image`, `application/pdf` → `document`, le reste → `other` ; un 3xx reste `redirect`. `meta.pages` compte les lignes non sondées. Miroir TypeScript : `probed: boolean` sur `PageRecord`, `indexable: "yes" | "no" | "notAnalyzed"` sur `UrlRow`.

- [ ] **Step 1 : écrire les tests**

```rust
// settings.rs
#[test] fn url_filter_reports_robots_and_pattern()
    // Disallow: /prive → check(".../prive/x") == Err(Robots) ; ignore_regex "/tag/" → Err(Pattern) ;
    // include_regex "/blog/" → une URL hors /blog/ → Err(Pattern) ; respect_robots faux → Ok
// tests/crawl.rs (site de test local, variante témoin à ajouter : canonical vers une cible interdite par robots.txt)
#[tokio::test(flavor = "multi_thread")] async fn disallowed_canonical_target_is_not_probed()
    // la cible n'est jamais demandée ; la page source garde canonicalToOtherUrl
#[tokio::test(flavor = "multi_thread")] async fn pattern_excluded_canonical_target_is_not_probed()
#[tokio::test(flavor = "multi_thread")] async fn probed_row_is_marked_and_typed()
    // cible en 200 servie en application/pdf → probed, content_type "document",
    // indexable NotAnalyzed, aucun constat sitemapUrlNonIndexable même si elle est dans le sitemap
#[tokio::test(flavor = "multi_thread")] async fn probed_rows_are_not_counted_in_meta_pages()
// tests/legacy_crawl.rs
#[test] fn crawl_without_probed_field_loads()   // Review Focus 1 : mêmes constats qu'avant
```

```ts
test('indexable affiche oui, non ou non analysé', ...)   // « non analysé » sans ton d'alerte
test('le CSV écrit oui, non ou non analysé', ...)
test('le tri sur indexable range oui, non, non analysé', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**, **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit`)

- [ ] **Step 5 : commit** `feat(core): sonde qui respecte robots.txt et lignes sondées marquées « non analysé »`

### Task 4 : tableau à colonnes choisies et export complet

**Files:**
- Create: `src/lib/columns.ts`, `src/lib/columns.test.ts`, `src/components/ColumnPicker.tsx`, `src/components/ColumnPicker.test.tsx`
- Modify: `crates/arpenteur-core/src/model.rs`, `checks.rs` (`build_rows`), `src/lib/types.ts`, `src/components/UrlTable.tsx`, `src/lib/csv.ts`, `src/lib/table.ts`, `src/i18n/fr.ts`, `src/styles.css` et leurs tests

**Interfaces:**
- Produces :

```rust
UrlRow { pub title_length: Option<u32>, pub meta_description_length: Option<u32>, pub elapsed_ms: u64, .. }
```

  Les longueurs sont en caractères après réduction des espaces, comme dans les contrôles (`collapsed`). Miroir TypeScript : `titleLength`, `metaDescriptionLength`, `elapsedMs`.

```ts
// src/lib/columns.ts
export type ColumnKey = "url" | "status" | "contentType" | "title" | "titleLength" | "metaDescription"
  | "metaDescriptionLength" | "h1" | "h1Count" | "canonical" | "indexable" | "depth" | "inlinks"
  | "wordCount" | "size" | "elapsedMs";
export const ALL_COLUMN_KEYS: ColumnKey[];      // les 16, dans cet ordre (ordre du CSV)
export const DEFAULT_COLUMN_KEYS: ColumnKey[];  // url, status, contentType, title, h1, indexable, depth, inlinks, wordCount
export function loadColumnKeys(): ColumnKey[];  // clé localStorage "arpenteur.columns"
export function saveColumnKeys(keys: ColumnKey[]): void;
// src/components/ColumnPicker.tsx
ColumnPicker({ selected, onChange }: { selected: ColumnKey[]; onChange: (keys: ColumnKey[]) => void })
```

  En-têtes du CSV, dans l'ordre : `url`, `statut`, `type`, `title`, `longueur du title`, `meta description`, `longueur de la meta description`, `h1`, `nombre de h1`, `canonical`, `indexable`, `profondeur`, `liens entrants`, `mots`, `poids (octets)`, `temps de réponse (ms)`.

- [ ] **Step 1 : écrire les tests**

```ts
test('le CSV des URL a 16 colonnes dans l’ordre de la conception', ...)
test('une valeur absente donne une cellule vide', ...)
test('les longueurs sont comptées après réduction des espaces', ...)   // côté Rust : « a   b » → 3
test('le tableau montre les 9 colonnes par défaut', ...)
test('cocher une colonne l’affiche et le choix est retenu', ...)       // localStorage relu au remontage
test('la colonne url ne peut pas être masquée', ...)
test('un choix enregistré invalide revient aux colonnes par défaut', ...)
    // Review Focus 5 : clé inconnue, JSON abîmé, getItem qui lève → DEFAULT_COLUMN_KEYS, pas d'erreur
test('le tri fonctionne sur poids et temps de réponse', ...)
test('poids et temps sont lisibles dans le tableau', ...)              // 86016 → « 84 Ko », 212 → « 212 ms »
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Le bouton « colonnes » ouvre une liste de cases à cocher, dans l'ordre de `ALL_COLUMN_KEYS`. Il porte `aria-expanded`, et la liste se ferme avec Échap.
- `gridTemplateColumns` est calculé à partir des colonnes affichées.
- Le CSV exporte toujours les 16 colonnes, quel que soit le choix du tableau.

- [ ] **Step 4 : vérifier** (`cargo test -p arpenteur-core && npm test && npx tsc --noEmit && npm run build`)

- [ ] **Step 5 : commit** `feat(ui): colonnes au choix dans le tableau et export CSV de 16 colonnes`

### Task 5 : vérification des liens externes (cœur)

**Files:**
- Create: `crates/arpenteur-core/src/external_links.rs`
- Modify: `crates/arpenteur-core/src/lib.rs`, `model.rs`, `canonical_probe.rs`, `engine.rs`, `defaults.rs`, `tests/crawl.rs`, `tests/support/mod.rs`, `tests/legacy_crawl.rs`

**Interfaces:**
- Consumes : `ProbeEnv`, `run_post_crawl_probes` (tâche 3).
- Produces :

```rust
// model.rs
#[serde(rename_all = "lowercase")]
pub enum ExternalOutcome { Ok, Broken, Unknown }
#[serde(rename_all = "camelCase")]
pub struct ExternalCheck {
    pub url: String,
    pub outcome: ExternalOutcome,
    pub status: Option<u16>,        // statut final ; None sans réponse
    pub final_url: Option<String>,  // renseignée si elle diffère de `url`
}
CollectedData { #[serde(default)] pub external_links: Vec<ExternalCheck>, #[serde(default)] pub external_unchecked: u32, .. }
CrawlSettings { #[serde(default = "default_true")] pub check_external_links: bool, .. }  // vrai par défaut
// external_links.rs
pub const MAX_REDIRECTS: usize = 5;
/// Adresses http(s) distinctes (normalisées) des liens `<a href>` des pages internes, hors du site, dans l'ordre de rencontre.
pub fn external_targets(pages: &[PageRecord], start: &Url) -> Vec<Url>;
pub fn external_client(user_agent: &str) -> Result<reqwest::Client, reqwest::Error>;
pub fn classify_status(status: u16) -> ExternalOutcome;   // < 400 → Ok ; 404, 410 → Broken ; le reste → Unknown
/// Renvoie les vérifications faites et le nombre de requêtes consommées.
pub async fn check_external_links(client: &reqwest::Client, targets: &[Url], budget: usize,
    min_interval: Duration, cancel: &CancellationToken) -> (Vec<ExternalCheck>, usize);
// canonical_probe.rs
PostCrawl { pub external: Vec<ExternalCheck>, pub external_unchecked: u32, .. }
```

  `run_post_crawl_probes` reçoit en plus `external_client: Option<&reqwest::Client>` (`None` quand le réglage est décoché) et enchaîne compression, canonical, liens externes sur un seul budget. `engine.rs` enregistre `external` et `external_unchecked` dans `CollectedData`.

- [ ] **Step 1 : écrire les tests**

  Le site de test écoute sur `127.0.0.1` : un lien vers `http://localhost:<port>/…` est donc externe. Ajouter une variante témoin dont la page d'accueil lie `/ext-ok`, `/ext-404`, `/ext-410`, `/ext-403`, `/ext-429`, `/ext-999`, `/ext-500`, `/ext-redirect` (302 vers `/ext-404`), `/ext-loop` (redirection sans fin), `/ext-hang` (ne répond jamais) sur `localhost`, et dont une seconde page relie `/ext-404`.

```rust
#[test] fn classify_status_boundaries()
    // 200, 301, 399 → Ok ; 404, 410 → Broken ; 400, 401, 403, 429, 500, 503, 999 → Unknown
#[test] fn external_targets_are_distinct_http_links_off_site()
    // même lien sur deux pages → une cible ; mailto:, tel:, lien interne, www du même site → exclus
#[tokio::test(flavor = "multi_thread")] async fn broken_only_on_404_410()
    // /ext-404 et /ext-410 → Broken ; /ext-redirect → Broken, status 404, final_url renseignée
    // /ext-403, /ext-429, /ext-999, /ext-500, /ext-loop, /ext-hang → Unknown   (Review Focus 2)
#[tokio::test(flavor = "multi_thread")] async fn each_external_url_is_requested_once()   // Review Focus 3
#[tokio::test(flavor = "multi_thread")] async fn unknown_domain_is_broken_after_a_second_try()
    // https://domaine-inexistant.invalid/ → Broken, status None, deux requêtes comptées
#[tokio::test(flavor = "multi_thread")] async fn refused_connection_is_unknown()         // port fermé
#[tokio::test(flavor = "multi_thread")] async fn external_checks_share_the_budget()
    // budget 3 avec 5 cibles → 3 vérifiées, external_unchecked == 2 ; budget 0 → aucune requête
#[tokio::test(flavor = "multi_thread")] async fn cancel_stops_external_checks()
    // jeton annulé pendant l'attente du débit → retour immédiat, les vérifications déjà faites sont gardées
#[tokio::test(flavor = "multi_thread")] async fn setting_off_makes_no_external_request()
#[tokio::test(flavor = "multi_thread")] async fn stopped_crawl_checks_no_external_link()
// defaults.rs / tests/legacy_crawl.rs
#[test] fn settings_without_check_external_links_default_to_true()   // Review Focus 1
#[test] fn crawl_without_external_links_loads()
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Une requête GET par cible, sans lire le corps, avec le User-Agent du crawl, délai de 5 secondes, redirections suivies jusqu'à `MAX_REDIRECTS`.
- Le robots.txt des sites tiers n'est pas lu : une seule requête par lien.
- Le débit réglé s'applique avant chaque requête, second essai compris, avec une attente sensible à l'annulation.
- Seul un échec de résolution du nom de domaine déclenche le second essai. Deux échecs de résolution donnent `Broken` ; toute autre erreur donne `Unknown`.
- Le délai de 5 secondes doit pouvoir être raccourci dans les tests, comme pour `probe_client_with_timeout`.

- [ ] **Step 4 : vérifier** (`cargo test -p arpenteur-core`, puis `cargo test -p arpenteur-core --test crawl` cinq fois de suite)

- [ ] **Step 5 : commit** `feat(core): vérification des liens externes après le crawl`

### Task 6 : « Lien externe cassé » dans le catalogue et l'interface

**Files:**
- Modify: `crates/arpenteur-core/src/model.rs`, `checks.rs`, `src/lib/finding-codes.json`, `src/lib/catalog.ts`, `src/lib/types.ts`, `src/i18n/fr.ts`, `src/components/UrlDetail.tsx`, `src/screens/Crawl.tsx`, `src/screens/Settings.tsx`, `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md`, `README.md` et leurs tests

**Interfaces:**
- Consumes : `ExternalCheck`, `CollectedData.external_links`, `external_unchecked`, `CrawlSettings.check_external_links` (tâche 5), `CheckInput` (tâche 1).
- Produces :
  - `FindingCode::BrokenExternalLink`, sérialisé `brokenExternalLink`, déclaré en dernier (38e nom de `finding-codes.json`) ;
  - `CheckInput.external_links: &'a [ExternalCheck]` (vide par défaut) ;
  - un constat par lien `Broken` : `url` = l'adresse externe, `related` = les pages internes qui la lient (triées), `detail` = `404`, `410` ou `domaine introuvable` ;
  - `CrawlResult.external_links: Vec<ExternalCheck>` et `CrawlResult.external_unchecked: u32`, avec leur miroir TypeScript ;
  - catalogue : niveau `moyenne`, titre « Lien externe cassé », pourquoi « Une page du site pointe vers une adresse d'un autre site qui n'existe plus. Le visiteur qui clique tombe sur une erreur. », correction « Corriger le lien ou le retirer. » La ligne est ajoutée au tableau « Moyenne » du document du catalogue, source « nous ».

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn broken_external_link_lists_its_source_pages()
    // lien Broken présent sur 3 pages internes → un constat, related = les 3 pages   (Review Focus 3)
#[test] fn ok_and_unknown_external_links_give_no_finding()
#[test] fn finding_codes_match_shared_list()   // existant : 38 noms
```

```ts
test('répartition validée : 7 haute, 15 moyenne, 12 basse, 4 info', ...)
test('brokenExternalLink reprend les textes de la conception', ...)
test('le détail d’une page liste ses liens externes cassés avec leur adresse', ...)
test('un lien sortant externe vérifié affiche son statut', ...)       // « 404 », « 200 », rien s'il n'a pas été vérifié
test('l’écran du crawl signale les liens externes non vérifiés', ...)
    // externalUnchecked 12 → « 12 liens externes non vérifiés (limite de requêtes atteinte) » ; 0 → rien
test('la case « vérifier les liens externes » est cochée par défaut et s’enregistre', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- Dans `UrlDetail`, `brokenExternalLink` passe par `related`, comme `brokenInternalLink` (« vers … »).
- Le README gagne une ligne dans « Ce que fait l'outil » : la vérification des liens externes, qui envoie une requête par lien aux sites tiers et se désactive dans les réglages.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit && npm run build`)

- [ ] **Step 5 : commit** `feat(app): problème « Lien externe cassé » et réglage associé`

### Task 7 : mode liste (cœur)

**Files:**
- Create: `crates/arpenteur-core/src/url_list.rs`
- Modify: `crates/arpenteur-core/src/lib.rs`, `model.rs`, `settings.rs`, `sitemap.rs`, `engine.rs`, `checks.rs`, `storage.rs`, `tests/crawl.rs`, `tests/support/mod.rs`, `tests/legacy_crawl.rs`, `src-tauri/src/commands.rs` (appel de `run_crawl`), `src/lib/types.ts`

**Interfaces:**
- Consumes : `UrlFilter`, `SkipReason` (tâche 3), `CheckInput` (tâche 1).
- Produces :

```rust
// model.rs
#[serde(rename_all = "lowercase")]
pub enum CrawlMode { #[default] Site, List }
CrawlMeta { #[serde(default)] pub mode: CrawlMode, #[serde(default)] pub list_urls: Vec<String>, .. }
#[serde(rename_all = "camelCase")]
pub struct SkippedUrl { pub url: String, pub reason: SkipReason }
CollectedData { #[serde(default)] pub list_skipped: Vec<SkippedUrl>, .. }
CrawlResult { pub list_skipped: Vec<SkippedUrl>, .. }
UrlRow { pub inlinks: Option<u32>, pub off_list: bool, .. }   // en mode liste : depth et inlinks à None
// url_list.rs
/// Une URL par ligne. Ignore lignes vides, espaces, BOM et doublons. `Err` = un message français par ligne fautive.
pub fn parse_url_list(input: &str) -> Result<Vec<Url>, Vec<String>>;
/// `Err` si la liste dépasse `settings.max_requests`.
pub fn check_list_size(urls: &[Url], settings: &CrawlSettings) -> Result<(), String>;
// engine.rs
pub enum CrawlTarget { Site(Url), List(Vec<Url>) }
pub async fn run_crawl(settings: CrawlSettings, target: CrawlTarget, store: CrawlStore,
    location: CrawlLocation, events: Arc<dyn CrawlEvents>, cancel: CancellationToken) -> CrawlMeta;
// checks.rs
CheckInput { pub list_mode: bool, .. }   // faux par défaut
```

  Messages de `parse_url_list` : `ligne 3 : « ftp://x » n'est pas une adresse http ou https`, `ligne 5 : « https://autre.fr/ » n'est pas sur le même site que la première adresse`, et `La liste est vide.` Message de `check_list_size` : `La liste compte 12 000 adresses, la limite de requêtes est de 10 000.` (avec les deux nombres réels). Message d'échec quand tout est écarté : `Toutes les adresses de la liste sont interdites par robots.txt ou exclues par les motifs.`

  En mode liste, `meta.start_url` est la première adresse de la liste et `meta.list_urls` la liste nettoyée. Miroir TypeScript : `mode: "site" | "list"`, `listUrls: string[]`, `listSkipped`, `inlinks: number | null`, `offList: boolean`.

- [ ] **Step 1 : écrire les tests**

```rust
// url_list.rs
#[test] fn parses_a_pasted_list()
    // "\u{feff}https://exemple.fr/\r\n\r\n  https://exemple.fr/tarifs  \r\nhttps://exemple.fr/\r\n"
    // → [https://exemple.fr/, https://exemple.fr/tarifs]   (Review Focus 4)
#[test] fn www_and_apex_are_the_same_site()
#[test] fn reports_faulty_lines_with_their_number()
    // ligne 2 "ftp://x", ligne 4 "https://autre.fr/" → deux messages, dans l'ordre
#[test] fn empty_list_is_refused()
#[test] fn list_larger_than_the_request_limit_is_refused()   // max_requests 2, 3 adresses → Err ; 2 adresses → Ok
// tests/crawl.rs
#[tokio::test(flavor = "multi_thread")] async fn list_mode_requests_only_listed_urls()
    // liste [/, /a] sur un site où / lie /b → /b jamais demandée, le sitemap n'est pas lu
#[tokio::test(flavor = "multi_thread")] async fn list_mode_follows_redirects_and_marks_them_off_list()
    // /old (301 vers /new) dans la liste → /new demandée, sa ligne a off_list == true
#[tokio::test(flavor = "multi_thread")] async fn list_mode_skips_disallowed_and_excluded_urls()
    // /prive (robots.txt) et /tag/x (motif) ne sont pas demandées, même en première ligne ;
    // list_skipped les cite avec leur raison
#[tokio::test(flavor = "multi_thread")] async fn list_mode_fails_when_everything_is_skipped()
#[tokio::test(flavor = "multi_thread")] async fn list_mode_runs_post_crawl_probes()
// checks.rs
#[test] fn list_mode_skips_site_wide_checks()
    // aucun orphanPage, notInSitemap, sitemapNotFound, sitemapUrlError, sitemapUrlNonIndexable ;
    // titleDuplicate entre deux pages de la liste → constat ; lignes : depth None, inlinks None ;
    // un lien vers une page hors liste, jamais demandée → pas de brokenInternalLink ;
    // securityHeadersMissing et modernCompressionMissing restent (constats de serveur, première adresse demandée)
#[test] fn site_mode_keeps_them()
// tests/legacy_crawl.rs
#[test] fn crawl_without_mode_is_a_site_crawl()   // Review Focus 1
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Le mode liste lit robots.txt sans lire le sitemap : extraire de `sitemap.rs` de quoi lire robots.txt seul.
- La liste est filtrée par `UrlFilter`. La première adresse autorisée devient l'URL de départ de SiteOne, les autres vont dans `url_list_urls`, et `single_page` est activé (`max_depth` ne suffit pas, voir `html_processor.rs` de SiteOne).
- `build_rows` reçoit le mode et la liste : `off_list` vaut vrai pour une ligne dont l'adresse normalisée n'est pas dans `meta.list_urls`.
- `commands.rs` appelle `run_crawl` avec `CrawlTarget::Site(start)` : rien d'autre ne change côté Tauri dans cette tâche.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npx tsc --noEmit`, puis `cargo test -p arpenteur-core --test crawl` cinq fois de suite)

- [ ] **Step 5 : commit** `feat(core): mode liste, une liste d'URL crawlée sans suivre les liens`

### Task 8 : mode liste (interface)

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, `src/screens/Home.tsx`, `src/screens/Crawl.tsx`, `src/components/UrlTable.tsx`, `src/lib/csv.ts`, `src/i18n/fr.ts`, `src/styles.css`, `README.md` et leurs tests

**Interfaces:**
- Consumes : `parse_url_list`, `CrawlTarget`, `CrawlMeta.mode`, `list_urls`, `CrawlResult.list_skipped`, `UrlRow.off_list` (tâche 7).
- Produces :
  - la commande `start_list_crawl(app, state, urls: String, settings: Option<CrawlSettings>) -> Result<StartedCrawl, String>` : elle valide les réglages comme `start_crawl`, appelle `parse_url_list` puis `check_list_size`, et renvoie leurs messages joints par un retour à la ligne, avant de prendre la place « crawl en cours » ; le crawl est rangé sous le site de la première adresse ;
  - `api.startListCrawl(urls: string): Promise<StartedCrawl>`.

- [ ] **Step 1 : écrire les tests**

```ts
test('l’accueil propose « site » et « liste d’URL »', ...)
test('en mode liste, le compteur suit les lignes non vides et « lancer » appelle startListCrawl', ...)
test('les lignes fautives renvoyées par le cœur s’affichent, une par ligne', ...)
test('une liste plus longue que la limite de requêtes est refusée avec son message', ...)
test('la liste des crawls affiche « liste, 3 URL »', ...)
test('une ligne hors liste porte la mention « hors liste »', ...)
test('l’écran du crawl dit combien d’adresses ont été écartées et pourquoi', ...)
    // 2 par robots.txt, 1 par un motif → « 3 adresses écartées : 2 par robots.txt, 1 par un motif »
test('en mode liste, profondeur et liens entrants affichent « – » et le CSV une cellule vide', ...)
test('« relancer » rejoue la liste', ...)   // startListCrawl(result.meta.listUrls.join("\n"))
```

  Et en Rust (`commands.rs`), sur une fonction pure extraite de la commande : `list_crawl_is_filed_under_the_first_url_site`.

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- Le choix « site » ou « liste d'URL » est un groupe de boutons radio avec un nom accessible. La zone de texte a un libellé associé.
- Le README gagne une ligne dans « Ce que fait l'outil » sur le mode liste.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit && npm run build && cargo build -p arpenteur`)

- [ ] **Step 5 : commit** `feat(app): mode liste sur l'accueil`

### Vérification finale (à la main, avec l'utilisateur)

Dans `npm run tauri dev` :
- rouvrir le crawl de la chocolaterie : il s'ouvre, sans nouveau constat ;
- crawler crocweb.fr : « Lien externe cassé » n'apparaît que sur des liens réellement morts (les ouvrir dans un navigateur), et aucun lien vers LinkedIn ou Facebook n'est signalé ;
- le bouton « colonnes » : cocher « meta description » et « temps », relancer l'app, le choix est gardé ;
- l'export des URL ouvert dans Excel ou Numbers : 16 colonnes ;
- le mode liste avec trois adresses collées depuis un tableur, dont une qui redirige : la cible apparaît « hors liste » ;
- décocher « vérifier les liens externes », relancer un crawl : le problème n'apparaît plus.
