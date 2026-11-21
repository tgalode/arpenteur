# Arpenteur, plan 3 : conception

21 novembre 2026 · suite du bilan du plan 2 (`docs/superpowers/plans/2026-11-20-arpenteur-plan-2-bilan.md`)

## En deux minutes

**Pourquoi.** Arpenteur doit remplacer Screaming Frog pour auditer des sites de TPE et PME de quelques centaines de pages. Le plan 2 a livré le cœur (37 problèmes, panneau, export, réglages). Il manque encore quatre choses pour que l'outil suffise sur l'essentiel.

**Ce que livre le plan 3.**
1. **Des résultats plus fiables** : six corrections sur des constats discutables, décidées avec toi le 21 novembre.
2. **Un tableau et un export complets** : 16 colonnes dans le CSV, un choix de colonnes dans le tableau.
3. **Les liens externes cassés** : un nouveau problème, vérifié après le crawl.
4. **Le mode liste** : crawler une liste d'URL collée, sans suivre les liens.

**Ce qui attend le plan 4.** L'aperçu du résultat Google (longueurs en pixels) et PageSpeed. Ce sont des écrans nouveaux, et PageSpeed envoie des adresses à Google.

**Ce que tu as décidé, et ce que j'ai choisi.** Le découpage en deux plans et les six corrections de fiabilité sont tes décisions. Tout le reste de ce document est mon choix : tu m'as dit d'avancer sans plus de questions. La liste de ces choix est à la fin, dans « Mes choix à vérifier ».

## Objectifs

- Un audit complet d'un site de quelques centaines de pages se fait sans rouvrir Screaming Frog.
- Chaque constat affiché est vrai. La règle D5 de la conception d'origine reste la boussole : un faux positif coûte plus qu'un contrôle manquant.
- L'export CSV contient tout ce que le crawl sait d'une URL.

## Ce que le plan 3 ne fait pas

- Pas d'aperçu Google ni de PageSpeed (plan 4).
- Pas de validation des données structurées pour les résultats enrichis.
- Pas de filtres par colonne dans le tableau. Le panneau des problèmes couvre déjà « meta description absente » et les cas voisins.
- Pas d'onglet « URL externes » dans le tableau. Les liens externes se lisent dans le problème et dans le détail d'une URL.
- Pas de liste d'URL sur plusieurs sites à la fois.
- Pas de rendu JavaScript, de hreflang ni de comparaison entre deux crawls.

## Ce qui ferait échouer le plan, même s'il marche

- Un constat « Lien externe cassé » sur un lien qui s'ouvre dans un navigateur. Beaucoup de sites (LinkedIn, Facebook, certains pare-feu) répondent 403 ou 999 aux robots : ces réponses ne doivent jamais donner de constat.
- Une vérification des liens externes qui multiplie les requêtes vers un même site tiers sans respecter le débit réglé.
- Un ancien crawl enregistré qui ne s'ouvre plus, ou qui affiche de nouveaux faux positifs.
- Un mode liste qui demande des URL interdites par robots.txt ou exclues par tes motifs.

## Contraintes

- Celles du plan 2 restent valables :
  - tout nouveau champ enregistré porte `#[serde(default)]` ;
  - les textes de l'interface sont dans `fr.ts` ;
  - l'habillage Console est conservé ;
  - chaque contrôle a un test positif et un test négatif sur sa frontière ;
  - toute requête ajoutée respecte le débit réglé et compte dans la limite de requêtes.
- Le catalogue validé fait foi. Le plan 3 lui ajoute un code. Les autres textes ne changent pas.
- SiteOne reste en version `=2.5.1`, utilisé comme bibliothèque.
- Rien n'est poussé et aucun dépôt distant n'est créé sans ton accord.

## Approches

### Liens externes : qui fait les requêtes ?

1. **Une sonde à nous, après le crawl (retenue).** La phase qui vérifie déjà les cibles de canonical et la compression gagne une troisième étape. Elle demande chaque adresse externe une fois, au débit réglé, dans le budget de requêtes restant. Nous décidons de ce qui compte comme cassé.
2. **Laisser SiteOne crawler les domaines externes** (`--allowed-domain-for-crawling` avec `--single-foreign-page`). Écartée : SiteOne téléchargerait et analyserait les pages tierces, leurs lignes se mêleraient aux pages du site, et nous ne maîtriserions ni le débit par site tiers ni la règle de ce qui est cassé.
3. **Une requête par lien pendant le crawl.** Écartée : elle ralentit le crawl du site audité et complique l'arrêt.

### En-têtes de sécurité : d'où vient l'information ?

