# Arpenteur, plan 2 : bilan

20 novembre 2026 · branche `feature/plan-1-moteur-et-coquille` (de `511075a` à `5984c35`, 21 commits, plus ce bilan)

## En bref

Le plan 2 (jalons 3 à 5) est implémenté :
- 11 tâches, chacune relue par un relecteur indépendant ;
- une revue finale de toute la branche, sans défaut critique ;
- une vague de corrections sur dix points, relue elle aussi.

**Tests**
- 161 tests Rust : cœur, crawls de bout en bout sur un site de test local, ancien crawl au format du plan 1, état Tauri.
- 92 tests front.
- TypeScript et builds propres.

**Ce que l'app fait en plus**
- Elle calcule les 37 problèmes du catalogue validé et les affiche dans un panneau, groupés par niveau. Un clic sur un problème filtre le tableau.
- Le détail d'une URL liste ses problèmes, avec le pourquoi et la correction.
- Elle exporte les URL et les problèmes en CSV pour Excel en français.
- Elle a un écran Réglages, avec des valeurs par défaut validées et enregistrées.
- Après un crawl terminé, elle vérifie les cibles de canonical jamais crawlées et regarde si le site sert du Brotli.
- Elle arrête proprement le crawl quand on ferme l'app.
- Elle a un écran « à propos », un README, les notices de licence et un workflow de release.

## Ce qui reste à faire avant la fusion

1. **La vérification à la main dans l'app** (`npm run tauri dev`), sur crocweb.fr :
   - le panneau Problèmes, avec `canonicalToRedirect` sur `/tarifs` ;
   - le filtre du tableau, y compris sur un problème de site (certificat, en-têtes, Brotli, sitemap) ;
   - le détail d'une URL avec ses problèmes ;
   - les deux exports CSV ouverts dans Excel ou Numbers ;
   - les réglages : un débit à 0, « 4.5 » dans les connexions et une regex avec une virgule doivent être refusés avec un message en français ;
   - la fermeture en plein crawl (fenêtre et Cmd+Q) : l'app se ferme en moins de 5 secondes et le crawl revient « arrêté », avec ses pages ;
   - la réouverture du crawl de la chocolaterie, enregistré avant le plan 2 : pas de « Langue de la page non déclarée » sur toutes les pages ;
   - un crawl sous Zscaler d'un site qui sert du Brotli, pour voir si le proxy fausse l'observation.
2. **Le dépôt GitHub, la CI et la première release.** Rien n'a encore tourné sur GitHub. Le workflow de release ne se prouve qu'au premier tag poussé. Il faudra ensuite installer le `.dmg` téléchargé sur un autre Mac, pour vérifier que la signature ad hoc suffit.

## Décisions prises en cours de route

Chaque décision est suivie de ce qu'elle coûte si elle est fausse.

