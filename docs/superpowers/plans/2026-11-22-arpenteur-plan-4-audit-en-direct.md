# Arpenteur, plan 4 : contrôles manquants, type et thème, vue d'ensemble, résultats en direct, robots.txt Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ajouter sept contrôles (catalogue à 45 codes), donner à chaque problème un type et un thème, afficher une vue d'ensemble, remplir le tableau et le panneau pendant le crawl, et rendre robots.txt consultable.

**Architecture:** le cœur Rust reste la seule source des constats : les nouveaux contrôles entrent dans `run_checks`, la sonde « adresse inexistante » rejoint la phase après crawl, et un mode partiel de `CheckInput` sert aux instantanés pris pendant le crawl. Un emplacement partagé (`LiveCrawl`) donne à l'app Tauri l'accès aux pages en cours de collecte. Le type et le thème vivent dans le catalogue de l'interface (`src/lib/catalog.ts`), à côté du niveau. L'interface interroge un instantané toutes les 2 secondes et réutilise le panneau et le tableau existants.

**Tech Stack:** Rust (`arpenteur-core`, `siteone-crawler =2.5.1`, `reqwest`, `tokio`), Tauri 2, React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-22-arpenteur-plan-4-design.md` (fait foi). Contexte : `docs/superpowers/plans/2026-11-21-arpenteur-plan-3-bilan.md`, catalogue `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md`.

**Branche :** `feature/plan-4-audit-en-direct`, créée depuis `main`, sans worktree. Jamais de commit de code sur `main`.

## Global Constraints

- Tout nouveau champ enregistré (`crawl.json`, `arpenteur.json`, `settings.json`) porte `#[serde(default)]`. Un crawl enregistré avant le plan 4 s'ouvre sans erreur. Il peut gagner `h1Duplicate` et `h1TooLong` (ses H1 sont enregistrés), jamais `canonicalMultiple`, `soft404`, `soft404Site` ni `robotsTxtMissing`.
- Règle D5 : chaque contrôle a un test positif et un test négatif sur sa frontière. Un faux positif coûte plus qu'un contrôle manquant.
- Toute requête ajoutée respecte le débit réglé et compte dans la limite de requêtes. Ordre dans le budget restant : compression, adresse inexistante (soft 404), cibles de canonical, liens externes.
- Le catalogue validé fait foi. Il passe à 45 codes : 10 en haute, 16 en moyenne, 15 en basse, 4 en information. Les textes des sept nouvelles entrées, et le type et le thème des 45 codes, sont copiés à l'identique depuis la conception. Les textes existants ne changent pas.
- Seuil : un H1 de plus de 70 caractères est trop long (70 ne donne rien, 71 est signalé), espaces réduits.
- Délai maximal d'une requête de sonde : 5 secondes. Redirections suivies pour l'adresse inexistante : 5 au plus. Corps lu : 1 Mo au plus (1 048 576 octets).
- Instantané pendant le crawl : toutes les 2 secondes ; si un instantané met plus de 500 ms, l'intervalle suivant vaut quatre fois sa durée. Jamais deux instantanés en même temps.
- Interface : textes dans `fr.ts`, habillage Console (variables de `styles.css`, aucune couleur codée en dur hors `:root`, pas de style en ligne hors virtualisation et `gridTemplateColumns`). Aucun tiret cadratin dans un texte ou un document.
- SiteOne reste en `=2.5.1`. Ne jamais pousser ni créer de dépôt distant sans l'accord de l'utilisateur.

## Review Focus

