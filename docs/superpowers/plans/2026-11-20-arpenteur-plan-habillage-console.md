# Arpenteur, habillage « Console » Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** habiller l'accueil et l'écran du crawl dans la direction « Console » validée, et ajouter le bouton « relancer ».

**Architecture:** une feuille `src/styles.css` porte les variables (couleurs, rayons, espacements) et des classes par composant. Les composants React ne gardent aucun style en ligne, sauf le positionnement des lignes virtualisées. Les textes restent dans `src/i18n/fr.ts`, passés en minuscules. Les décorations (`!` des bandeaux, `‹` du retour, `●` du statut en cours, flèche de tri) sont en CSS, pas dans les textes.

**Tech Stack:** React 19, TypeScript, Vite, Vitest + Testing Library, `@fontsource/jetbrains-mono`.

**Spec:** `docs/superpowers/specs/2026-11-20-arpenteur-design-console.md` (maquette : `docs/superpowers/specs/assets/2026-11-20-console-maquette.html`).

## Global Constraints

- Thème sombre seul. Couleurs exactes du tableau « Valeurs de référence » de la spec, déclarées une seule fois en variables sur `:root`.
- Police : `"JetBrains Mono", "SF Mono", Menlo, Consolas, monospace`, poids 400 et 700 importés depuis `@fontsource/jetbrains-mono` dans `src/main.tsx`. Taille de base 12 px, interligne 1,4.
- Statuts : 2xx `--ok`, 3xx `--redir`, 4xx, 5xx et négatifs (sauf -6) `--err`, -6 `--dim`.
- Accent `--accent` réservé au bouton principal, à la progression et au logo.
- Tous les textes visibles viennent de `fr.ts`, en minuscules. Aucun style en ligne hors virtualisation. Focus clavier visible sur tout élément interactif.
- Pas de mode clair, pas de colonne des problèmes, aucune autre fonctionnalité que « relancer ».

## Review Focus

1. **Le tableau remplit la hauteur de la fenêtre et reste virtualisé**, avec 10 000 lignes possibles. La hauteur fixe de 360 px disparaît, mais le conteneur défilant doit garder une hauteur bornée (flex avec `min-height: 0`), sinon le virtualiseur rend toutes les lignes. Test dans la tâche 3.
2. **« relancer » pendant qu'un autre crawl tourne** : le backend refuse (« Un crawl est déjà en cours. »). Le message s'affiche et l'écran reste sur le crawl courant. Test dans la tâche 3.
3. **Une URL très longue ou un title très long** : la cellule est tronquée avec une ellipse, sans casser la grille ni élargir la page. L'URL complète reste dans le détail. Couvert par le CSS de la tâche 3, vérifié à la main.
4. **Le clavier** : on peut parcourir l'accueil, les lignes de crawl et les en-têtes de tri au clavier, avec un focus visible. Vérifié à la main.

---

### Task 1 : fondations (police, feuille de style, textes)

**Files:**
- Create: `src/styles.css`
- Modify: `package.json`, `src/main.tsx`, `src/i18n/fr.ts`, `src/i18n/fr.test.ts`, `src/screens/Home.test.tsx`, `src/screens/Crawl.test.tsx`

**Interfaces:**
- Produces : les variables CSS `--bg --panel --side --line --line2 --field --text --strong --dim --accent --accent-ink --ok --redir --warn --err --sel --radius` ; les classes de base `.btn`, `.btn-primary`, `.field`, `.tone-ok`, `.tone-redir`, `.tone-warn`, `.tone-err`, `.tone-dim`, `.strong`, `.dim`, utilisées par les tâches 2 et 3.
- Produces dans `fr.ts` : les textes en minuscules ci-dessous, plus les nouvelles clés `restart`, `open`, `crawlsOne`, `crawlsMany`.

- [ ] **Step 1 : installer la police** avec `npm install @fontsource/jetbrains-mono`, puis importer `@fontsource/jetbrains-mono/400.css`, `@fontsource/jetbrains-mono/700.css` et `./styles.css` dans `src/main.tsx`.

