# Arpenteur, plan 4 : conception

22 novembre 2026 · suite du bilan du plan 3 (`docs/superpowers/plans/2026-11-21-arpenteur-plan-3-bilan.md`)

## En deux minutes

**Pourquoi.** Un message vocal du 22 novembre liste ce qu'un auditeur attend de Screaming Frog : des contrôles, et une façon de travailler. Arpenteur couvre déjà une bonne moitié des contrôles. Il est le plus loin sur l'interface : l'écran reste muet pendant le crawl, et les problèmes n'ont ni type ni vue d'ensemble.

**Ce que livre le plan 4.**
1. **Sept contrôles de plus** : H1 en double, H1 trop long, plusieurs canonical, fausses pages d'erreur (soft 404, deux contrôles), adresse de la liste en erreur, robots.txt introuvable.
2. **Un type sur chaque problème** : problème, avertissement ou opportunité.
3. **Une vue d'ensemble** : un résumé du crawl et un classement des problèmes par thème, en plus du classement par priorité.
4. **Les résultats pendant le crawl** : le tableau et le panneau se remplissent au fil de l'eau.
5. **robots.txt consultable** : son contenu, les sitemaps qu'il déclare, les adresses qu'il bloque.

**Ce qui attend le plan 5.** Hreflang, pagination, données structurées détaillées, visualisation du maillage.

**Plus tard.** Problèmes de JavaScript (il faut un navigateur embarqué), Search Console et Analytics (connexion à un compte Google), aperçu Google en pixels, PageSpeed.

**Ce que tu as décidé, et ce que j'ai choisi.** Tu as choisi ce découpage. Le reste de ce document est mon choix. Deux parties demandent ta validation avant le code, parce qu'elles touchent le catalogue : les sept nouvelles entrées, et le type et le thème de chacun des 45 problèmes. La liste de mes choix est à la fin.

## Objectifs

- Pendant un crawl, l'auditeur voit ce qui est déjà trouvé, comme dans Screaming Frog.
- Chaque problème dit sa priorité, son type, ce qu'il veut dire et comment le corriger.
- Le panneau donne une vue d'ensemble par thème (titles, canonical, images…), pas seulement par priorité.
- La règle D5 reste la boussole : un faux positif coûte plus qu'un contrôle manquant.

## Ce que le plan 4 ne fait pas

- Pas de hreflang, de pagination, de validation des données structurées ni de visualisation du maillage (plan 5).
- Pas de rendu JavaScript, de Search Console, d'Analytics, d'aperçu Google ni de PageSpeed.
- Pas de détail d'une URL pendant le crawl : il s'ouvre une fois le crawl fini.
- Pas de déplacement du panneau à droite. Il reste à gauche, comme dans la maquette validée au plan 2.
- Pas de correction des points mineurs laissés par le plan 3, sauf l'adresse de la liste en erreur.

## Ce qui ferait échouer le plan, même s'il marche

- Un « soft 404 » sur une page légitime : un article qui parle des erreurs 404, une page qui partage par hasard un mot avec la page d'erreur.
- Un aperçu en direct qui ralentit le crawl, ou qui affiche un problème qui disparaît ensuite parce que le crawl a avancé.
- Un type ou un thème qui diffère entre le panneau, le détail d'une URL et le CSV.
- Un ancien crawl qui ne s'ouvre plus, ou qui gagne un faux constat.

## Contraintes

- Celles des plans 2 et 3 restent valables :
  - tout nouveau champ enregistré porte `#[serde(default)]` ;
  - les textes de l'interface sont dans `fr.ts` ;
  - l'habillage Console est conservé ;
  - chaque contrôle a un test positif et un test négatif sur sa frontière ;
  - toute requête ajoutée respecte le débit réglé et compte dans la limite de requêtes.
- Le catalogue validé fait foi. Le plan 4 lui ajoute sept codes et deux propriétés (type, thème). Les textes existants ne changent pas.
- SiteOne reste en version `=2.5.1`.
- Rien n'est poussé et aucun dépôt distant n'est créé sans ton accord.

## Approches

### Soft 404 : comment reconnaître une fausse page d'erreur ?

