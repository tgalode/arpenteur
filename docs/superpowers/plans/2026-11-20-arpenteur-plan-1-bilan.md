# Arpenteur, plan 1 : bilan

20 novembre 2026 · branche `feature/plan-1-moteur-et-coquille` (de `176a02b` à `a3ebd03`, 27 commits, plus ce bilan)

## En bref

Le plan 1 (jalons 0 à 2) est implémenté :
- 13 tâches, chacune relue par un relecteur indépendant ;
- une revue finale de toute la branche ;
- une vague de corrections, relue elle aussi.

**Tests**
- 98 tests Rust : cœur, crawls de bout en bout sur un site de test local, état Tauri.
- 24 tests front.
- TypeScript, build et `cargo fmt` propres.

**Ce que fait l'app**
- Elle crawle un site avec le moteur SiteOne 2.5.1, à débit prudent.
- Elle s'arrête à la demande et détecte un site qui limite les requêtes.
- Elle enregistre chaque crawl par site.
- Elle affiche la progression, puis un tableau des URL (profondeur, liens entrants, canonical, indexabilité) et le détail d'une URL.
- Le cœur calcule déjà 19 contrôles SEO. Leur affichage (panneau Problèmes) relève du plan 2.

## Ce qui reste à faire avant la fusion

1. **Le dépôt GitHub et la CI.** La CI (macOS et Windows) est écrite, mais elle n'a jamais tourné. Il faut créer le dépôt privé, pousser la branche et vérifier les deux jobs. C'est la seule preuve que l'app compile sous Windows (jalon 0).
2. **La vérification à la main dans l'app** (`npm run tauri dev`), sur crocweb.fr :
   - la progression s'affiche, puis le tableau ;
   - `/tarifs` est à la profondeur 1, avec ses liens entrants dans le détail ;
   - « Arrêter » affiche le bandeau ;
   - « Retour » pendant un crawl, puis rouvrir le crawl : la progression et le bouton « Arrêter » reviennent ;
   - fermer l'app en plein crawl, la rouvrir : le crawl apparaît en échec (« interrompu »), pas « En cours ».

## Décisions prises en cours de route

Chaque décision est suivie de ce qu'elle coûte si elle est fausse.

**Organisation**
- Le travail s'est fait sur une branche, sans worktree, d'après ta préférence. Si c'est faux : un renommage.
- La création du dépôt, le push et l'exécution de la CI sont repoussés à la fin, avec ton accord. Si c'est faux : un problème Windows découvert plus tard que prévu.
- Les dépendances sont ajoutées tâche par tâche. Coût : aucun.

**Moteur**
- `content_type` reprend le nom du type SiteOne en minuscules (`html`, `image`, `redirect`). Coût : aucun, un test le vérifie.
- Le moteur exige un runtime tokio multi-thread, parce que l'analyse DNS de SiteOne panique sur un runtime mono-thread. Si c'est faux : un plantage au démarrage d'un crawl.
- Le fournisseur crypto rustls (ring) est installé avant chaque crawl, comme le fait la CLI. Si c'est faux : les sites HTTPS échouent.
- `manager.run()` tourne directement dans le `select!` d'arrêt, sans tâche détachée. Un arrêt coupe donc la boucle de SiteOne, et seules les requêtes déjà parties se terminent. Si c'est faux : le crawl continuerait après « Arrêter ». La revue finale l'a vérifié avec une sonde.
- Après un arrêt, les pages tout juste arrivées peuvent manquer, la tâche Ctrl+C interne de SiteOne n'est pas libérée, et des progressions tardives peuvent arriver. Tout cela est accepté pour la v1. Coût : un petit écart d'affichage.
- Le User-Agent de la découverte du sitemap n'a pas le `!` final, qui est un drapeau propre à SiteOne. Un débit ≤ 0 est refusé.
- reqwest est ajouté sans ses fonctionnalités par défaut, pour ne pas tirer une seconde pile TLS.