- [ ] **Step 2 : mettre à jour les tests sur les nouveaux textes, et vérifier qu'ils échouent**

- `Home.test.tsx` : le bouton s'appelle « lancer ».
- `Crawl.test.tsx` : « crawl arrêté avant la fin. », `/baissez le débit/`, et l'en-tête de tri `/prof\./`.
- `fr.test.ts` : ajouter `t('crawlsMany', { count: 3 }) === '3 crawls'`.

Run: `npm test`
Expected : FAIL sur ces textes.

- [ ] **Step 3 : passer les textes de `fr.ts` en minuscules**

`appName: "arpenteur"`, `urlPlaceholder: "exemple.fr"`, `urlLabel: "adresse du site"`, `launch: "lancer"`, `launching: "lancement…"`, `sites: "# sites"`, `noSites: "aucun crawl pour l’instant."`, `pages: "{count} URL"`, `back: "retour"`, `status_running: "en cours"`, `status_completed: "terminé"`, `status_stopped: "arrêté"`, `status_throttled: "interrompu · le site limite les requêtes"`, `status_failed: "échec"`, `stop: "arrêter"`, `restart: "relancer"`, `open: "ouvrir"`, `crawlsOne: "1 crawl"`, `crawlsMany: "{count} crawls"`, `loading: "chargement…"`, `banner_stopped: "crawl arrêté avant la fin."`, `banner_throttled: "crawl interrompu : le site limite les requêtes. baissez le débit et relancez."`, `banner_failed: "le crawl a échoué : {message}"`, `filterPlaceholder: "/ filtrer url, title, h1…"`, `filterLabel: "filtre"`, `statusFilterLabel: "classe de statut"`, `statusAll: "statut : tous"`, `statusError: "erreurs réseau"`, `col_url: "url"`, `col_status: "statut"`, `col_type: "type"`, `col_title: "title"`, `col_h1: "h1"`, `col_indexable: "indexable"`, `col_depth: "prof."`, `col_inlinks: "liens entr."`, `col_words: "mots"`, `noRows: "aucune URL ne correspond."`, `detail_canonical: "canonical"`, `detail_metaDescription: "meta description"`, `detail_h1: "h1"`, `detail_inlinks: "liens entrants"`, `detail_outlinks: "liens sortants"`, `detail_headers: "en-têtes"`, `detail_jsonLd: "json-ld"`. Les autres clés (`progress`, `httpStatus_*`, `yes`, `no`, `none`) gardent leur texte.

- [ ] **Step 4 : écrire `src/styles.css` (fondations)**

Contenu :
- les variables de la spec sur `:root`, avec `--radius: 4px` ;
- un reset minimal (`box-sizing`, marges à 0) ;
- `html, body, #root` à pleine hauteur, fond `--bg`, texte `--text`, police et taille de base ;
- `.btn` (bordure `--field`, fond transparent), `.btn-primary` (fond `--accent`, encre `--accent-ink`, gras), états `:hover`, `:disabled` et `:focus-visible` (contour `--accent`) ;
- `.field` (bordure `--field`, fond `--bg`, texte `--strong`) ;
- les classes `.tone-*`, `.strong`, `.dim` ;
- la sélection de texte et une barre de défilement discrète.

- [ ] **Step 5 : vérifier**

Run: `npm test && npx tsc --noEmit && npm run build`
Expected : tout passe.

- [ ] **Step 6 : commit** `style(ui): police, variables et textes de l'habillage console`.

### Task 2 : l'accueil

**Files:**
- Modify: `src/screens/Home.tsx`, `src/screens/Home.test.tsx`, `src/styles.css`

**Interfaces:**
- Consumes : les classes et les textes de la tâche 1.
- Produces : `statusTone(kind: CrawlStatus['kind']): 'ok' | 'redir' | 'warn' | 'err' | 'dim'` dans `src/lib/table.ts`. Correspondance : completed → ok, running → redir, stopped → warn, throttled → warn, failed → err. Réutilisé par la tâche 3 pour le bandeau.