1. **Demander une adresse qui n'existe pas, et comparer (retenue).** Après le crawl, une requête vers une adresse tirée au hasard montre ce que le site répond pour une page inexistante. Si c'est 200, le site entier a le défaut. La page reçue sert aussi de modèle : une page crawlée en 200 qui a le même title et le même H1 que ce modèle est une fausse page d'erreur.
2. **Chercher des mots dans la page** (« introuvable », « 404 »). Écartée : un article sur les erreurs 404 serait signalé.
3. **Se fier au peu de contenu.** Écartée : le contrôle « contenu court » existe déjà, et une page courte n'est pas une erreur.

### Résultats pendant le crawl : comment les amener à l'écran ?

1. **Un instantané demandé par l'interface toutes les quelques secondes (retenue).** Le moteur sait donner à tout moment les pages déjà collectées. L'interface demande un instantané, le cœur calcule lignes et constats avec le même code qu'à la fin. L'intervalle s'allonge si le calcul devient long.
2. **Envoyer chaque page et chaque constat dès qu'ils arrivent.** Écartée : il faudrait un second calcul des constats, page par page, à garder cohérent avec le premier.
3. **N'afficher que des compteurs.** Écartée : le message demande de voir les erreurs, pas seulement leur nombre.

### Type et thème : où les ranger ?

1. **Dans le catalogue de l'interface, à côté du niveau (retenue).** Le cœur ne produit que des codes. Le niveau y est déjà.
2. **Dans le cœur Rust.** Écartée : deux endroits à tenir d'accord pour une information d'affichage.

## 1. Sept contrôles de plus

**Entrées du catalogue, à valider par toi.**

| Identifiant | Niveau | Titre | Ce que ça veut dire | Comment corriger |
|---|---|---|---|---|
| `soft404` | haute | Fausse page d'erreur (soft 404) | La page affiche le message d'erreur du site, mais répond 200. Google peut l'indexer ou perdre du temps dessus. | Faire répondre 404 ou 410 à cette adresse, ou y remettre un vrai contenu. |
| `soft404Site` | haute | Le site répond 200 aux adresses inexistantes | Une adresse qui n'existe pas renvoie une page normale au lieu d'une erreur 404. Google ne peut pas distinguer les vraies pages des fausses. | Configurer le serveur pour répondre 404 aux adresses inexistantes. |
| `listedUrlError` | haute | Adresse de la liste en erreur | Une adresse de la liste répond par une erreur, ou ne répond pas. | Corriger l'adresse, la rediriger ou la retirer. |
| `canonicalMultiple` | moyenne | Plusieurs balises canonical | La page désigne plusieurs adresses de référence. Google peut les ignorer toutes. | Ne garder qu'une balise canonical par page. |
| `h1Duplicate` | basse | H1 en double | Plusieurs pages ont le même titre principal. Elles se distinguent moins bien. | Donner à chaque page un H1 qui lui est propre. |
| `h1TooLong` | basse | H1 trop long (plus de 70 caractères) | Un titre principal long se lit mal et dilue le sujet de la page. | Raccourcir le H1 à l'essentiel. |
| `robotsTxtMissing` | basse | Fichier robots.txt introuvable | Sans robots.txt, le sitemap n'est pas déclaré aux moteurs. | Ajouter un fichier robots.txt à la racine, avec l'adresse du sitemap. |

Le catalogue passe à 45 codes : 10 en haute, 16 en moyenne, 15 en basse, 4 en information.

**Règles.**

- **`h1Duplicate`** : le premier H1 de la page, espaces réduits, identique sur au moins deux pages indexables. Même périmètre que « Title en double ». En mode liste, même périmètre que les autres doublons.
- **Anciens crawls.** Les H1 sont déjà enregistrés : `h1Duplicate` et `h1TooLong` apparaissent donc aussi à la réouverture d'un crawl fait avant le plan 4. Ce sont des constats vrais, pas des faux positifs.
- **`h1TooLong`** : le premier H1 fait plus de 70 caractères, espaces réduits. 70 ne donne rien, 71 est signalé. Pas sur une page en noindex, comme les autres contrôles de H1.
- **`canonicalMultiple`** : la page a au moins deux balises `link rel="canonical"`. Le détail cite les adresses. L'extraction garde aujourd'hui la première seulement : elle doit les relever toutes. Un crawl déjà enregistré n'a pas cette donnée et ne reçoit pas ce constat.
- **`listedUrlError`** : en mode liste seulement. Une adresse de la liste dont la réponse est une erreur (statut 400 ou plus, ou pas de réponse), ou qui redirige vers une page en erreur. Sur un crawl limité, 429 et 503 ne comptent pas, comme ailleurs. Une adresse écartée par robots.txt ou par un motif n'est pas en erreur.
- **`robotsTxtMissing`** : un constat de site, quand le serveur répond 404 ou 410 à `/robots.txt`. Une erreur 5xx ou une erreur de réseau ne donne pas de constat : le fichier existe peut-être. La règle est la même pour un site et pour une liste. Le crawl doit donc enregistrer la réponse reçue pour robots.txt, ce qu'il ne fait pas aujourd'hui. Un crawl déjà enregistré n'a pas cette donnée et ne reçoit pas ce constat.

