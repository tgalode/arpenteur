# Arpenteur

Arpenteur est un crawler SEO pour poste de travail (Windows et macOS). Il parcourt un site, liste ses URL et signale les problèmes de référencement, du plus grave au plus léger.

Il est construit avec Tauri et React. Le moteur de crawl est [SiteOne Crawler](https://github.com/janreges/siteone-crawler), utilisé comme bibliothèque Rust.

## Ce que fait l'outil

- Il crawle un site à partir de son adresse et liste toutes les URL trouvées.
- Il crawle aussi une liste d'URL (une adresse par ligne, un seul site à la fois) sans suivre les liens : utile pour revérifier quelques pages.
- Il repère les problèmes du catalogue (38 contrôles) et les regroupe par niveau : haute, moyenne, basse, information.
- Il exporte les résultats en CSV, lisible directement dans Excel en français.
- Il a des réglages : nombre de connexions, débit, limite de requêtes, motifs à inclure ou à ignorer, respect du robots.txt.
- Il vérifie les liens externes cassés : cette vérification envoie une requête par lien externe aux sites tiers, et se désactive dans les réglages.
- Les crawls sont enregistrés sur votre poste. Rien n'est envoyé ailleurs.

Un écran « à propos » donne la version et la licence du moteur de crawl.

## Installation

Téléchargez l'installeur correspondant à votre système, puis lancez-le. Les installeurs ne sont pas signés par un certificat reconnu par Apple ou Microsoft (c'est payant) : macOS et Windows affichent donc un avertissement au premier lancement.

### macOS

1. Ouvrez l'installeur `.dmg` et glissez Arpenteur dans le dossier Applications.
2. Lancez Arpenteur. macOS refuse de l'ouvrir et affiche un avertissement : fermez-le.
3. Ouvrez Réglages Système, puis Confidentialité et sécurité.
4. Descendez jusqu'au message qui parle d'Arpenteur et cliquez sur « Ouvrir quand même ».
5. Confirmez. Les lancements suivants ne demandent plus rien.

### Windows

1. Lancez l'installeur `.exe`.
2. Windows SmartScreen affiche « Windows a protégé votre ordinateur ».
3. Cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».
4. Suivez l'installation. Les lancements suivants ne demandent plus rien.

## Développement

Prérequis : Node.js, Rust (version stable) et les [dépendances système de Tauri](https://v2.tauri.app/start/prerequisites/).

```sh
npm install          # dépendances du front
npm run tauri dev    # lance l'application en mode développement
cargo test --workspace   # tests Rust
npm test             # tests du front (Vitest)
```

Organisation du dépôt :

- `crates/arpenteur-core` : le moteur : crawl (qui encapsule SiteOne Crawler), contrôles qui détectent les problèmes, vérification des cibles de canonical, enregistrement des crawls et des réglages par défaut ;
- `src-tauri` : l'application Tauri : commandes appelées par l'interface, crawl en cours, arrêt propre à la fermeture ;
- `src` : l'interface React : catalogue des problèmes (niveaux et textes français dans `src/i18n/fr.ts`), export CSV.

## Publication d'une version

Une version est construite par l'intégration continue quand on pousse un tag `X.Y.Z`, sans préfixe `v` (par exemple `0.2.0`).

1. Changez la version dans `package.json`, `src-tauri/tauri.conf.json` et `src-tauri/Cargo.toml`.
2. Faites un commit.
3. Posez le tag : `git tag 0.2.0`.
4. Poussez le tag : `git push origin 0.2.0`.
5. Attendez la fin du workflow « Release » dans l'onglet Actions de GitHub. Il s'arrête tout de suite si le tag diffère de la version de l'un des trois fichiers qui la portent (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`).
6. Ouvrez la release en brouillon, vérifiez les fichiers, puis publiez-la.

Le workflow produit un `.dmg` pour les Mac Apple Silicon et un installeur `.exe` NSIS pour Windows. Ni l'un ni l'autre n'est signé.

Ce workflow ne peut tourner qu'une fois le dépôt GitHub créé et le code poussé dessus.

## Licences

Arpenteur utilise SiteOne Crawler, sous licence MIT. Le texte complet est dans [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) et dans l'écran « à propos ».