- [ ] **Step 1 : écrire les tests**

```ts
test('statusTone associe une teinte à chaque statut de crawl', ...)  // completed→ok, running→redir, stopped/throttled→warn, failed→err
test('chaque crawl de l’accueil est un bouton « ouvrir » accessible au clavier', ...)
    // getAllByRole('button', { name: /ouvrir/ }).length === nombre de crawls
test('l’en-tête d’un site affiche le nombre de crawls', ...)        // « 1 crawl » et « 3 crawls »
```

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `npm test`

- [ ] **Step 3 : implémenter la structure de la maquette**

1. Une barre du haut `.topbar` : logo `.logo` en `--accent`, numéro de version en `.dim`.
2. Le formulaire de lancement : champ `.field` et bouton `.btn-primary`.
3. Le titre « # sites ».
4. Pour chaque site, un bloc `.site` : en-tête avec le domaine en `.strong` et le nombre de crawls en `.dim`, puis une liste de lignes `.crawl-row`. Chaque ligne est un `<button>` en grille de 4 colonnes : date, statut teinté par `statusTone` (pastille `●` en CSS pour « en cours »), « N URL » en `.dim`, « ouvrir ›» en `.dim` (`›` en CSS).
5. L'erreur s'affiche dans un bloc `role="alert"` teinté `--err`.

- [ ] **Step 4 : vérifier**

Run: `npm test && npx tsc --noEmit`
Expected : PASS.

- [ ] **Step 5 : commit** `style(ui): accueil en habillage console`.

### Task 3 : l'écran du crawl, le tableau, le détail et « relancer »

**Files:**
- Modify: `src/screens/Crawl.tsx`, `src/screens/Crawl.test.tsx`, `src/components/UrlTable.tsx`, `src/components/UrlDetail.tsx`, `src/App.tsx`, `src/lib/table.ts`, `src/lib/table.test.ts`, `src/styles.css`

**Interfaces:**
- Consumes : `statusTone` (tâche 2), `startCrawl` (`src/lib/api.ts`).
- Produces :
  - `httpTone(status: number): 'ok' | 'redir' | 'err' | 'dim'` dans `table.ts` : 2xx ok, 3xx redir, ≥ 400 err, -6 dim, autres négatifs err ;
  - une nouvelle prop `onOpen: (domain: string, id: string, running: boolean) => void` sur `Crawl`, que `App.tsx` câble comme pour l'accueil.

- [ ] **Step 1 : écrire les tests**

```ts
test('httpTone colore les statuts HTTP', ...)          // 200→ok, 301→redir, 404→err, 503→err, -1→err, -6→dim
test('relancer démarre un crawl sur la même URL et l’ouvre', ...)
    // crawl terminé (meta.startUrl = 'https://exemple.fr/') → clic « relancer »
    // → startCrawl appelé avec 'https://exemple.fr/' → onOpen('exemple.fr', 'nouvel-id', true)
test('relancer refusé affiche le message et reste sur l’écran', ...)
    // startCrawl rejette « Un crawl est déjà en cours. » → texte visible, onOpen non appelé
test('pendant le crawl, arrêter est visible et relancer ne l’est pas', ...)
test('le tableau ne rend qu’une fenêtre de lignes', ...)
    // 2 000 lignes → getAllByRole('row').length < 200 (Review Focus 1)
```

Restaurer les stubs `clientHeight`/`clientWidth` dans un `afterAll`, en gardant les descripteurs d'origine.

- [ ] **Step 2 : vérifier que les tests échouent**

Run: `npm test`

- [ ] **Step 3 : implémenter**

- **`Crawl.tsx`**
  - Une barre du haut `.topbar` :
    - bouton « retour » (`‹` en CSS) et logo ;
    - URL de départ en `.field` non éditable (`result.meta.startUrl` une fois chargé, sinon le domaine) ;
    - pendant le crawl : bouton « arrêter » et progression (point vert animé, nombre en `--accent`) ;
    - après le crawl : bouton « relancer » en `.btn-primary`.
  - Le bandeau `role="status"` est teinté par `statusTone` (`!` en CSS).
  - La mise en page occupe toute la fenêtre en colonne flex : barre du haut, bandeau, outils, tableau (`flex: 1; min-height: 0`), détail.