**Soft 404.**

- **La sonde.** Après un crawl terminé (site ou liste), une requête GET vers `<origine>/arpenteur-introuvable-<16 caractères au hasard>`, avec le client et le délai de 5 secondes des autres sondes. Les redirections sont suivies, cinq au plus. Le corps est lu dans la limite de 1 Mo, pour en tirer le title, le premier H1 et l'empreinte du texte.
- **Sa place dans le budget.** Compression, soft 404, cibles de canonical, liens externes. Elle compte pour une requête. Si l'adresse tirée est interdite par robots.txt, la sonde n'a pas lieu.
- **`soft404Site`.** La réponse finale est 200, directement ou après redirection. Le détail dit lequel : « répond 200 » ou « redirige vers … ».
- **`soft404`.** Une page interne, HTML, en 200, qui a le même title et le même premier H1 que la page reçue par la sonde (tous deux non vides), ou la même empreinte de texte. Ce contrôle ne tourne que si la sonde a reçu sa réponse sans redirection : si le site redirige les adresses inexistantes vers l'accueil, seul `soft404Site` existe. Une page n'est jamais comparée à elle-même.
- **Garde-fou.** Si plus de la moitié des pages du site ont le title et le H1 de la page d'erreur, aucune n'est signalée : c'est un thème qui met le nom du site partout, pas une série de fausses pages d'erreur.
- **Sans réponse de la sonde** (délai dépassé, erreur réseau, crawl arrêté) : aucun des deux constats.
- **L'enregistrement.** Le résultat de la sonde (statut, adresse finale, title, H1, empreinte) va dans les données du crawl.

## 2. Un type sur chaque problème

Trois types, repris de Screaming Frog :
- **problème** : une erreur à corriger ;
- **avertissement** : à vérifier, ce n'est pas forcément une erreur ;
- **opportunité** : une amélioration possible.

Le type ne remplace pas le niveau : un problème garde sa priorité haute, moyenne ou basse. Le niveau « information » reste hors du total.

**Où il s'affiche.** Une étiquette sur chaque ligne du panneau, dans le détail d'une URL, et une colonne « type » dans le CSV des problèmes, entre « niveau » et « code ».

**Type et thème de chaque code, à valider par toi.**

| Thème | Problème | Avertissement | Opportunité |
|---|---|---|---|
| Réponses et redirections | `serverError`, `soft404`, `soft404Site`, `listedUrlError`, `redirectChain` | `temporaryRedirect` | |
| Liens | `brokenInternalLink`, `brokenExternalLink` | `orphanPage` | |
| Titles | `titleMissing`, `titleDuplicate` | | `titleTooLong`, `titleTooShort` |
| Meta descriptions | | | `metaDescriptionMissing`, `metaDescriptionDuplicate`, `metaDescriptionTooLong`, `metaDescriptionTooShort` |
| Titres H1 à H6 | `h1Missing` | `h1Multiple`, `headingStructure` | `h1Duplicate`, `h1TooLong` |
| Canonical | `canonicalMissing`, `canonicalToError`, `canonicalToRedirect`, `canonicalMultiple` | `canonicalToOtherHost`, `canonicalToOtherUrl` | |
| Indexation et sitemap | `sitemapUrlError`, `sitemapUrlNonIndexable` | `noindex`, `notInSitemap`, `sitemapNotFound`, `robotsTxtMissing` | |
| Contenu | `duplicateContent` | | `thinContent` |
| Images | `imageMissingAlt` | | `imageTooHeavy` |
| Technique | `httpsInvalid`, `jsonLdInvalid` | `securityHeadersMissing`, `htmlLangMissing` | `jsonLdMissing`, `phoneNotClickable`, `modernCompressionMissing` |