1. **Un crawl enregistré avant le plan 4** (faits en version 2, sans `canonicals`, sans sonde d'adresse inexistante, sans statut de robots.txt) s'ouvre sans erreur et sans aucun des quatre constats qui demandent ces données. Tests dans les tâches 1 et 2 (`tests/legacy_crawl.rs`).
2. **Un site dont toutes les pages partagent le title et le H1 de sa page d'erreur** (thème qui met le nom du site partout), et un article qui parle des erreurs 404 : aucun `soft404`. Test dans la tâche 2.
3. **Un instantané demandé au mauvais moment** (avant la première page, pendant les sondes de fin, juste après la fin) ne donne ni erreur ni tableau vidé ; un instantané lent n'en lance pas un second. Tests dans la tâche 5.
4. **Un classement enregistré devenu invalide** (valeur inconnue, stockage indisponible) revient à « priorité », sans erreur. Test dans la tâche 4.
5. **Un rapport SiteOne inattendu** (table `skipped` absente, autre raison, adresse déjà absolue, adresse d'un autre hôte) : la vue robots.txt s'affiche, sans ligne fausse. Test dans la tâche 6.

---

### Task 1 : quatre contrôles sur les données déjà lues

**Files:**
- Modify: `crates/arpenteur-core/src/model.rs`, `html.rs`, `checks.rs`, `tests/legacy_crawl.rs`
- Modify: `src/lib/finding-codes.json`, `src/lib/types.ts`, `src/lib/catalog.ts`, `src/i18n/fr.ts`, `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md` et leurs tests

**Interfaces:**
- Produces :

```rust
// model.rs
pub const FACTS_VERSION: u32 = 3;
PageFacts { #[serde(default)] pub canonicals: Vec<String>, .. }  // toutes les balises canonical, résolues, dans l'ordre ; `canonical` reste la première
// FindingCode, déclarés à la suite dans cet ordre (noms 39 à 42 de finding-codes.json) :
H1Duplicate, H1TooLong, CanonicalMultiple, ListedUrlError   // h1Duplicate, h1TooLong, canonicalMultiple, listedUrlError
// checks.rs
pub const H1_MAX_CHARS: usize = 70;
```

  Niveaux : `listedUrlError` haute, `canonicalMultiple` moyenne, `h1Duplicate` et `h1TooLong` basse. Après cette tâche le catalogue compte 42 codes : 8 haute, 16 moyenne, 14 basse, 4 information. Titres et textes : conception, section 1. Les quatre lignes sont ajoutées au document du catalogue, source « nous ».

- [ ] **Step 1 : écrire les tests**

```rust
// html.rs
#[test] fn extracts_every_canonical_in_order()
    // deux <link rel="canonical"> → canonicals == [a, b], canonical == Some(a) ; une seule → [a] ; aucune → []
// checks.rs
#[test] fn h1_too_long_boundary()            // 70 caractères → rien ; 71 → h1TooLong ; « a   b » compté 3 ; page en noindex → rien
#[test] fn h1_duplicate_among_indexable_pages()
    // même premier H1 sur 2 pages indexables → h1Duplicate sur chacune (related = l'autre) ;
    // l'une en noindex → rien ; H1 vides → rien ; mode liste : page hors périmètre → rien
#[test] fn canonical_multiple_needs_two_tags_and_new_facts()
    // canonicals [a, b] en version 3 → canonicalMultiple, detail == "a, b" ; [a] → rien ;
    // [a, a] → constat (deux balises) ; version 2 → rien
#[test] fn listed_url_error_only_in_list_mode()
    // liste [/a (404), /b (200), /c (301 vers /d en 500), /e (sans réponse), /f (écartée, sans ligne)] →
    // listedUrlError sur /a (detail "404"), /c (detail "redirige vers …/d (500)"), /e (detail "sans réponse") ;
    // rien sur /b ni /f ; mode site → jamais ce code ; crawl limité : 429 et 503 → rien
// tests/legacy_crawl.rs
#[test] fn plan_3_crawl_gains_no_canonical_multiple()   // Review Focus 1 : faits en version 2
```

```ts
test('répartition : 8 haute, 16 moyenne, 14 basse, 4 info', ...)
test('les quatre nouvelles entrées reprennent les textes de la conception', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- `h1Duplicate` suit `titleDuplicate` en tout : pages indexables, périmètre du mode liste, comparaison après réduction des espaces.
- `listedUrlError` suit les redirections avec les données de `links.rs`, comme le périmètre du mode liste.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit`)

- [ ] **Step 5 : commit** `feat(core): H1 en double ou trop long, plusieurs canonical, adresse de la liste en erreur`

### Task 2 : soft 404 et robots.txt introuvable

**Files:**
- Create: `crates/arpenteur-core/src/not_found_probe.rs`
- Modify: `crates/arpenteur-core/src/lib.rs`, `model.rs`, `canonical_probe.rs`, `engine.rs`, `sitemap.rs`, `checks.rs`, `tests/crawl.rs`, `tests/support/mod.rs`, `tests/legacy_crawl.rs`
- Modify: `src/lib/finding-codes.json`, `src/lib/types.ts`, `src/lib/catalog.ts`, `src/i18n/fr.ts`, le document du catalogue et leurs tests

**Interfaces:**
- Consumes : `ProbeEnv`, `run_post_crawl_probes`, `external_links::external_client` (suit 5 redirections, délai de 5 s), `html::extract_page`, `site_finding_url`.
- Produces :

```rust
// model.rs
#[serde(rename_all = "camelCase")]
pub struct NotFoundProbe {
    pub status: u16,             // statut final
    pub redirected: bool,        // vrai si l'adresse finale diffère de l'adresse demandée
    pub final_url: String,
    pub title: Option<String>,
    pub h1: Option<String>,      // premier H1
    pub text_hash: String,
}
CollectedData { #[serde(default)] pub not_found_probe: Option<NotFoundProbe>, .. }
SitemapResult { #[serde(default)] pub robots_status: Option<u16>, .. }  // statut reçu pour /robots.txt ; None sans réponse
// FindingCode, à la suite (noms 43 à 45) : Soft404, Soft404Site, RobotsTxtMissing   // soft404, soft404Site, robotsTxtMissing
// not_found_probe.rs
pub const NOT_FOUND_BODY_LIMIT: usize = 1_048_576;
pub fn not_found_url(start: &Url, token: &str) -> Url;   // <origine>/arpenteur-introuvable-<token>
pub async fn probe_not_found(client: &reqwest::Client, url: &Url, cancel: &CancellationToken) -> Option<NotFoundProbe>;
// canonical_probe.rs
PostCrawl { pub not_found: Option<NotFoundProbe>, .. }
pub async fn run_post_crawl_probes(env: &ProbeEnv<'_>, pages: &[PageRecord], max_requests: usize,
    follow_client: &reqwest::Client, check_external_links: bool) -> PostCrawl;
// checks.rs
CheckInput { pub not_found: Option<&'a NotFoundProbe>, .. }   // None par défaut
```

  Niveaux : `soft404` et `soft404Site` haute, `robotsTxtMissing` basse. Le catalogue atteint 45 codes : 10 haute, 16 moyenne, 15 basse, 4 information. Titres et textes : conception, section 1.

- [ ] **Step 1 : écrire les tests**

  Variantes témoins à ajouter au site de test : adresses inexistantes servies en 200 avec la page d'erreur ; adresses inexistantes redirigées vers l'accueil ; vraie 404 avec une page crawlée en 200 qui a le même title et le même H1 ; robots.txt en 404, en 410, en 500.

```rust
#[tokio::test(flavor = "multi_thread")] async fn site_answering_200_to_unknown_urls()
    // soft404Site sur l'URL de départ, detail "répond 200" ; l'adresse inexistante est demandée une fois
#[tokio::test(flavor = "multi_thread")] async fn site_redirecting_unknown_urls_to_home()
    // soft404Site, detail "redirige vers <accueil>" ; aucun soft404 de page
#[tokio::test(flavor = "multi_thread")] async fn page_with_the_error_page_signature_is_a_soft_404()
    // vraie 404 pour l'adresse inexistante ; /fantome en 200 avec le même title et le même H1 → soft404 sur /fantome,
    // pas de soft404Site ; /article-404 (« Comprendre l'erreur 404 », autre H1) → rien          (Review Focus 2)
#[test] fn soft_404_needs_title_and_h1_or_the_same_text()
    // même title, autre H1 → rien ; title ou H1 vide des deux côtés → rien ; même empreinte de texte → constat
#[test] fn signature_shared_by_most_pages_is_not_a_soft_404()
    // 3 pages sur 4 ont le title et le H1 de la page d'erreur → aucun soft404                    (Review Focus 2)
#[tokio::test(flavor = "multi_thread")] async fn not_found_probe_shares_the_budget_and_respects_robots()
    // ordre des requêtes : compression, adresse inexistante, canonical, liens externes ; budget 1 → pas de sonde ;
    // adresse interdite par robots.txt → pas de sonde ; crawl arrêté → pas de sonde ; délai dépassé → None, aucun constat
#[test] fn robots_txt_missing_only_on_404_or_410()
    // robots_status Some(404) et Some(410) → robotsTxtMissing ; Some(500), Some(200), None → rien
// tests/legacy_crawl.rs
#[test] fn plan_3_crawl_gains_no_soft_404_nor_robots_finding()   // Review Focus 1
```

```ts
test('répartition validée : 10 haute, 16 moyenne, 15 basse, 4 info', ...)
test('les trois nouvelles entrées reprennent les textes de la conception', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Le jeton de l'adresse inexistante : 16 caractères hexadécimaux tirés au hasard à chaque crawl.
- La sonde n'envoie pas l'en-tête `Accept-Encoding` de la sonde de compression. Elle lit le corps jusqu'à `NOT_FOUND_BODY_LIMIT`, puis passe par `extract_page`.
- `soft404` : page interne, HTML, en 200, dont le title et le premier H1 (espaces réduits, tous deux non vides) égalent ceux de la sonde, ou dont l'empreinte de texte égale la sienne. Seulement si la sonde n'a pas été redirigée. Si plus de la moitié des pages HTML internes en 200 ont cette signature, aucune n'est signalée.
- `soft404Site` et `robotsTxtMissing` sont des constats de site, portés par `site_finding_url`.
- `robots_status` est rempli par la découverte du sitemap et par la lecture de robots.txt du mode liste.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit`, puis `cargo test -p arpenteur-core --test crawl` cinq fois de suite)