- **Le plan 2 est posé sur la même branche que le plan 1.** Si c'est faux : une séparation à faire plus tard.
- **« Compression Brotli absente » se constate sur une observation, pas sur les pages crawlées.** Le client HTTP de SiteOne retire l'en-tête `Content-Encoding` : le contrôle prévu par le plan se serait déclenché sur tous les sites. Une requête de plus, sans décompression, est faite sur la page de départ après un crawl terminé. Elle compte dans la limite de requêtes. Un crawl arrêté n'a donc pas ce constat. Si c'est faux : un champ et une requête à retirer.
- **Un problème de site filtre le tableau sur l'URL de départ.** Elle a toujours une ligne, même quand elle redirige. Si c'est faux : une ligne de repli à ajouter.
- **Les cellules du CSV sont protégées contre les formules.** Un texte qui commence par `=`, `+`, `@` ou `-` (hors nombre) reçoit une apostrophe, parce qu'il vient de pages tierces et s'ouvre dans Excel. Excel affiche cette apostrophe. Si c'est faux : une fonction à retirer.
- **`export_file` écrit là où l'interface le demande, tant que c'est un `.csv`.** C'est la signature du plan. Le risque est faible pour une app locale. Pour durcir : ouvrir la boîte « Enregistrer sous » depuis Rust. Si c'est faux : une commande à réécrire.
- **Une regex qui contient une virgule est refusée.** SiteOne découpe ces options sur la virgule. Le message propose `\x2C` ou une autre écriture. Si c'est faux : `{n,m}` reste interdit sans raison.
- **« relancer » part avec les réglages enregistrés**, plus avec ceux de l'ancien crawl. Le bandeau d'un crawl limité demande de baisser le débit, et c'est l'écran Réglages qui le permet. Si c'est faux : une relance n'est plus identique au crawl d'origine.
- **Une cible de canonical en erreur réseau n'est pas enregistrée.** La page garde « Canonical vers une autre page » (information) au lieu d'un problème de niveau haute sur une seule requête ratée. La sonde coupe à 5 secondes. Si c'est faux : une cible vraiment morte est sous-signalée.
- **Un crawl enregistré avant le plan 2 ne reçoit pas « Langue non déclarée »**, grâce à un numéro de version dans `arpenteur.json`. Si c'est faux : un champ de trop.
- **« Certificat HTTPS invalide » ignore les échecs de connexion de l'analyse TLS de SiteOne.** Ils sont reconnus par le début du message, ce qui tient tant que SiteOne reste en 2.5.1.
- **La revue finale a tourné sur un modèle un cran sous le meilleur**, parce que la session précédente avait atteint la limite d'usage. Si c'est faux : un défaut subtil a pu passer.
- **La garde de version de la release a été corrigée après la contre-revue** (fins de ligne Windows), sans nouvelle relecture indépendante. Si c'est faux : un échec visible au premier tag.

## Questions pour toi

Ces cinq points demandent un choix de produit. Ils ne sont pas corrigés.

1. **La sonde des canonical ignore robots.txt et tes motifs à inclure ou à ignorer.** La conception promet « robots.txt respecté ».
2. **Une cible sondée qui répond 200 s'affiche « indexable : non »**, faute de contenu analysé. Son type vaut `html` même pour un PDF.
3. **« En-têtes de sécurité manquants » regroupe aussi des en-têtes présents mais trop bavards** (`Server`, `X-Powered-By`) et des cookies. Le détail est en anglais, c'est le texte de SiteOne.
4. **Les 503 d'un crawl limité deviennent des « Erreur serveur ».**
5. **Deux choix de catalogue :**
   - les contrôles de title, meta description et H1 tournent aussi sur les pages en noindex ;
   - « Niveaux de titres sautés » se déclenche quand le pied de page du thème est en h3 sur une page sans H2 (`/devis/` sur le crawl de la chocolaterie).

## À ouvrir au début du plan 3

**Hors plan, relevé par la revue finale**
- Les réglages propres à un crawl : seules les valeurs par défaut existent.
- Les notices des autres composants distribués (React, Tauri, reqwest).
- Une CSP pour la fenêtre (`csp: null` depuis le plan 1).
- La boîte « Enregistrer sous » de Windows avec un nom tapé sans `.csv`.

**Remarques mineures** (aucune ne fausse un résultat)
- **Téléphones** : préfixe 0033, numéro coupé par une balise, « ref A0299123456 » pris pour un numéro.
- **Contrôles** : doublons sensibles à la casse, images sans `alt` listées deux fois.
- **Interface** : double clic sur un export, alerte des réglages en haut d'un long formulaire, détail d'une ligne masquée par le filtre.
- **Sonde** : pas de test « aucune sonde si le crawl n'est pas terminé », lignes sondées comptées dans le nombre d'URL.
- **Tests manquants** : commandes Tauri des réglages, boucle d'attente de la fermeture.
- **Divers** : « version 2.5.1 » en dur dans l'écran « à propos », version portée par trois fichiers.
