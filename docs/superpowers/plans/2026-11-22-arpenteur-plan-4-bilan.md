# Arpenteur, plan 4 : bilan

23 novembre 2026 · branche `feature/plan-4-audit-en-direct` (de `1d07c91` à `13a855c`, 15 commits, plus ce bilan)

## En bref

Le plan 4 est implémenté :
- 6 tâches, chacune relue par un relecteur indépendant ;
- une revue finale de toute la branche, sans défaut critique ;
- une vague de corrections sur neuf points, relue elle aussi.

**Tests**
- 284 tests Rust : cœur, crawls de bout en bout sur un site de test local, anciens crawls, app Tauri.
- 206 tests front.
- TypeScript, builds et format du code propres.

**Ce que l'app fait en plus**
- **Sept contrôles de plus.** H1 en double, H1 trop long, plusieurs balises canonical, fausses pages d'erreur (soft 404, deux contrôles), adresse de la liste en erreur, robots.txt introuvable. Le catalogue passe à 45 problèmes.
- **Un type sur chaque problème** : problème, avertissement ou opportunité, dans le panneau, le détail d'une URL et le CSV.
- **Une vue d'ensemble.** Un résumé du crawl en haut du panneau, et un classement des problèmes par thème au choix.
- **Les résultats pendant le crawl.** Le tableau et le panneau se remplissent au fil de l'eau, avec la mention « résultats provisoires ».
- **robots.txt consultable** : son contenu, ses sitemaps, les adresses qu'il a bloquées.

## Ce qui reste à faire avant la fusion

1. **La vérification à la main dans l'app** (`npm run tauri dev`). Personne n'a encore vu ces écrans dans l'app elle-même :
   - lancer un crawl de crocweb.fr : le tableau et le panneau se remplissent, « résultats provisoires » s'affiche, un clic sur une ligne dit que le détail attend la fin ;
   - à la fin : le filtre choisi pendant le crawl est gardé, et « Page orpheline » n'apparaît qu'à ce moment ;
   - le résumé en haut du panneau : un clic sur « 4xx » filtre le tableau, et le classement « thème » est gardé après un redémarrage ;
   - chaque problème porte son type, dans le panneau, le détail et le CSV ;
   - rouvrir le crawl de la chocolaterie : il s'ouvre, sans « soft 404 » ni « robots.txt introuvable » ;
   - crawler un site en tapant son domaine sans `www` : la sonde d'adresse inexistante doit viser l'hôte réellement crawlé ;
   - le bouton « robots.txt » : le fichier, ses sitemaps, les adresses bloquées ; après fermeture, le tableau a gardé sa position ;
   - le mode liste avec une adresse en 404 : « Adresse de la liste en erreur » ;
   - un crawl de plusieurs milliers de pages : l'app reste fluide pendant le crawl.
2. **Le dépôt GitHub, la CI et la première release**, toujours jamais lancés (voir le bilan du plan 2).

## Ce qu'il te faut valider

Le catalogue est le tien. Deux parties ont été codées sur mes propositions :
- **les sept nouvelles entrées** (niveau, titre, explication, correction), dans la conception du plan 4, section 1 ;
- **le type et le thème des 45 problèmes**, dans le tableau de la section 2.

## Une question pour toi

**En mode liste, les garde-fous du soft 404 peuvent cacher une fausse page d'erreur.** Pour éviter de signaler à tort une page légitime, deux règles coupent le contrôle : quand la page de départ ressemble à la page d'erreur, et quand plus de la moitié des pages lui ressemblent. Pour un site, c'est prudent. Pour une liste, la « page de départ » est la première adresse collée : si c'est elle la fausse page d'erreur, ou si tu colles une liste de pages supprimées, rien n'est signalé. Deux voies : accepter (le statut et « Adresse de la liste en erreur » couvrent les vraies 404), ou écrire une règle propre au mode liste.

