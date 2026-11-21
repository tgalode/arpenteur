# Arpenteur, plan 3 : bilan

21 novembre 2026 · branche `feature/plan-3-fiabilite-liens-liste` (de `b6dd3f4` à `f316d55`, 17 commits, plus ce bilan)

## En bref

Le plan 3 est implémenté :
- 8 tâches, chacune relue par un relecteur indépendant ;
- une revue finale de toute la branche, sans défaut critique ;
- une vague de corrections sur huit points, relue elle aussi.

**Tests**
- 232 tests Rust : cœur, crawls de bout en bout sur un site de test local, anciens crawls, app Tauri.
- 146 tests front.
- TypeScript et builds propres.

**Ce que l'app fait en plus**
- **Des constats plus justes.** Six corrections : crawl limité, pages en noindex, titres du pied de page, en-têtes de sécurité, sonde qui respecte robots.txt, lignes sondées « non analysé ».
- **Un tableau complet.** Un bouton « colonnes » pour choisir parmi 16 colonnes, et un export CSV qui les contient toutes.
- **Les liens externes cassés.** Vérifiés après le crawl, avec une progression affichée et une case pour désactiver la vérification. Le catalogue passe à 38 problèmes.
- **Le mode liste.** Une liste d'adresses collée sur l'accueil, crawlée sans suivre les liens.

## Ce qui reste à faire avant la fusion

1. **La vérification à la main dans l'app** (`npm run tauri dev`). Personne n'a encore vu ces écrans dans l'app elle-même :
   - rouvrir le crawl de la chocolaterie : il s'ouvre, et affiche désormais « En-têtes de sécurité manquants » si ses en-têtes enregistrés sont vraiment absents (voir les décisions) ;
   - crawler crocweb.fr : « Lien externe cassé » ne sort que sur des liens réellement morts, et aucun lien vers LinkedIn ou Facebook n'est signalé ;
   - arrêter un crawl pendant la vérification des liens externes : la progression « vérification des liens externes : 12 / 48 » s'affiche, et le crawl revient « arrêté » ;
   - le bouton « colonnes » : cocher « meta description » et « temps », relancer l'app, le choix est gardé ;
   - l'export des URL ouvert dans Excel ou Numbers : 16 colonnes ;
   - le mode liste avec trois adresses collées depuis un tableur sur deux colonnes, dont une qui redirige : la cible porte « hors liste », visible même sur une adresse longue ;
   - décocher « vérifier les liens externes », relancer un crawl : le problème n'apparaît plus ;
   - un site local crawlé hors ligne : aucun lien externe n'est déclaré cassé.
2. **Le dépôt GitHub, la CI et la première release**, toujours jamais lancés (voir le bilan du plan 2).

## Une question pour toi

**En mode liste, une adresse de la liste qui répond 404 n'apparaît pas dans les problèmes.** Elle se voit seulement dans la colonne « statut ». Le catalogue n'a pas de problème pour ce cas : « Lien interne cassé » demande une page qui porte le lien. Or revérifier des pages après une correction est l'usage annoncé du mode liste. Trois voies :
- un 39e problème, « Adresse de la liste en erreur » (ma préférence) ;
- accepter : le statut suffit ;
- reporter au plan 4.

## Décisions prises en cours de route

Chaque décision est suivie de ce qu'elle coûte si elle est fausse. Elles s'ajoutent aux seize choix listés à la fin de la conception.

**Fiabilité**
- **Un ancien crawl peut gagner « En-têtes de sécurité manquants ».** Le constat se calcule maintenant sur les en-têtes enregistrés avec chaque page, que les anciens crawls ont aussi. La conception disait « sans nouveau constat » : celui-ci est vrai (les en-têtes sont réellement absents), mais c'est bien un constat de plus à la réouverture. Si c'est faux : limiter le contrôle aux nouveaux crawls.
- **Le temps de réponse est vide pour une page sans réponse**, au lieu de « 0 ms ». Si c'est faux : une fonction à retirer.
- **Les cibles de canonical qui sont des fichiers (PDF) ne sont plus sondées**, puisque la sonde suit désormais les mêmes règles que le crawl. Si c'est faux : une exception à ajouter.