- [ ] **Step 5 : commit** `feat(core): fausses pages d'erreur (soft 404) et robots.txt introuvable`

### Task 3 : type et thème de chaque problème

**Files:**
- Modify: `src/lib/catalog.ts`, `src/i18n/fr.ts`, `src/components/ProblemsPanel.tsx`, `src/components/UrlDetail.tsx`, `src/lib/csv.ts`, `src/styles.css`, le document du catalogue et leurs tests

**Interfaces:**
- Produces :

```ts
export type Kind = "probleme" | "avertissement" | "opportunite";
export const KINDS: Kind[];     // dans cet ordre
export type Theme = "reponses" | "liens" | "titles" | "metaDescriptions" | "headings"
  | "canonical" | "indexation" | "contenu" | "images" | "technique";
export const THEMES: Theme[];   // dans cet ordre, celui du tableau de la conception
export const CATALOG: Record<FindingCode, { level: Level; kind: Kind; theme: Theme }>;
export function kindLabel(kind: Kind): string;     // « problème », « avertissement », « opportunité »
export function themeLabel(theme: Theme): string;  // « réponses et redirections », « liens », « titles », « meta descriptions »,
                                                   // « titres H1 à H6 », « canonical », « indexation et sitemap », « contenu », « images », « technique »
```

  Le type et le thème de chacun des 45 codes : tableau de la conception, section 2. Le CSV des problèmes gagne une colonne « type » entre « niveau » et « code » (7 colonnes). Le document du catalogue gagne la définition des trois types et le tableau des types et des thèmes.