## Décisions prises en cours de route

Chaque décision est suivie de ce qu'elle coûte si elle est fausse. Elles sont aussi dans la conception, choix 17 à 26.

**Soft 404**
- **Aucune page n'est signalée quand la page de départ ressemble à la page d'erreur**, ni quand plus de la moitié des pages lui ressemblent. Sans cela, un thème qui met le nom du site partout faisait signaler l'accueil. Si c'est faux : une fausse page d'erreur passe inaperçue sur ces sites.
- **Quand le H1 de la page d'erreur est aussi celui de la page de départ (un logo), seul le texte des pages est comparé.** Si c'est faux : une fausse page d'erreur au texte différent n'est pas vue.
- **La sonde vise l'hôte réellement crawlé**, pas l'adresse tapée. Sinon elle était redirigée dès qu'on tapait le domaine sans `www`, et le contrôle restait muet.
- **Son adresse finale est enregistrée sans ses paramètres** : une redirection vers une page de connexion peut porter un jeton.
- **Une redirection vers un autre site qui répond 200 compte comme « le site répond 200 aux adresses inexistantes ».** Si c'est faux : une vérification d'hôte à ajouter.

**Résultats pendant le crawl**
- **Les réponses 429 et 503 ne comptent jamais comme des erreurs pendant le crawl**, et un canonical vers une telle page reste caché. Si le site freine le crawl, ces constats auraient disparu à la fin. Ils apparaissent à la fin dans le cas contraire. Si c'est faux : de vraies erreurs 503 ne se voient qu'une fois le crawl fini.
- **Les doublons n'indiquent pas leurs pages jumelles pendant le crawl.** Sur un site de mille pages au même H1, chaque instantané aurait pesé des dizaines de Mo. La liste arrive avec le résultat final.
- **« Page orpheline » et un canonical dont la cible n'est pas encore lue n'apparaissent qu'à la fin**, comme prévu par la conception.

**Interface**
- **La vue robots.txt se pose par-dessus le crawl**, qui reste en place dessous, pour ne pas perdre la position du tableau.
- **Elle distingue « introuvable » (404 ou 410), « illisible » (autre réponse), « non lu » et « fichier vide ».**
- **Le résumé compte les lignes du tableau**, lignes sondées comprises, avec une ligne « ignorées » quand il y en a, pour que les nombres tombent juste.
- **Le panneau reste à gauche**, alors que le message vocal parlait du panneau de droite de Screaming Frog.

**Organisation**
- **Le travail s'est fait sur une branche, sans worktree.** Coût : aucun.
- **La revue finale a tourné sur un modèle un cran sous le meilleur**, comme aux plans 2 et 3. Si c'est faux : un défaut subtil a pu passer.

## À ouvrir au début du plan 5

**Prévu pour le plan 5** (conception du plan 4)
- Hreflang, pagination, données structurées détaillées, visualisation du maillage.

**Relevé par la revue finale, non corrigé**
- Les listes de pages jumelles des doublons pèsent lourd aussi dans le résultat final et le CSV, sur un gros site.
- Pendant le crawl, chaque instantané copie toutes les pages collectées : à rendre incrémental pour les crawls de plusieurs milliers de pages.
- Un constat de site peut changer brièvement de ligne pendant le crawl, avant l'arrivée de la page de départ.
- Dans la vue robots.txt, une adresse bloquée sur un hôte voisin ou un autre port est réécrite sur l'adresse de départ.
- Le client HTTP qui lit robots.txt et le sitemap n'a toujours pas de délai maximal.

**Points du plan 3 toujours ouverts**
- Les liens vers le réseau local trouvés sur un site public sont vérifiés depuis ton poste.
- Une CSP posée par balise meta ne compte pas pour « En-têtes de sécurité manquants ».

**Plus tard**
- Problèmes de JavaScript, Search Console et Analytics, aperçu Google en pixels, PageSpeed.