1. **Les en-têtes de la page de départ, que nous avons déjà (retenue).** Nous regardons nous-mêmes si cinq en-têtes sont présents. Le détail est en français, et un crawl arrêté a aussi ce constat.
2. **Garder la table `security` de SiteOne en la filtrant.** Écartée : ses textes sont en anglais, elle mêle en-têtes absents, en-têtes bavards et cookies, et il faudrait reconnaître ses messages un par un.

### Mode liste : qui crawle ?

1. **SiteOne, avec sa liste d'URL et sans suivi des liens (retenue).** Les pages passent par la même extraction et les mêmes contrôles que pour un crawl de site. Nous filtrons la liste nous-mêmes avant de la lui passer, parce que SiteOne ne lui applique ni robots.txt ni les motifs.
2. **Une boucle de requêtes à nous.** Écartée : elle dupliquerait l'extraction, la détection du site qui limite et l'arrêt.

## 1. Fiabilité (décidé avec toi)

| # | Aujourd'hui | Après le plan 3 |
|---|---|---|
| 1 | La sonde des canonical ignore robots.txt et tes motifs à inclure ou à ignorer. | Une cible interdite ou exclue n'est pas demandée. La page garde « Canonical vers une autre page » (information). |
| 2 | Une cible sondée qui répond 200 s'affiche « indexable : non », avec le type `html`. | « Indexable » a trois valeurs : oui, non, non analysé. Une ligne sondée est « non analysé ». Son type reprend celui annoncé par le serveur (PDF, image). |
| 3 | « En-têtes de sécurité manquants » regroupe des en-têtes absents, des en-têtes bavards et des cookies, avec un détail en anglais. | Le constat ne cite que les en-têtes absents, nommés dans le détail en français. |
| 4 | Les 503 et 429 d'un crawl « limité » deviennent des « Erreur serveur ». | Sur un crawl limité, ces deux statuts ne donnent pas de constat. Les autres 5xx restent signalés. |
| 5 | Les contrôles de title, de meta description et de H1 tournent sur les pages en noindex. | Ces contrôles ne tournent plus sur une page en noindex. Le constat « Page en noindex » reste. |
| 6 | « Niveaux de titres sautés » compte les titres du pied de page. | Les titres placés dans `footer`, `nav` et `aside` ne comptent plus. |

Précisions, qui sont mes choix :

- **Point 2.** Les lignes sondées portent une marque enregistrée avec le crawl. Dans le tableau et le CSV, la colonne « indexable » écrit « oui », « non » ou « non analysé », sans couleur d'alerte pour ce dernier. Une adresse du sitemap qui n'a été que sondée ne donne pas « URL du sitemap non indexable ». Les lignes sondées restent dans le tableau, mais ne comptent plus dans le nombre d'URL du crawl affiché sur l'accueil.
- **Point 3.** Les cinq en-têtes regardés : `Strict-Transport-Security`, `Content-Security-Policy`, `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`. `X-Frame-Options` compte comme présent si la CSP contient `frame-ancestors`. `Strict-Transport-Security` n'est attendu que sur un site en HTTPS. La page regardée est la page de départ, ou la première page HTML interne en 200 si elle redirige. Le constat reste un constat de site, de niveau basse, avec le titre et les textes du catalogue.
- **Point 5.** Une page est « en noindex » par sa balise meta robots ou par l'en-tête `X-Robots-Tag`, comme pour le constat existant. Les huit constats concernés : title absent, trop long, trop court ; meta description absente, trop longue, trop courte ; H1 absent ; plusieurs H1. Les doublons de title et de meta description ne changent pas : ils ne portent déjà que sur les pages indexables. Les autres contrôles (texte alternatif des images, langue, niveaux de titres, téléphone, contenu court) continuent de tourner, parce qu'ils concernent le visiteur.
- **Point 6.** La règle ne touche que le contrôle des niveaux sautés. La détection des H1 (absent, plusieurs) et les colonnes « h1 » et « nombre de h1 » ne changent pas. Elle s'applique à l'extraction : elle vaut pour les nouveaux crawls. Un crawl déjà enregistré garde ses titres tels qu'ils ont été lus.

## 2. Tableau et export

**Le CSV des URL exporte 16 colonnes**, quel que soit l'affichage : url, statut, type, title, longueur du title, meta description, longueur de la meta description, h1, nombre de h1, canonical, indexable, profondeur, liens entrants, mots, poids (octets), temps de réponse (ms).

- Les longueurs sont en caractères, comptées comme dans les contrôles (espaces réduits).
- Une valeur absente donne une cellule vide.

**Le tableau garde ses 9 colonnes par défaut.** Un bouton « colonnes » ouvre une liste de cases à cocher pour afficher les autres. Le choix est retenu d'un lancement à l'autre, sur le poste.