- [ ] **Step 1 : écrire les tests**

```ts
test('chaque code a le type et le thème de la conception', ...)
    // les 45 couples (type, thème) écrits en clair dans le test, un par code
test('chaque thème et chaque type a son libellé', ...)
test('le panneau affiche le type sur chaque ligne de problème', ...)
test('le détail d’une URL affiche le type à côté du niveau', ...)
test('le CSV des problèmes a une colonne type entre niveau et code', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- L'étiquette de type est un texte discret (ton `dim`), pas une couleur de plus : la couleur reste celle du niveau.

- [ ] **Step 4 : vérifier** (`npm test && npx tsc --noEmit && npm run build`)

- [ ] **Step 5 : commit** `feat(ui): type et thème de chaque problème`

### Task 4 : vue d'ensemble

**Files:**
- Create: `src/lib/overview.ts`, `src/lib/overview.test.ts`
- Modify: `src/components/ProblemsPanel.tsx`, `src/components/UrlTable.tsx`, `src/screens/Crawl.tsx`, `src/i18n/fr.ts`, `src/styles.css` et leurs tests

**Interfaces:**
- Consumes : `Kind`, `Theme`, `THEMES`, `CATALOG` (tâche 3), `StatusClass` et `filterRows` (`src/lib/table.ts`).
- Produces :

```ts
// src/lib/overview.ts
export type Grouping = "priorite" | "theme";
export function crawlSummary(rows: UrlRow[]): { urls: number; byStatus: Record<"2xx" | "3xx" | "4xx" | "5xx" | "error", number>; indexable: number };
export function loadGrouping(): Grouping;               // clé localStorage "arpenteur.grouping", « priorite » par défaut
export function saveGrouping(grouping: Grouping): void;
// ProblemsPanel
ProblemsPanel({ findings, rows, selectedCode, onSelect, statusClass, onStatusClass }: {
  findings: Finding[]; rows: UrlRow[];
  selectedCode: FindingCode | null; onSelect: (code: FindingCode | null) => void;
  statusClass: StatusClass; onStatusClass: (c: StatusClass) => void;
})
// UrlTable : le filtre de statut devient pilotable
UrlTable({ ..., statusClass, onStatusClass }: { statusClass: StatusClass; onStatusClass: (c: StatusClass) => void })
```

  `crawlSummary` compte les lignes du tableau, pour que les nombres du résumé égalent ce que montre le tableau une fois filtré. Les classes de statut sont celles de `table.ts`.

- [ ] **Step 1 : écrire les tests**

```ts
test('le résumé compte les URL par réponse et les pages indexables', ...)
    // 95 lignes dont 88 en 2xx, 4 en 3xx, 3 en 4xx, 0 en 5xx, 0 sans réponse, 71 indexables