**Sitemap**
- Le test de débit attend une seule pause pour robots.txt puis le sitemap : le plan comptait deux pauses par erreur.
- `sources` liste les fichiers lus avec succès. `found` est vrai dès qu'un fichier est lu, même vide. Les sitemaps d'un autre hôte sont écartés et notés en erreur.
- **Après la revue finale**, les URL du sitemap passent par robots.txt et par les filtres (ressources, regex de l'utilisateur) avant d'être crawlées, avec les règles de SiteOne. Si c'est faux : des pages interdites ou des PDF seraient téléchargés.

**Extraction et contrôles**
- Les URL sont résolues contre `<base href>`. Un H1 vide compte comme un H1. Les types JSON-LD sont lus au premier niveau et dans `@graph`.
- Les pages sans texte n'entrent pas dans la détection des doublons, puisqu'elles partagent toutes la même empreinte. Si c'est faux : de faux doublons.
- Une page reçoit au plus un constat de canonical, dans cet ordre de priorité : absent > erreur > redirection > autre hôte > autre URL.
- Les liens entrants sont comptés une fois par page source. La profondeur traverse les redirections internes sans ajouter de niveau.
- **Après la revue finale :**
  - une page externe atteinte par redirection n'a ni constat ni lien sortant compté ;
  - une URL ignorée par SiteOne (statut -6) n'est pas une erreur ;
  - un site injoignable termine en échec, avec un message.

**Stockage**
- Les noms de site sont nettoyés selon les règles de Windows. Le dossier d'un crawl est créé de façon atomique. Un crawl corrompu est ignoré dans la liste.
- La lecture d'un crawl refuse les chemins qui sortiraient du dossier de stockage.
- **Après la revue finale**, un crawl resté « en cours » au démarrage de l'app est marqué en échec : « Crawl interrompu par la fermeture de l'application. »

**Coquille Tauri et interface**
- `panic = "abort"` est retiré de la version finale. Une panique du moteur est ainsi contenue, et le crawl est marqué en échec. Si c'est faux : un binaire un peu plus gros.
- La place « crawl en cours » se libère par identifiant de crawl. C'est corrigé en deux tours, le second sur un cas limite que ma propre consigne avait introduit.
- L'écran du crawl rattrape un crawl déjà fini avant de commencer à écouter les événements.
- Les détails d'une URL utilisent une copie TypeScript de la normalisation d'URL du Rust.

## À ouvrir au début du plan 2

**Avant le catalogue des problèmes**
- **Cibles de canonical** : elles ne sont jamais demandées. `canonicalToRedirect` ne marche donc que si la cible est liée ailleurs. C'est pourtant le cas de crocweb.fr. Il faut les vérifier après le crawl, dans le budget de requêtes. C'est un choix de conception.
- **Fermeture de l'app** : demander l'arrêt du crawl et attendre la fin, pour garder les pages.
- **Messages à ajouter** :
  - « le site de départ redirige vers un autre domaine » ;
  - un libellé pour le cas où robots.txt de l'utilisateur vise `Arpenteur`, alors que SiteOne ne lit que les groupes `*` et `SiteOne-Crawler`.
- **Gros crawls** :
  - commandes Tauri asynchrones, pour ne plus lire les fichiers sur le thread principal ;
  - `CrawlResult` envoyé en entier par l'IPC ;
  - recalcul des images lourdes.
- **Sitemap** :
  - borner le nombre de fichiers lus ;
  - ne pas suivre de redirection hors du site ;
  - compter ces requêtes dans le budget ;
  - ne lire robots.txt qu'une fois.
- **Divers** :
  - une CSP pour la fenêtre ;
  - les messages Rust et les erreurs système à passer par `fr.ts` ;
  - les données dans `app_local_data_dir` sous Windows ;
  - un README de développement.

**Remarques mineures** (détail dans l'historique des revues)
- **Extraction HTML** :
  - `Hel<b>lo</b>` compte deux mots ;
  - le `<title>` d'un SVG peut être pris pour celui de la page ;
  - le test de profondeur extrême dure 16 s ;
  - une page piège très imbriquée est lente à analyser.
- **Sitemap** : `.gz` servi avec `Content-Encoding`, URL gardées d'un fichier en échec, index tronqué, tests manquants (`.gz`, chemins relatifs, profondeur).
- **Stockage** :
  - nom de site vide ;
  - noms réservés de Windows ;
  - pas de `fsync` ;
  - le balayage au démarrage s'arrête sur la première erreur d'écriture ;
  - pas de garde contre une seconde instance de l'app.
- **Moteur** :
  - débit minuscule qui peut paniquer ;
  - filtre des graines compilé depuis les réglages bruts ;
  - robots.txt accepté après redirection.
- **Contrôles** :
  - tests négatifs à ajouter (404 externe, image externe) ;
  - URL du constat de lien cassé ;
  - avertissements clippy.
- **Garde-fou** : un seuil à 0 se déclenche tout de suite, à valider dans l'écran Réglages. Tests de neutralité et de remise à zéro à ajouter.
- **Interface** :
  - tri sur une date invalide ;
  - bouton « Arrêter » sans état d'attente ;
  - lignes du tableau non accessibles au clavier ;
  - stub jsdom jamais restauré ;
  - fenêtrage non testé ;
  - bandeau d'échec non testé ;
  - libellé « N pages », qui compte aussi les images.