**Liens externes**
- **Un domaine n'est déclaré disparu que si un autre lien externe a répondu pendant la même vérification.** Sans cette preuve, un poste hors ligne ou un DNS en panne faisait passer tous les liens pour cassés. Si c'est faux : un site dont le seul lien externe pointe vers un domaine mort n'a pas le constat.
- **Un lien externe qui contient des identifiants (`https://user:pass@…`) n'est jamais vérifié**, pour ne pas les envoyer à un site tiers. Si c'est faux : ces liens ne sont pas contrôlés.
- **Un arrêt pendant la vérification donne un crawl « arrêté »**, avec ce qui a déjà été vérifié. Le message « non vérifiés (limite de requêtes atteinte) » ne paraît que si c'est vrai. Si c'est faux : un crawl aux pages complètes s'affiche « arrêté ».
- **Les redirections d'un lien externe ne comptent pas dans la limite de requêtes** et ne respectent pas le débit entre elles : c'est le client HTTP qui les suit, cinq au plus.

**Mode liste**
- **Les images des pages listées sont demandées**, comme pour un site. SiteOne les demande même sans suivre les liens, et le contrôle « image trop lourde » en a besoin. La conception a été amendée. Si c'est faux : un motif pour les écarter, et ce contrôle disparaît en mode liste.
- **Une adresse listée qui redirige met sa page d'arrivée dans le périmètre** : ses problèmes de page, ses doublons et ses liens cassés sont signalés, et la ligne porte « hors liste ». Si c'est faux : des constats sur une page que tu n'as pas tapée.
- **Une ligne collée sur plusieurs colonnes garde sa première colonne.** S'il reste une espace dans l'adresse, la ligne est refusée avec son numéro. Si c'est faux : une adresse placée en deuxième colonne est ignorée.
- **Si toutes les adresses sont écartées, le crawl échoue aussitôt avec un message**, au lieu d'un refus avant le lancement : robots.txt n'est lu qu'au démarrage.
- **Les constats de site d'une liste portent sur une ligne du tableau**, même quand la première adresse a été écartée.

**Organisation**
- **Le travail s'est fait sur une branche, sans worktree.** Coût : aucun.
- **La revue finale a tourné sur un modèle un cran sous le meilleur**, comme au plan 2. Si c'est faux : un défaut subtil a pu passer.

## À ouvrir au début du plan 4

**Relevé par la revue finale, non corrigé**
- Un fichier listé (PDF) est dit écarté « par un motif », alors que tu n'en as posé aucun.
- Une redirection vers un domaine mort affiche « domaine introuvable » sur une adresse dont le domaine existe.
- Les liens vers le réseau local (`localhost`, adresses privées) trouvés sur un site public sont vérifiés depuis ton poste.
- Une CSP ou une politique de referrer posée par balise meta ne compte pas pour « En-têtes de sécurité manquants ».
- Un lien externe vérifié sans réponse ressemble à un lien jamais vérifié dans le détail.
- Le CSV écrit 0 comme nombre de H1 pour une image, et ne dit pas quelles lignes sont « hors liste ».
- Le client HTTP qui lit robots.txt et le sitemap n'a pas de délai maximal (antérieur au plan 3).

**Remarques mineures** (aucune ne fausse un résultat)
- **Interface** : double espace entre la zone de saisie de la liste et son compteur, erreur encore affichée après un changement de mode sur l'accueil, libellés abrégés dans la liste des colonnes.
- **Tests** : un test dépend de la réponse du DNS pour un domaine en `.invalid`, un test de crawl attend le vrai délai de 5 secondes.
- **Code** : messages d'erreur joints différemment selon la commande, `commands.rs` non formaté.

**Prévu pour le plan 4** (conception du plan 3)
- L'aperçu du résultat Google, avec les longueurs en pixels.
- PageSpeed et les Core Web Vitals.