test('un clic sur « 4xx » règle le filtre de statut du tableau, un second clic le retire', ...)
test('le classement par thème regroupe les problèmes sous leur thème, avec le compteur du thème', ...)
test('un thème sans problème n’apparaît pas', ...)
test('les codes de niveau information restent à part, hors du total, dans les deux classements', ...)
test('le classement choisi est retenu', ...)
test('un classement enregistré invalide revient à priorité', ...)   // Review Focus 4 : valeur inconnue, getItem qui lève
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- Le choix « priorité | thème » est un groupe de deux boutons avec `aria-pressed`.
- Les lignes du résumé sont des boutons pour les classes de statut, du texte pour « URL » et « indexables ».
- Dans le classement par thème, l'ordre des thèmes est celui de `THEMES`. Dans un thème, les problèmes sont rangés par niveau puis par nombre.

- [ ] **Step 4 : vérifier** (`npm test && npx tsc --noEmit && npm run build`)

- [ ] **Step 5 : commit** `feat(ui): vue d'ensemble du crawl et classement des problèmes par thème`

### Task 5 : résultats pendant le crawl

**Files:**
- Create: `crates/arpenteur-core/src/live.rs`
- Modify: `crates/arpenteur-core/src/lib.rs`, `engine.rs`, `checks.rs`, `tests/crawl.rs`, `src-tauri/src/state.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, `src/lib/types.ts`, `src/screens/Crawl.tsx`, `src/components/UrlTable.tsx`, `src/i18n/fr.ts`, `src/styles.css` et leurs tests

**Interfaces:**
- Consumes : `CollectorHandle::pages()`, `CheckInput`, `build_rows`, `run_crawl`.
- Produces :

```rust
// live.rs
#[derive(Clone, Default)]
pub struct LiveCrawl { /* emplacement partagé : poignée du collecteur, sitemap, méta du crawl en cours */ }
impl LiveCrawl {
    pub fn new() -> LiveCrawl;
    /// `None` tant qu'aucun crawl ne collecte, et dès que la collecte est finie.
    pub fn snapshot(&self) -> Option<LiveView>;
}
#[serde(rename_all = "camelCase")]
pub struct LiveView { pub meta: CrawlMeta, pub rows: Vec<UrlRow>, pub findings: Vec<Finding> }
// engine.rs
pub async fn run_crawl(settings: CrawlSettings, target: CrawlTarget, store: CrawlStore, location: CrawlLocation,
    events: Arc<dyn CrawlEvents>, cancel: CancellationToken, live: LiveCrawl) -> CrawlMeta;