- **`UrlTable.tsx`**
  - Barre d'outils : champ de filtre et liste des statuts.
  - En-tête collant ; la colonne triée est en `--strong` avec sa flèche (↑ ou ↓) en CSS, via `aria-sort`.
  - Statut teinté par `httpTone`, oui en `--ok` et non en `--err`, valeurs vides en `--dim`, nombres alignés à droite en chiffres tabulaires.
  - Cellules tronquées avec ellipse ; ligne sélectionnée sur `--sel`.
  - Lignes focalisables (`tabIndex={0}`), sélection à Entrée ou Espace.
  - Le conteneur défilant prend la hauteur disponible au lieu des 360 px.
- **`UrlDetail.tsx`** : une grille à deux colonnes (libellé en `.dim`, valeur en `.strong`), hauteur bornée avec défilement.
- **`App.tsx`** : passer `onOpen` à `Crawl`.

- [ ] **Step 4 : vérifier**

Run: `npm test && npx tsc --noEmit && npm run build`
Expected : PASS.

- [ ] **Step 5 : commit** `style(ui): écran du crawl en habillage console et bouton relancer`.

### Vérification finale (à la main, avec l'utilisateur)

Lancer `npm run tauri dev`, puis crawler crocweb.fr et contrôler :
- la ressemblance avec la maquette ;
- le tableau qui remplit la fenêtre ;
- la navigation au clavier ;
- une URL longue, tronquée sans casser la grille ;
- « relancer » après un crawl terminé ;
- le bandeau d'un crawl arrêté.

---

## Bilan d'exécution (20 novembre 2026)

Commits `4306ec6` à `9c623dd`. 36 tests front verts, TypeScript et build propres. Chaque tâche a été relue, puis la passe entière, puis la vague de corrections.

**Décisions prises en cours de route** (chacune avec ce qu'elle coûte si elle est fausse)
- L'habillage est sur la même branche que le plan 1, puisque tu l'as gardée non fusionnée. Coût : une séparation de branche plus tard.
- La ligne « 1 connexion · 2 req/s » de la maquette est retirée de l'accueil : l'interface ne connaît pas les réglages du backend. Coût : une ligne d'information.
- Un repli global `:focus-visible` (contour vert citron) couvre tout élément interactif. Coût : aucun.
- `gridTemplateColumns` reste en style en ligne dans le tableau, car il est calculé depuis `COLUMNS`, source unique pour l'en-tête et les lignes. Coût : un style en ligne.
- Après la revue finale :
  - les colonnes sont en `ch` pour la chasse fixe, avec la flèche de tri qui ne peut plus être coupée ;
  - l'en-tête collant est placé dans la zone de défilement ;
  - « relancer » reprend les réglages du crawl ;
  - la fenêtre s'ouvre en 1280×800, au minimum 960×600 ;
  - les décorations CSS ne sont plus lues par les lecteurs d'écran.
- Non corrigés et reportés :
  - le bandeau « baissez le débit », écrit avant que les réglages existent (plan 2) ;
  - le placeholder « / filtrer », qui suggère un raccourci inexistant ;
  - l'affichage des URL en chemin relatif pour le même hôte.

  Coût : une petite dette d'interface.

**Remarques mineures reportées**
- « connexion impossible » remplit exactement sa colonne : prévoir 1 `ch` de marge.
- Deux blocs `*` et deux règles `.crawl-banner` à fusionner.
- La logique de classes de teinte du tableau est à simplifier.
- Aucun test pour le bouton « relancer » désactivé pendant l'appel.
- Sous un macOS antérieur à Safari 16.2, `color-mix` n'est pas pris en charge : bandeau sans fond teinté.