```
[ rechercher… ]  [ statut ▾ ]  [ colonnes ▾ ]            exporter les URL
url            statut  title        meta description     canonical   poids   temps
/              200     Chocolat…    Artisan chocola…     /           84 Ko   212 ms
/devis/        200     Devis        –                    /devis/     41 Ko   180 ms
```

- Le tri fonctionne sur toutes les colonnes.
- La colonne « url » ne peut pas être masquée.
- Le temps de réponse n'était pas transmis à l'interface : il le devient.

## 3. Liens externes cassés

**Ce que voit l'utilisateur.** Un nouveau problème, « Lien externe cassé », dans le panneau. Un clic filtre le tableau sur les pages qui portent ces liens. Le détail d'une page liste chaque lien cassé avec son adresse. Dans la liste des liens sortants du détail, chaque lien externe vérifié affiche son statut.

**Entrée du catalogue (à valider par toi).**

| Identifiant | Niveau | Titre | Ce que ça veut dire | Comment corriger |
|---|---|---|---|---|
| `brokenExternalLink` | moyenne | Lien externe cassé | Une page du site pointe vers une adresse d'un autre site qui n'existe plus. Le visiteur qui clique tombe sur une erreur. | Corriger le lien ou le retirer. |

Le catalogue passe à 38 codes : 7 en haute, 15 en moyenne, 12 en basse, 4 en information.

**Ce qui est vérifié.** Les adresses `http` et `https` des liens `<a href>` trouvés sur les pages internes, quand elles pointent hors du site. Chaque adresse distincte est demandée une seule fois. Les images, scripts et feuilles de style externes ne sont pas vérifiés.

**Quand.** Après un crawl terminé, dans la phase qui vérifie déjà les canonical. Pas après un arrêt ni une interruption. L'ordre dans le budget restant : compression, cibles de canonical, puis liens externes.

**Comment.**
- Une requête GET par adresse, sans lire le corps, avec le User-Agent du crawl.
- Les redirections sont suivies, cinq au plus. C'est la réponse finale qui compte.
- Le délai maximal est de 5 secondes, comme pour les canonical.
- Le débit réglé s'applique entre deux requêtes.
- Le robots.txt des sites tiers n'est pas lu. C'est une seule requête par lien, comme un visiteur qui clique, et c'est l'usage des vérificateurs de liens.

**Ce qui compte comme cassé.** Seulement deux cas :
- la réponse finale est 404 ou 410 ;
- le nom de domaine n'existe plus, après un second essai.

Tout le reste ne donne pas de constat : 401, 403, 429, 999, les 5xx, un délai dépassé, une connexion refusée. Ces réponses viennent trop souvent d'un site qui bloque les robots ou d'une panne passagère.

**Le budget.** Ces requêtes comptent dans la limite de requêtes. Si le budget s'épuise, les liens restants ne sont pas vérifiés et l'écran du crawl l'indique : « 12 liens externes non vérifiés (limite de requêtes atteinte) ».

**Le réglage.** L'écran Réglages gagne une case « vérifier les liens externes », cochée par défaut.

**L'enregistrement.** Les résultats vont dans une liste à part du crawl (adresse, statut final, adresse finale), pas dans les pages. Ils n'apparaissent donc pas comme lignes du tableau et ne faussent aucun compteur.

## 4. Mode liste

**À quoi il sert.** Revérifier quelques pages après une correction, ou auditer un lot de pages d'atterrissage sans crawler tout le site.

**Ce que voit l'utilisateur.** Sur l'accueil, un choix entre « site » et « liste d'URL ». En mode liste, une zone de texte reçoit une URL par ligne.

```
( ) site    (•) liste d'URL
┌──────────────────────────────────────┐
│ https://exemple.fr/                  │
│ https://exemple.fr/tarifs            │
│ https://exemple.fr/contact           │
└──────────────────────────────────────┘
3 URL · [ lancer ]
```

**Règles.**
- Les lignes vides et les doublons sont ignorés.
- Toutes les URL doivent être en `http` ou `https` et sur le même site que la première. Sinon le lancement est refusé, avec la liste des lignes fautives.
- La liste ne peut pas dépasser la limite de requêtes.
- Les URL interdites par robots.txt ou exclues par tes motifs ne sont pas demandées. L'écran du crawl dit combien ont été écartées et pourquoi. Si toutes le sont, le lancement est refusé avec un message.
- Les liens ne sont pas suivis et le sitemap n'est pas lu. Les seules adresses demandées en plus de la liste sont les cibles de redirection : si `/tarifs` redirige vers `/tarifs/`, la cible est demandée et apparaît dans le tableau avec la mention « hors liste ». On voit ainsi où mène chaque redirection, et les constats de redirection restent justes.