// checks.rs
CheckInput { pub partial: bool, .. }   // faux par défaut
```

  En mode partiel, `run_checks` ne produit pas `orphanPage`, ni un constat de canonical dont la cible n'a pas encore de ligne. `LiveView.meta` porte le statut « en cours » et le nombre de pages crawlées. Côté Tauri : `AppState` garde un `LiveCrawl`, et la commande `peek_crawl(state) -> Option<LiveView>` le lit sans bloquer la fenêtre. Côté interface : `api.peekCrawl(): Promise<LiveView | null>`.

- [ ] **Step 1 : écrire les tests**

```rust
// checks.rs
#[test] fn partial_mode_hides_orphans_and_unresolved_canonicals()
    // partiel : page du sitemap sans lien entrant → pas d'orphanPage ; canonical vers une cible sans ligne → aucun constat de canonical ;
    // canonical vers une cible connue en 301 → canonicalToRedirect ; titleMissing reste ; non partiel → orphanPage et canonicalToOtherUrl
// tests/crawl.rs (variante lente du site de test)
#[tokio::test(flavor = "multi_thread")] async fn snapshot_during_the_crawl_shows_collected_pages()
    // pendant le crawl : Some(vue), rows non vide et plus courte qu'à la fin, meta.status == Running, meta.pages == rows crawlées ;
    // le sitemap est connu de l'instantané (pas de sitemapNotFound à tort)
#[tokio::test(flavor = "multi_thread")] async fn snapshot_is_none_outside_the_collection()
    // avant le lancement, pendant les sondes de fin et après la fin → None      (Review Focus 3)
#[tokio::test(flavor = "multi_thread")] async fn snapshot_does_not_slow_the_crawl()
    // 50 instantanés pendant un crawl : toutes les pages sont collectées, aucune perdue
