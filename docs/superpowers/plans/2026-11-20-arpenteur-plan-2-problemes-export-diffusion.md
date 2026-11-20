# Arpenteur, plan 2 : problèmes, export, réglages et diffusion (jalons 3 à 5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** afficher les 37 problèmes du catalogue validé dans un panneau qui filtre le tableau, exporter en CSV, régler le crawl, vérifier les cibles de canonical, arrêter proprement à la fermeture, et préparer la diffusion (À propos, README, CI de release).

**Architecture:** le cœur Rust calcule tous les constats (`Finding`) à l'ouverture d'un crawl, à partir des pages collectées, plus deux constats de site tirés du JSON de SiteOne (certificat, en-têtes de sécurité). L'interface porte le catalogue : niveau et textes de chaque code, dans `src/lib/catalog.ts` et `src/i18n/fr.ts`. Une liste de codes partagée (`src/lib/finding-codes.json`), vérifiée des deux côtés, garantit que Rust et TypeScript parlent des mêmes codes.

**Tech Stack:** Rust (crate `arpenteur-core`, `regex`, `reqwest`), Tauri 2 (`tauri-plugin-dialog`), React 19 + TypeScript, Vitest, GitHub Actions (`tauri-apps/tauri-action`).

**Specs:**
- `docs/superpowers/specs/2026-11-20-arpenteur-design.md` (D5, D8, D9, D10, §5 et §6)
- `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md` (catalogue validé : codes, niveaux, textes)
- `docs/superpowers/specs/2026-11-20-arpenteur-design-console.md` (habillage)
- `docs/superpowers/plans/2026-11-20-arpenteur-plan-1-bilan.md` (ce qui ouvre le plan 2)

## Décisions de ce plan (à relire avec le plan)

- **Presque tous les contrôles sont calculés par nous, pas repris de SiteOne.** Le JSON de SiteOne ne donne, pour l'accessibilité et les bonnes pratiques, que des compteurs par analyse, sans les URL concernées. Les titles et descriptions dupliqués, l'alt des images, la structure des titres, la langue, le téléphone non cliquable et Brotli sont donc calculés à partir des pages, que nous avons déjà. Effet de bord heureux : un crawl arrêté a quand même ces problèmes. Seuls le certificat HTTPS et les en-têtes de sécurité viennent de SiteOne, comme constats de site.
- **`httpsInvalid` ne signale qu'un certificat en erreur ou expiré, pas « bientôt expiré ».** Derrière un proxy qui inspecte le TLS (Zscaler sur ton poste : le crawl de la chocolaterie montre un certificat Zscaler valide 7 jours), SiteOne voit le certificat du proxy. Un seuil « bientôt expiré » donnerait un faux positif sur chaque site. Le titre devient « Certificat HTTPS invalide ». Le détail du constat cite l'émetteur.
- **Les textes du catalogue gardent leur casse** (« H1 », « JSON-LD », « HTTPS ») : c'est du contenu destiné aux rapports, pas un libellé d'interface.
- **CSV pour Excel en français** : séparateur `;`, encodage UTF-8 avec BOM, fins de ligne CRLF.
- **Réglages** : valeurs par défaut persistées dans `<app_data_dir>/settings.json`, bornes : connexions 1 à 8, débit 0,1 à 20 requêtes par seconde, limite 1 à 10 000 requêtes, seuil de limitation 1 à 50, regex compilées avant enregistrement.
- **Tags de release sans `v`** (`0.2.0`), comme dans tes autres dépôts. Pas de signature payante (D9).

## Global Constraints