**Ce qui change dans les résultats.**
- Les contrôles qui ont besoin du site entier ne tournent pas : page orpheline, page absente du sitemap, sitemap introuvable, URL du sitemap en erreur ou non indexable.
- Les doublons (title, meta description, contenu) sont cherchés parmi les pages de la liste.
- « Profondeur » et « liens entrants » affichent « – ».
- Les sondes après crawl tournent comme pour un site : compression, cibles de canonical, liens externes.
- Les liens internes cassés ne sont signalés que si la cible est dans la liste.
- Les constats qui décrivent le serveur restent : certificat HTTPS, en-têtes de sécurité et compression. Ils sont regardés sur la première URL demandée.
- Dans le CSV, « profondeur » et « liens entrants » sont des cellules vides.
- La limite de requêtes s'applique comme pour un site : les sondes après crawl disposent de ce qui reste une fois les pages demandées.

**L'enregistrement.** Le crawl est rangé sous le site de la première URL, avec son mode et sa liste. La liste des crawls affiche « liste, 3 URL ». « relancer » rejoue la même liste avec les réglages enregistrés.

## Ce qui est laissé au bâtisseur

- Le branchement du mode liste sur SiteOne. La relecture de la conception a vérifié l'hypothèse dans son code : la liste passe par `url_list_urls` (déjà utilisé pour les adresses du sitemap), et `single_page` coupe le suivi des liens. `max_depth` ne suffit pas. Deux pièges relevés : SiteOne demande toujours l'URL de départ sans la filtrer, il faut donc lui donner pour départ la première URL de la liste qui passe robots.txt et les motifs ; et il suit les redirections, d'où la mention « hors liste ».
- La façon de reconnaître un nom de domaine inexistant dans les erreurs de `reqwest`.
- La forme des nouveaux champs enregistrés, tant qu'un ancien crawl s'ouvre sans erreur et sans nouveau constat.
- L'endroit où le choix des colonnes est retenu (stockage du navigateur intégré ou fichier de réglages).
- Le découpage en tâches et leur ordre. Je suggère : fiabilité, tableau et export, liens externes, mode liste.

## Mes choix à vérifier

Tu ne les as pas décidés. Chacun est suivi de ce qu'il coûte s'il est faux.

1. **Un lien externe n'est « cassé » que sur 404, 410 ou domaine inexistant.** Si c'est trop prudent : des liens en 500 durable ne sont pas signalés.
2. **`brokenExternalLink` est de niveau moyenne**, un cran sous le lien interne cassé (haute). Si c'est faux : une ligne du catalogue à changer.
3. **Le robots.txt des sites tiers n'est pas lu** pour vérifier un lien. Si c'est faux : une requête de plus par site tiers.
4. **Les liens externes comptent dans la limite de requêtes**, après les canonical. Si c'est faux : sur un gros site, ils ne sont pas tous vérifiés.
5. **Seuls les liens `<a href>` sont vérifiés**, pas les images ni les scripts externes. Si c'est faux : une image externe disparue n'est pas signalée.
6. **Les en-têtes de sécurité sont lus par nous sur la page de départ**, avec une liste de cinq. Si c'est faux : la liste est à ajuster.
7. **Le tableau a un bouton « colonnes » mais pas de filtres par colonne.** Si tu travailles surtout dans l'app : les filtres manqueront.
8. **Le mode liste n'accepte qu'un seul site à la fois.** Si c'est faux : le rangement des crawls par site est à revoir.
9. **En mode liste, les contrôles de site entier ne tournent pas**, et profondeur et liens entrants sont vides. Si c'est faux : des constats trompeurs réapparaissent.
10. **La case « vérifier les liens externes » est cochée par défaut.** Si c'est faux : chaque crawl fait des requêtes vers des sites tiers sans que tu l'aies demandé.
11. **En mode liste, la cible d'une redirection est demandée et affichée « hors liste ».** Si c'est faux : le tableau contient des lignes que tu n'as pas demandées.
12. **En mode liste, les constats de serveur restent** (certificat, en-têtes de sécurité, compression). Si c'est faux : trois constats de site sur une liste de trois pages.
13. **Sur une page en noindex, seuls les huit constats de title, de meta description et de H1 s'arrêtent.** Si c'est faux : il reste du bruit (images, langue) sur ces pages, ou au contraire il en manque.
14. **Les titres de `footer`, `nav` et `aside` sortent seulement du contrôle des niveaux sautés**, pas de la détection des H1. Si c'est faux : un H1 placé dans un menu compte toujours.
15. **Les lignes sondées ne comptent plus dans le nombre d'URL d'un crawl.** Si c'est faux : un compteur à remettre.