```

```ts
test('pendant le crawl, le panneau et le tableau se remplissent et « résultats provisoires » s’affiche', ...)
test('un instantané est demandé toutes les 2 secondes', ...)                  // minuteries simulées
test('un instantané de 800 ms repousse le suivant à 3,2 secondes', ...)
test('un instantané encore en cours n’en lance pas un second', ...)           // Review Focus 3
test('un instantané nul ou en erreur garde le dernier affichage', ...)        // Review Focus 3 : le tableau n'est pas vidé
test('le filtre de problème, la recherche et le tri survivent à un nouvel instantané', ...)
test('un clic sur une ligne pendant le crawl dit que le détail s’ouvre une fois le crawl terminé', ...)
test('à la fin du crawl, le résultat complet remplace l’instantané sans changer de filtre', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Le moteur alimente `LiveCrawl` dès que la collecte commence (poignée, sitemap lu, méta) et le vide quand elle finit, avant les sondes.
- `snapshot` copie les pages sous le verrou du collecteur, puis calcule lignes et constats hors du verrou.
- La commande `peek_crawl` s'exécute hors du fil principal de la fenêtre.
- L'interface garde montés le panneau et le tableau entre deux instantanés : seules leurs données changent. Le choix des colonnes et la position de défilement sont donc gardés.
- Les boutons d'export restent cachés tant que le crawl tourne, comme aujourd'hui.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit && npm run build && cargo build -p arpenteur`, puis `cargo test -p arpenteur-core --test crawl` cinq fois de suite)

- [ ] **Step 5 : commit** `feat(app): tableau et problèmes pendant le crawl`

### Task 6 : robots.txt consultable

**Files:**
- Create: `src/components/RobotsView.tsx`, `src/components/RobotsView.test.tsx`
- Modify: `crates/arpenteur-core/src/model.rs`, `siteone.rs`, `sitemap.rs`, `checks.rs` (`assemble`), `storage.rs`, `src/lib/types.ts`, `src/screens/Crawl.tsx`, `src/i18n/fr.ts`, `src/styles.css`, `README.md` et leurs tests

**Interfaces:**
- Consumes : `SitemapResult.robots_txt`, `robots_status` (tâche 2), `CollectedData.list_skipped`, `CrawlMeta.settings.respect_robots`.
- Produces :

```rust
// model.rs
#[serde(rename_all = "camelCase")]
pub struct BlockedUrl { pub url: String, pub found_on: Option<String> }
#[derive(Default)] #[serde(rename_all = "camelCase")]
pub struct RobotsReport {
    pub content: Option<String>,        // le fichier tel qu'il a été lu
    pub status: Option<u16>,
    pub sitemaps: Vec<String>,          // directives « Sitemap: » du fichier
    pub blocked: Vec<BlockedUrl>,
    pub blocked_known: bool,            // faux quand le rapport SiteOne manque (crawl arrêté ou limité)
    pub respected: bool,                // réglage « respecter robots.txt » du crawl
}
CrawlResult { #[serde(default)] pub robots: RobotsReport, .. }
// siteone.rs
SiteOneSummary { pub robots_blocked: Vec<BlockedUrl>, .. }
pub fn read_siteone(json: &str, start_url: &str) -> Option<SiteOneSummary>;
```

  Dans le rapport SiteOne, les adresses bloquées sont les lignes de la table `skipped` dont la raison est `Robots.txt`. Leurs colonnes `url` et `sourceUqId` sont écrites sans le nom du site quand il s'agit de l'hôte de départ : elles sont rendues absolues avec l'origine de `start_url`. Composant : `RobotsView({ robots, onClose }: { robots: RobotsReport; onClose: () => void })`, ouvert par un bouton « robots.txt » sur l'écran d'un crawl terminé.

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn reads_robots_blocked_urls_from_the_skipped_table()
    // { reason: "Robots.txt", url: "/prive?x=1", sourceUqId: "/" } avec start https://exemple.fr/ →
    // BlockedUrl { url: "https://exemple.fr/prive?x=1", found_on: Some("https://exemple.fr/") }
#[test] fn ignores_other_reasons_and_tolerates_odd_rows()
    // "Not allowed host", "Max depth" → ignorées ; table absente → liste vide ; adresse déjà absolue → gardée telle quelle ;
    // ligne sans url → ignorée                                                   (Review Focus 5)
#[test] fn robots_report_lists_declared_sitemaps()
    // "Sitemap: https://exemple.fr/sitemap.xml" (casse libre, espaces) → sitemaps == [cette adresse] ; fichier absent → []
#[test] fn robots_report_without_siteone_json_says_blocked_unknown()
    // siteone None → blocked_known == false ; siteone Some avec zéro ligne → blocked_known == true, blocked vide
#[test] fn list_mode_adds_addresses_skipped_by_robots()
    // list_skipped (raison robots) → BlockedUrl sans found_on ; raison motif → pas dans la liste
```

```ts
test('la vue affiche le contenu du fichier, ses sitemaps et les adresses bloquées avec leur page source', ...)
test('un fichier absent affiche « introuvable »', ...)
test('sans rapport SiteOne, la vue dit que la liste est disponible pour un crawl terminé', ...)
test('la vue rappelle que les fichiers (images, scripts) ne sont pas listés', ...)
test('si robots.txt n’est pas respecté, la vue dit que rien n’a été bloqué', ...)
test('le bouton « robots.txt » n’apparaît que pour un crawl chargé et terminé', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**

- Le contenu du fichier s'affiche en texte préformaté, jamais interprété.
- Le README gagne une ligne dans « Ce que fait l'outil » sur la vue robots.txt, et la ligne des contrôles dit 45.

- [ ] **Step 4 : vérifier** (`cargo test --workspace && npm test && npx tsc --noEmit && npm run build && cargo build -p arpenteur`)

- [ ] **Step 5 : commit** `feat(app): robots.txt consultable, avec les adresses bloquées`

### Vérification finale (à la main, avec l'utilisateur)

Dans `npm run tauri dev` :
- lancer un crawl de crocweb.fr : le tableau et le panneau se remplissent, « résultats provisoires » s'affiche, un clic sur une ligne dit que le détail attend la fin ;
- à la fin : le filtre choisi pendant le crawl est gardé, et « Page orpheline » n'apparaît qu'à ce moment ;
- le panneau : le résumé en haut, un clic sur « 4xx » filtre le tableau, le classement « thème » est gardé après un redémarrage ;
- chaque problème porte son type, dans le panneau, le détail et le CSV ;
- rouvrir le crawl de la chocolaterie : il s'ouvre, sans « soft 404 » ni « robots.txt introuvable » ;
- crawler un site qui renvoie 200 pour une adresse inexistante, s'il y en a un sous la main : « Le site répond 200 aux adresses inexistantes » ;
- le bouton « robots.txt » : le fichier, ses sitemaps, les adresses bloquées ;
- le mode liste avec une adresse en 404 : « Adresse de la liste en erreur ».