## 3. Une vue d'ensemble

Le panneau de gauche gagne deux choses.

**Un résumé du crawl, en haut.** Le nombre d'URL, leur répartition par réponse, et les pages indexables.

**Deux classements des problèmes**, au choix : « priorité » (celui d'aujourd'hui) et « thème ».

```
# vue d'ensemble
URL             95
2xx             88
3xx              4
4xx              3
5xx              0
indexables      71

# problèmes 16     [ priorité | thème ]
▸ titles                    6
    Title en double          4   problème
    Title trop long          2   opportunité
▸ canonical                 3
    Canonical absent         3   problème
▸ images                    7
    …
information (hors total)    5
```

- Un clic sur une ligne de réponse (« 4xx ») règle le filtre de statut du tableau, qui existe déjà.
- Un clic sur un problème filtre le tableau, comme aujourd'hui.
- Dans la vue par thème, un thème sans problème n'apparaît pas. Les codes de niveau « information » restent regroupés à part, hors du total.
- Le classement choisi est retenu d'un lancement à l'autre.

## 4. Les résultats pendant le crawl

**Ce que voit l'utilisateur.** Dès les premières pages, l'écran du crawl affiche le panneau et le tableau, qui se remplissent. La ligne de progression et le bouton « arrêter » restent en haut. Une mention « résultats provisoires » dit que le crawl n'est pas fini.

**Comment.** Tant que le crawl tourne, l'interface demande un instantané toutes les 2 secondes. Le cœur le calcule avec les pages déjà collectées, par le même code qu'à la fin. Si un calcul dure plus de 500 ms, l'intervalle passe à quatre fois cette durée.

**Ce qui est provisoire.**
- Un constat qui pourrait disparaître ou changer quand le crawl avance n'est pas affiché avant la fin. Deux cas :
  - « Page orpheline » : la page peut encore être liée plus loin ;
  - un constat de canonical dont la cible n'est pas encore connue : « Canonical vers une autre page » deviendrait « vers une redirection » ou « vers une page en erreur » une fois la cible lue.
- Les constats qui demandent la fin du crawl arrivent à la fin : soft 404, liens externes, compression, certificat.
- « Profondeur » et « liens entrants » peuvent encore changer : c'est le sens de « résultats provisoires ».

**Ce qui n'est pas disponible pendant le crawl.** Le détail d'une URL et les exports CSV. Un clic sur une ligne dit : « le détail s'ouvre une fois le crawl terminé ».

**Ce que l'instantané reçoit.** Les pages déjà collectées et le sitemap lu avant le crawl, pour que les contrôles aient les mêmes entrées qu'à la fin. Sans le sitemap, « Aucun sitemap trouvé » s'afficherait à tort pendant tout le crawl. Le nombre d'URL affiché compte les pages crawlées, comme à la fin.

**Ce qui est gardé d'un instantané à l'autre.** Le filtre de problème, la recherche, le tri, le choix des colonnes et la position dans le tableau.

**La fin.** Quand le crawl se termine, l'écran charge le résultat complet, comme aujourd'hui, sans changer de filtre.

## 5. robots.txt consultable

**Ce que voit l'utilisateur.** Sur l'écran d'un crawl, un bouton « robots.txt » ouvre une vue avec :
- le contenu du fichier, tel qu'il a été lu, ou « introuvable » ;
- les sitemaps qu'il déclare ;
- les adresses rencontrées pendant le crawl et bloquées par robots.txt, avec la page où chacune a été trouvée.

**D'où viennent les adresses bloquées.** Du rapport de SiteOne, qui les note avec leur raison, et de la liste des adresses écartées en mode liste. Nous ne les recalculons pas nous-mêmes. Deux limites, que la vue indique :
- un crawl arrêté ou limité n'a pas de rapport SiteOne (« liste disponible pour un crawl terminé ») ;
- SiteOne ne note pas les fichiers (images, scripts, feuilles de style) : seules les pages bloquées sont listées.

**Si « respecter robots.txt » est décoché**, la vue le rappelle : rien n'a été bloqué.

## Ce qui est laissé au bâtisseur