- Le catalogue validé fait foi : 37 codes, 7 en haute, 14 en moyenne, 12 en basse, 4 en information. Textes copiés à l'identique depuis le document du catalogue, sauf le titre de `httpsInvalid` (« Certificat HTTPS invalide »).
- Règle D5 contre les faux positifs : chaque nouveau contrôle a un test positif et un test négatif sur sa frontière.
- Seuils : title moins de 30 ou plus de 60 caractères ; meta description moins de 70 ou plus de 160 ; image lourde au-delà de 204 800 octets ; contenu court sous 200 mots.
- Constats sur les pages internes (même site que l'URL de départ) uniquement, comme au plan 1.
- Compatibilité des crawls déjà enregistrés : tout nouveau champ sérialisé porte `#[serde(default)]`.
- Interface : textes dans `fr.ts`, habillage Console (variables de `styles.css`, aucune couleur codée en dur hors `:root`, pas de style en ligne hors virtualisation et `gridTemplateColumns`).
- Débit : toute requête supplémentaire (vérification des canonical) respecte le débit réglé et compte dans la limite de requêtes.
- Ne jamais pousser ni créer de dépôt distant sans l'accord de l'utilisateur.

## Review Focus

1. **Un ancien crawl enregistré (plan 1) s'ouvre toujours**, sans les nouveaux champs : pas d'erreur de désérialisation, les nouveaux contrôles tournent sur ce qui existe. Test dans la tâche 1.
2. **Faux positifs sur des pages légitimes** : une page contact sans H2, un numéro de SIRET (14 chiffres) ou un prix qui ressemble à un numéro, une image décorative avec `alt=""`. Aucun constat. Tests dans les tâches 1 et 2.
3. **Clic sur un constat de site** (certificat, en-têtes, Brotli, sitemap absent) : le filtre du tableau montre l'URL de départ, sans tableau vide incompréhensible. Test dans la tâche 5.
4. **Réglages invalides** (débit 0, regex qui ne compile pas, seuil 0) : refusés avec un message, rien n'est enregistré. Tests dans la tâche 7.
5. **Fermer l'app en plein crawl** : la fenêtre se ferme dans les 5 secondes et le crawl est enregistré « arrêté » avec ses pages. Vérification à la main (tâche 9).

---

### Task 1 : compléments d'extraction HTML

**Files:**
- Modify: `crates/arpenteur-core/src/model.rs`, `crates/arpenteur-core/src/html.rs`, `src/lib/types.ts`

**Interfaces:**
- Produces : `PageFacts` gagne `lang: Option<String>`, `headings: Vec<u8>` (niveaux 1 à 6 dans l'ordre du document) et `unlinked_phones: Vec<String>` (numéros vus dans le texte visible sans lien `tel:` équivalent), tous `#[serde(default)]`. Miroir TypeScript : `lang: string | null`, `headings: number[]`, `unlinkedPhones: string[]`.

- [ ] **Step 1 : écrire les tests dans `html.rs`**

```rust
#[test] fn extracts_html_lang()                 // <html lang="fr-FR"> → Some("fr-FR") ; absent → None
#[test] fn extracts_heading_levels_in_order()   // h1, h2, h3, h2 → [1, 2, 3, 2]
#[test] fn detects_unlinked_french_phone()      // « Appelez le 02 99 12 34 56 » sans lien → ["02 99 12 34 56"]
#[test] fn linked_phone_is_not_reported()       // même numéro dans <a href="tel:+33299123456"> → []
#[test] fn siret_and_prices_are_not_phones()    // « SIRET 123 456 789 00012 », « 1 290,00 € » → []
#[test] fn old_facts_without_new_fields_deserialize() // JSON de PageFacts du plan 1 → lang None, headings [], unlinked_phones []
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core html`

- [ ] **Step 3 : implémenter**

Numéros : regex `(?:\+33\s?(?:\(0\)\s?)?|0)[1-9](?:[\s.\-]?\d{2}){4}` sur le texte visible, avec une borne de mot avant et après, pour ne pas mordre dans un SIRET. Comparaison avec les liens `tel:` sur les chiffres normalisés (`+33` devient `0`, séparateurs retirés).

- [ ] **Step 4 : vérifier**

Run: `cargo test -p arpenteur-core && npx tsc --noEmit`

- [ ] **Step 5 : commit** `feat(core): langue, niveaux de titres et téléphones dans l'extraction`.

### Task 2 : les nouveaux contrôles de page et de site

**Files:**
- Modify: `crates/arpenteur-core/src/model.rs`, `crates/arpenteur-core/src/checks.rs`
- Create: `src/lib/finding-codes.json`

**Interfaces:**
- Consumes : `PageFacts` (tâche 1).
- Produces :
  - `FindingCode` gagne 18 variantes : `ServerError`, `TitleMissing`, `TitleTooLong`, `TitleTooShort`, `MetaDescriptionMissing`, `MetaDescriptionTooLong`, `MetaDescriptionTooShort`, `H1Missing`, `H1Multiple`, `TitleDuplicate`, `MetaDescriptionDuplicate`, `ImageMissingAlt`, `HeadingStructure`, `HtmlLangMissing`, `PhoneNotClickable`, `ModernCompressionMissing`, `HttpsInvalid`, `SecurityHeadersMissing`.
  - `Finding` gagne `detail: Option<String>` (`#[serde(default)]`).
  - Constantes `TITLE_MIN_CHARS = 30`, `TITLE_MAX_CHARS = 60`, `META_MIN_CHARS = 70`, `META_MAX_CHARS = 160`.
  - `src/lib/finding-codes.json` : la liste des 37 noms sérialisés, dans l'ordre de déclaration.

- [ ] **Step 1 : écrire les tests dans `checks.rs`** (un positif et un négatif par code, sur la frontière)

```rust
#[test] fn server_error_on_internal_5xx()          // 503 interne → constat ; 404 → pas ce code
#[test] fn title_rules()                           // absent ; 29 et 61 caractères → trop court, trop long ; 30 et 60 → rien
#[test] fn meta_description_rules()                // absente ; 69 et 161 → signalés ; 70 et 160 → rien
#[test] fn h1_rules()                              // aucun H1 → h1Missing ; deux → h1Multiple ; un → rien
#[test] fn duplicates_only_among_indexable_pages() // même title sur 2 pages indexables → titleDuplicate (related = l'autre) ;
                                                   // même title dont une page en noindex → rien ; idem pour la meta description
#[test] fn image_missing_alt_ignores_decorative()  // <img> sans alt → constat (related = src) ; alt="" → rien
#[test] fn heading_structure_detects_skips()       // [1, 3] → constat ; [1, 2, 3, 2] → rien ; [2, 3] → rien (on ne saute pas en descendant)
#[test] fn html_lang_missing()                     // None → constat ; Some("fr") → rien
#[test] fn phone_not_clickable()                   // unlinked_phones non vide → constat (detail = premier numéro) ; vide → rien
#[test] fn brotli_missing_is_one_site_finding()    // aucune page HTML 200 avec content-encoding br → un seul constat sur l'URL de départ ;
                                                   // au moins une page en br → rien
#[test] fn checks_only_internal_html_200()         // page externe ou en 404 : aucun des contrôles de page ci-dessus
#[test] fn finding_codes_match_shared_list()       // noms sérialisés de toutes les variantes == include_str!("../../../src/lib/finding-codes.json")
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `cargo test -p arpenteur-core checks`

- [ ] **Step 3 : implémenter les règles**

- Constats par page sur les pages HTML 200 internes.
- Doublons parmi les pages indexables seulement.
- `HttpsInvalid` et `SecurityHeadersMissing` sont déclarés ici, mais produits par la tâche 3.
- Longueurs comptées en caractères (`chars().count()`), après réduction des espaces.

- [ ] **Step 4 : vérifier**

Run: `cargo test -p arpenteur-core`

- [ ] **Step 5 : commit** `feat(core): contrôles de title, meta, titres, images, langue, téléphone et compression`.

### Task 3 : constats de site tirés de SiteOne

**Files:**
- Create: `crates/arpenteur-core/src/siteone.rs` (tests dans le fichier)
- Modify: `crates/arpenteur-core/src/checks.rs`, `crates/arpenteur-core/src/storage.rs`

**Interfaces:**
- Produces :
  - `SiteOneSummary { certificate_errors: Vec<String>, certificate_expired: bool, certificate_issuer: Option<String>, security_issues: Vec<SecurityIssue> }`, avec `SecurityIssue { header: String, recommendation: String }` ;
  - `read_siteone(json: &str) -> Option<SiteOneSummary>`, qui tolère les tables absentes ;
  - `assemble(meta, data, siteone: Option<SiteOneSummary>) -> CrawlResult`. `load_result` lit `siteone.json` s'il existe. Un fichier absent (crawl arrêté) ou illisible donne `None`, sans erreur.
  - Règles :
    - `HttpsInvalid` : une ligne « Errors » dans `certificate-info`, ou « EXPIRED » dans « Valid to ». `detail` = erreurs + émetteur.
    - `SecurityHeadersMissing` : un seul constat, sur l'URL de départ, si au moins un en-tête de la table `security` a des avertissements ou des critiques (lire `security_analyzer.rs` de SiteOne pour la sémantique des colonnes). `related` = les en-têtes, `detail` = leurs recommandations jointes.

- [ ] **Step 1 : écrire les tests**, à partir d'extraits réels du JSON (tables `certificate-info` et `security` d'un crawl, réduites à quelques lignes)

```rust
#[test] fn valid_certificate_gives_no_finding()     // « Valid to … (VALID still for 7.4 day(s)) » → rien (pas de seuil « bientôt »)
#[test] fn expired_or_errored_certificate()         // « (EXPIRED 3 day(s) ago) » ou ligne « Errors » → HttpsInvalid avec l'émetteur
#[test] fn security_warnings_give_one_site_finding()
#[test] fn missing_or_garbled_json_is_none()
#[test] fn load_result_without_siteone_json_still_works()
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter**, **Step 4 : vérifier** (`cargo test -p arpenteur-core`).

- [ ] **Step 5 : commit** `feat(core): certificat et en-têtes de sécurité depuis SiteOne`.

### Task 4 : le catalogue dans l'interface

**Files:**
- Create: `src/lib/catalog.ts`, `src/lib/catalog.test.ts`
- Modify: `src/lib/types.ts`, `src/i18n/fr.ts`

**Interfaces:**
- Produces :
  - `type Level = 'haute' | 'moyenne' | 'basse' | 'info'` ;
  - `CATALOG: Record<FindingCode, { level: Level }>` ;
  - `findingTitle(code)`, `findingWhy(code)`, `findingFix(code): string | null` (null pour le niveau info) ;
  - clés `fr.ts` `finding_<code>_title`, `_why`, `_fix` ;
  - `FindingCode` en TypeScript devient l'union des 37 codes. `Finding` gagne `detail: string | null`.

- [ ] **Step 1 : écrire les tests**

```ts
test('chaque code de finding-codes.json a une entrée et ses textes', ...)
test('répartition validée : 7 haute, 14 moyenne, 12 basse, 4 info', ...)
test('textes repris du catalogue', ...)   // canonicalToRedirect : titre, pourquoi et correction mot pour mot ;
                                          // httpsInvalid : « Certificat HTTPS invalide »
```

- [ ] **Step 2 : vérifier que les tests échouent**, **Step 3 : implémenter** (textes copiés depuis `docs/superpowers/specs/2026-11-20-arpenteur-catalogue-problemes.md`), **Step 4 : vérifier** (`npm test && npx tsc --noEmit`).

- [ ] **Step 5 : commit** `feat(ui): catalogue des problèmes`.

### Task 5 : le panneau Problèmes

**Files:**
- Create: `src/components/ProblemsPanel.tsx`, `src/components/ProblemsPanel.test.tsx`
- Modify: `src/screens/Crawl.tsx`, `src/components/UrlTable.tsx`, `src/components/UrlDetail.tsx`, `src/styles.css`, `src/i18n/fr.ts`, `src/screens/Crawl.test.tsx`

**Interfaces:**
- Produces :
  - `ProblemsPanel({ findings, selectedCode, onSelect }: { findings: Finding[]; selectedCode: FindingCode | null; onSelect: (code: FindingCode | null) => void })` ;
  - `UrlTable` reçoit `urlFilter: Set<string> | null` (URL normalisées) ;
  - `UrlDetail` affiche une section « problèmes » pour l'URL choisie.

- [ ] **Step 1 : écrire les tests**

```ts
test('le panneau groupe par niveau avec les compteurs', ...)
    // en-têtes « haute 3 », « moyenne 5 »… ; l'information à part, hors total « # problèmes 16 »
test('un clic sur un code filtre le tableau sur ses URL', ...)
    // le filtre actif s'affiche avec un bouton pour le retirer
test('un constat de site filtre sur l’URL de départ', ...)   // Review Focus 3
test('le détail d’une URL liste ses problèmes avec le pourquoi et la correction', ...)
test('un code sans constat n’apparaît pas', ...)
```

- [ ] **Step 2 : vérifier que les tests échouent**

- [ ] **Step 3 : implémenter**

- Une colonne gauche de 220 px, comme la maquette.
- Pastilles de niveau en couleur : haute `--err`, moyenne `--warn`, basse `--dim`, information `--redir`.
- Chaque code est un `<button>` avec son compteur. Le code sélectionné est surligné et `aria-pressed` est posé.
- Le filtre combine les URL du constat et les `related` qui sont des URL du crawl. Tout passe par `normalizeUrl`.

- [ ] **Step 4 : vérifier** (`npm test && npx tsc --noEmit && npm run build`).

- [ ] **Step 5 : commit** `feat(ui): panneau des problèmes et filtre du tableau`.

### Task 6 : export CSV

**Files:**
- Create: `src/lib/csv.ts`, `src/lib/csv.test.ts`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/capabilities/default.json`, `package.json`, `src/lib/api.ts`, `src/screens/Crawl.tsx`, `src/i18n/fr.ts`

**Interfaces:**
- Produces :
  - `urlsCsv(rows: UrlRow[]): string` ;
  - `problemsCsv(findings: Finding[]): string`, avec les colonnes niveau, code, problème, URL, détail, URL liées ;
  - la commande Tauri `export_file(path: String, contents: String) -> Result<(), String>`, qui refuse une extension autre que `.csv` ;
  - `api.exportCsv(defaultName: string, contents: string): Promise<boolean>`, qui ouvre la boîte « Enregistrer sous » de `@tauri-apps/plugin-dialog` puis appelle la commande. Renvoie `false` si l'utilisateur annule.

- [ ] **Step 1 : écrire les tests**

```ts
test('séparateur point-virgule, BOM et CRLF', ...)      // commence par '﻿', lignes séparées par '\r\n'
test('les champs avec ; " ou retour à la ligne sont échappés', ...)
test('problemsCsv donne le niveau et le titre du catalogue', ...)
```

Et en Rust : `export_file_rejects_other_extensions`.

- [ ] **Step 2 à 4 : échec, implémentation, vérification**

- Deux boutons après le crawl : « exporter les URL » et « exporter les problèmes ».
- Nom proposé : `<domaine>-<id>-urls.csv` et `<domaine>-<id>-problemes.csv`.
- Vérifier avec `npm test && npx tsc --noEmit && cargo test -p arpenteur && cargo build -p arpenteur`.

- [ ] **Step 5 : commit** `feat(app): export CSV des URL et des problèmes`.

### Task 7 : écran Réglages

**Files:**
- Create: `crates/arpenteur-core/src/defaults.rs` (tests dans le fichier), `src/screens/Settings.tsx`, `src/screens/Settings.test.tsx`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/lib/api.ts`, `src/App.tsx`, `src/screens/Home.tsx`, `src/i18n/fr.ts`, `src/styles.css`

**Interfaces:**
- Produces :
  - `validate_settings(s: &CrawlSettings) -> Result<(), Vec<String>>` (messages en français, bornes de la section Décisions, regex compilées avec la crate `regex` comme SiteOne) ;
  - `CrawlStore::read_default_settings() -> CrawlSettings`, qui renvoie la valeur par défaut si le fichier est absent ou illisible ;
  - `CrawlStore::write_default_settings(&CrawlSettings) -> io::Result<()>`, en écriture atomique dans `<root>/settings.json` ;
  - les commandes `get_settings()` et `save_settings(settings)`, cette dernière renvoyant les erreurs de validation ;
  - `start_crawl` utilise les réglages enregistrés quand aucun n'est fourni ;
  - l'écran `Settings({ onBack })`, accessible depuis la barre du haut de l'accueil (« réglages »).

- [ ] **Step 1 : écrire les tests**

```rust
#[test] fn rejects_out_of_range_values()   // débit 0, connexions 9, seuil 0, limite 0 → erreurs ; valeurs par défaut → Ok
#[test] fn rejects_invalid_regex()         // "(" → erreur qui cite la regex
#[test] fn default_settings_roundtrip_and_fallback()
```

```ts
test('enregistrer des réglages valides appelle saveSettings', ...)
test('les erreurs de validation s’affichent et rien n’est enregistré', ...)   // Review Focus 4
```

- [ ] **Step 2 à 4 : échec, implémentation, vérification** (`cargo test --workspace && npm test && npx tsc --noEmit`).

- [ ] **Step 5 : commit** `feat(app): écran des réglages et valeurs par défaut enregistrées`.

### Task 8 : vérification des cibles de canonical

**Files:**
- Create: `crates/arpenteur-core/src/canonical_probe.rs`
- Modify: `crates/arpenteur-core/src/engine.rs`, `crates/arpenteur-core/tests/support/mod.rs`, `crates/arpenteur-core/tests/crawl.rs`

**Interfaces:**
- Produces : `async fn probe_canonical_targets(client: &reqwest::Client, pages: &[PageRecord], start: &Url, budget: usize, min_interval: Duration, cancel: &CancellationToken) -> Vec<PageRecord>`.
  - Il interroge les cibles de canonical internes qui n'ont pas été crawlées, sans suivre les redirections, au débit réglé et dans la limite du budget restant (`max_requests - pages crawlées`).
  - Les records ajoutés n'ont pas de `facts`. Leur `content_type` vaut `redirect` pour un 3xx, `html` sinon, et l'en-tête `location` est conservé.
  - `run_crawl` l'appelle après un crawl terminé (pas après un arrêt ou une interruption), puis ajoute les records aux pages avant d'écrire `arpenteur.json`.

- [ ] **Step 1 : écrire les tests**

Ajouter une variante témoin `CanonicalOrphanTarget` : `/x` a un canonical vers `/canon-target`, qui redirige en 301 et n'est liée nulle part.

```rust
#[tokio::test(flavor = "multi_thread")] async fn unlinked_canonical_target_is_probed()
    // canonicalToRedirect sur /x, et /canon-target demandé une seule fois
#[tokio::test(flavor = "multi_thread")] async fn probe_respects_budget_and_cancel()
    // budget 0 → aucune requête ; jeton annulé → aucune requête
#[tokio::test(flavor = "multi_thread")] async fn external_canonical_is_not_probed()
```

- [ ] **Step 2 à 4 : échec, implémentation, vérification** (`cargo test -p arpenteur-core`).

- [ ] **Step 5 : commit** `feat(core): vérification des cibles de canonical non crawlées`.

### Task 9 : arrêt propre à la fermeture

**Files:**
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`

**Interfaces:**
- Produces :
  - `AppState::is_running(&self) -> bool` ;
  - sur `RunEvent::ExitRequested`, si un crawl tourne : `api.prevent_exit()`, `state.stop()`, puis une tâche qui attend que `is_running()` passe à faux (au plus 5 s, en vérifiant toutes les 100 ms) avant d'appeler `app.exit(0)`. Un second `ExitRequested` pendant l'attente laisse sortir.

- [ ] **Step 1 : écrire les tests** (`state.rs`) : `is_running_follows_begin_and_finish`.

- [ ] **Step 2 à 4 : échec, implémentation, vérification** (`cargo test -p arpenteur && cargo build -p arpenteur`). Vérification à la main avec l'utilisateur (Review Focus 5).

- [ ] **Step 5 : commit** `feat(app): arrêt propre du crawl à la fermeture`.

### Task 10 : À propos et README

**Files:**
- Create: `src/screens/About.tsx`, `src/screens/About.test.tsx`, `README.md`, `THIRD_PARTY_NOTICES.md`
- Modify: `src/App.tsx`, `src/screens/Home.tsx`, `src/i18n/fr.ts`

**Interfaces:**
- Produces : l'écran `About({ onBack })`, accessible depuis l'accueil (« à propos »). Il affiche le nom, la version (`import { version }`), une phrase sur l'outil et la notice de licence de SiteOne Crawler (MIT, texte complet repris du fichier `LICENSE` de la crate 2.5.1). `THIRD_PARTY_NOTICES.md` porte le même texte.
- `README.md` contient :
  - ce que fait l'outil ;
  - l'installation sur Mac (« Ouvrir quand même » dans Réglages Système › Confidentialité et sécurité) et sous Windows (SmartScreen › « Informations complémentaires » › « Exécuter quand même ») ;
  - le développement (`npm install`, `npm run tauri dev`, `cargo test --workspace`, `npm test`) ;
  - la publication d'une version.

- [ ] **Step 1 : écrire le test** `l’écran À propos affiche la version et la licence MIT de SiteOne`.

- [ ] **Step 2 à 4 : échec, implémentation, vérification** (`npm test && npx tsc --noEmit`).

- [ ] **Step 5 : commit** `docs: README, notices tierces et écran À propos`.

### Task 11 : CI de release

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `src-tauri/tauri.conf.json` (cibles du bundle : `dmg` et `nsis`), `README.md` (section publication)

**Interfaces:**
- Produces : sur un push de tag `[0-9]+.[0-9]+.[0-9]+`, une matrice `macos-latest` (cible `aarch64-apple-darwin`) et `windows-latest` construit l'app avec `tauri-apps/tauri-action@v0`, puis crée une release GitHub en brouillon avec le `.dmg` et l'installeur `.exe` NSIS. Pas de signature.

- [ ] **Step 1 : écrire le workflow et valider le YAML localement** (`python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`). Il ne pourra tourner qu'une fois le dépôt créé et poussé, avec l'accord de l'utilisateur.

- [ ] **Step 2 : commit** `ci: release des installeurs macOS et Windows`.

### Vérification finale (à la main, avec l'utilisateur)

Dans `npm run tauri dev`, crawler crocweb.fr et vérifier :
- le panneau Problèmes, avec `canonicalToRedirect` sur `/tarifs` ;
- le filtre du tableau ;
- le détail d'une URL avec ses problèmes ;
- les deux exports CSV ouverts dans Excel ou Numbers ;
- les réglages (un débit à 0 doit être refusé) ;
- la fermeture en plein crawl.
