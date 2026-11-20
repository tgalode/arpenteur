# Arpenteur : habillage « Console »

Conception courte, version 1 · 20 novembre 2026 · statut : validée

## Pourquoi

Le plan 1 a livré une interface sans feuille de style : le rendu par défaut du navigateur (Times, boutons gris, aucune couleur). L'outil marche, mais il n'est pas agréable à utiliser ni à montrer.

## Ce qu'on veut

Les deux écrans (accueil et crawl) habillés dans la direction « Console », choisie le 20 novembre parmi trois maquettes (Atelier, Console, Cartographe) :
- fond sombre, tout en chasse fixe, interface dense ;
- textes en français et en minuscules, façon console (« lancer », « arrêter », « statut », « liens entrants ») ;
- statuts en couleur, sans pastille : 2xx vert, 3xx bleu, 4xx, 5xx et erreurs réseau rouge, « ignorée » gris ; accent vert citron réservé au bouton principal et à la progression.

Maquette de référence : `docs/superpowers/specs/assets/2026-11-20-console-maquette.html` (fragment HTML, à ouvrir dans un navigateur) (accueil, crawl en cours, crawl interrompu).

**Hors périmètre** : mode clair, colonne des problèmes (elle arrive avec le plan 2 ; d'ici là le tableau prend toute la largeur), toute autre fonctionnalité.

## Les choix

- **Une feuille `src/styles.css`** avec les couleurs, espacements et rayons en variables CSS, et des classes par composant. Les styles en ligne disparaissent, sauf le positionnement des lignes du tableau virtualisé. Écarté : Tailwind ou une bibliothèque de composants, trop d'outillage pour quatre composants.
- **Thème sombre seul** en v1. Les variables permettront d'ajouter un mode clair sans reprise.
- **JetBrains Mono embarquée** (paquet `@fontsource/jetbrains-mono`, licence OFL), pour un rendu identique sur Mac et Windows et sans réseau. Repli : `"SF Mono", Menlo, Consolas, monospace`.
- **Tous les textes restent dans `fr.ts`**, passés en minuscules. Les tests front suivent.
- **Un bouton « relancer »** sur un crawl terminé, arrêté, interrompu ou en échec : il relance un crawl sur la même URL de départ et ouvre le nouveau crawl. C'est le seul ajout fonctionnel, validé avec le design.

## Valeurs de référence (reprises de la maquette)

| Rôle | Valeur |
|---|---|
| fond / panneau / colonne | `#0d0f12` / `#111418` / `#0f1216` |
| traits | `#22262d`, lignes du tableau `#161a1f`, champs `#2d333b` |
| texte / fort / atténué | `#c9d1d9` / `#e6edf3` / `#7d8590` |
| accent / encre sur accent | `#a3e635` / `#0d0f12` |
| 2xx / 3xx / avertissement / erreur | `#7ee787` / `#79c0ff` / `#e3b341` / `#ff7b72` |
| ligne sélectionnée | `#1b2a12` |
| taille de texte de base | 12 px, interligne 1,4 |

## Ce qui reste au constructeur

Les espacements exacts, le rendu des états survol et focus (visibles au clavier), et la hauteur du tableau, qui doit maintenant occuper la hauteur disponible de la fenêtre au lieu des 360 px fixes.