- La forme des nouveaux champs enregistrés, tant qu'un ancien crawl s'ouvre sans erreur et sans nouveau constat faux.
- La façon de donner à l'app l'accès aux pages en cours de collecte. La relecture a vérifié le code : le collecteur sait donner ses pages à tout moment, mais sa poignée et le sitemap restent aujourd'hui à l'intérieur du moteur. Il faut un emplacement partagé, que le moteur alimente et qu'une commande lit. La copie des pages se fait sous le verrou que le crawl prend à chaque page : elle doit rester brève, et le calcul se fait hors du verrou.
- La lecture des adresses bloquées dans le rapport de SiteOne. Vérifié : table `skipped`, raison `Robots.txt`, adresses écrites sans le nom du site (à reconstruire avec l'origine du crawl), page source dans la colonne `sourceUqId`.
- La sonde soft 404. Vérifié : le client des sondes ne suit pas les redirections et ne lit pas le corps. Il faut suivre les redirections à la main ou par un second client, lire le corps dans la limite de 1 Mo, et ne pas envoyer l'en-tête `Accept-Encoding` de la sonde de compression. L'extraction existante (`extract_page`) donne le title, le H1 et l'empreinte, comparables à ceux des pages crawlées.
- Le relevé de toutes les balises canonical demande un nouveau champ des faits de page et un numéro de version des faits, pour ne pas signaler les anciens crawls.
- Le nom des classes CSS et la place exacte du résumé dans le panneau, dans l'habillage Console.
- Le découpage en tâches. Je suggère : contrôles simples, soft 404, type et thème, vue d'ensemble, résultats pendant le crawl, robots.txt.

## Mes choix à vérifier

Chacun est suivi de ce qu'il coûte s'il est faux.

1. **Les sept entrées du catalogue**, avec leurs niveaux et leurs textes. Si c'est faux : des lignes du catalogue à changer.
2. **Le type et le thème de chaque code.** J'ai classé « Meta description absente » en opportunité, comme Screaming Frog, alors que son niveau est moyenne. Si c'est faux : une case du tableau à déplacer.
3. **`listedUrlError` répond à la question laissée ouverte par le plan 3.** Si c'est faux : un code à retirer.
4. **Le soft 404 ne compare que le title et le H1, ou l'empreinte du texte.** Si c'est trop prudent : des fausses pages d'erreur dont le title change ne sont pas vues.
5. **Une seule requête de plus par crawl pour le soft 404**, vers une adresse au nom reconnaissable dans les journaux du site (`arpenteur-introuvable-…`). Si c'est faux : un nom plus neutre.
6. **« Plusieurs balises canonical » se déclenche même si elles disent la même chose.** Si c'est trop sévère : ne signaler que les adresses différentes.
7. **H1 trop long à partir de 71 caractères**, le seuil de Screaming Frog. Si c'est faux : un nombre à changer.
8. **Le panneau reste à gauche.** Le message parle du panneau de droite de Screaming Frog. Si c'est faux : une colonne à déplacer.
9. **Pas de détail d'URL ni d'export pendant le crawl.** Si c'est faux : un instantané plus lourd à transmettre.
10. **« Page orpheline » n'apparaît qu'à la fin du crawl.** Si c'est faux : un constat qui clignote pendant le crawl.
11. **Un instantané toutes les 2 secondes.** Si c'est trop : un intervalle à allonger.
12. **robots.txt introuvable est un avertissement de niveau basse.** Si c'est faux : une ligne du catalogue à changer.
13. **Pendant le crawl, un canonical dont la cible n'est pas encore lue n'est pas affiché.** Si c'est faux : un constat qui change de nom en cours de route.
14. **robots.txt « introuvable » veut dire 404 ou 410**, pas une erreur 5xx ni une panne de réseau. Si c'est trop prudent : un site dont le robots.txt est en erreur 500 n'a pas le constat.
15. **La liste des adresses bloquées vient de SiteOne seul.** Elle manque pour un crawl arrêté et ignore les fichiers. Si c'est faux : un calcul à nous, à ajouter.
16. **Les deux contrôles de H1 valent aussi pour les anciens crawls.** Si c'est faux : les réserver aux nouveaux crawls.
17. **Aucun soft 404 n'est signalé quand plus de la moitié des pages ressemblent à la page d'erreur.** Si c'est faux : un site réellement rempli de fausses pages d'erreur n'a que le constat de site.
