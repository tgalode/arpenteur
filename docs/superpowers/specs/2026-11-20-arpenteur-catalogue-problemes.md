# Arpenteur : catalogue des problèmes

Version 1 · 20 novembre 2026 · statut : validé le 20 novembre, complété avec le plan 3 par `brokenExternalLink`

Ce catalogue alimente le panneau Problèmes du plan 2. Chaque problème a un identifiant stable, un niveau, un titre, une explication et une correction. Les textes sont écrits pour pouvoir figurer tels quels dans un rapport client.

**Les niveaux**
- **Haute** : la page est en erreur, ou Google reçoit un signal contradictoire qui peut la faire disparaître.
- **Moyenne** : ça pénalise le référencement ou la qualité, sans bloquer.
- **Basse** : bonne pratique à corriger quand on a le temps.
- **Information** : un constat souvent voulu. Il est affiché, mais ne compte pas dans les problèmes.

**Les sources**
- **nous** : contrôle déjà calculé par Arpenteur.
- **nous (à ajouter)** : les données sont déjà relevées, la règle reste à écrire.
- **SiteOne** : repris de l'analyse SiteOne.

Les seuils viennent de la conception (D5) et restent réglables.

## Haute

| Identifiant | Titre | Pourquoi | Correction | Source |
|---|---|---|---|---|
| `brokenInternalLink` | Lien interne cassé | Une page du site pointe vers une adresse en erreur. Les visiteurs et Google tombent sur une impasse. | Corriger le lien, le retirer, ou rediriger l'adresse cible. | nous |
| `serverError` | Erreur serveur (5xx) | La page ne répond pas. Google finit par la retirer de l'index si l'erreur dure. | Corriger l'erreur côté serveur. | nous (à ajouter) |
| `canonicalToError` | Canonical vers une page en erreur | La page désigne comme référence une adresse qui n'existe pas. Google ignore ce signal. | Pointer le canonical vers l'adresse valide de la page. | nous |
| `canonicalToRedirect` | Canonical vers une redirection | La page désigne comme référence une adresse qui redirige. Les signaux envoyés à Google se contredisent. | Pointer le canonical vers l'adresse finale. | nous |
| `sitemapUrlError` | Adresse du sitemap en erreur | Le sitemap annonce à Google une page qui n'existe plus. | Retirer l'adresse du sitemap ou la corriger. | nous |
| `titleMissing` | Title absent | Google invente un titre pour le résultat de recherche. | Écrire un title unique et descriptif. | nous (à ajouter) |
| `httpsInvalid` | Certificat HTTPS invalide ou bientôt expiré | Les navigateurs affichent une alerte et les visiteurs repartent. | Renouveler ou corriger le certificat. | SiteOne |

## Moyenne

| Identifiant | Titre | Pourquoi | Correction | Source |
|---|---|---|---|---|
| `redirectChain` | Chaîne de redirections | Plusieurs redirections à la suite ralentissent la page et diluent les signaux. | Rediriger directement vers l'adresse finale. | nous |
| `temporaryRedirect` | Redirection temporaire (302, 303, 307) | Google peut garder l'ancienne adresse dans ses résultats. | Passer en redirection permanente (301 ou 308) si le changement est définitif. | nous |
| `canonicalMissing` | Canonical absent | Sans référence, Google choisit seul entre les variantes d'une page (paramètres, majuscules, slash final). | Ajouter un canonical vers l'adresse de la page elle-même. | nous |
| `canonicalToOtherHost` | Canonical vers un autre site | La page cède son référencement à un autre domaine. | Vérifier que c'est voulu, sinon pointer vers la page elle-même. | nous |
| `orphanPage` | Page orpheline | La page est dans le sitemap, mais aucune page du site n'y mène. Google la juge peu importante. | Ajouter des liens internes vers cette page. | nous |
| `duplicateContent` | Contenu identique sur plusieurs pages | Google ne sait pas quelle page montrer et peut n'en garder aucune. | Fusionner les pages ou désigner une page de référence avec le canonical. | nous |
| `titleDuplicate` | Title en double | Plusieurs pages portent le même titre. Google les distingue mal. | Donner à chaque page un title unique. | SiteOne |
| `metaDescriptionMissing` | Meta description absente | Google fabrique lui-même l'extrait affiché sous le titre. | Rédiger une description qui donne envie de cliquer. | nous (à ajouter) |
| `h1Missing` | H1 absent | La page n'a pas de titre principal lisible par Google. | Ajouter un H1 qui résume la page. | nous (à ajouter) |
| `jsonLdInvalid` | Données structurées illisibles | Le bloc JSON-LD contient une erreur. Google l'ignore entièrement. | Corriger la syntaxe du bloc JSON-LD. | nous |
| `sitemapNotFound` | Aucun sitemap trouvé | Google découvre les pages seulement en suivant les liens. | Publier un sitemap.xml et le déclarer dans robots.txt. | nous |
| `sitemapUrlNonIndexable` | Adresse du sitemap non indexable | Le sitemap annonce une page en noindex ou qui désigne une autre page comme référence. | Retirer cette adresse du sitemap. | nous |
| `imageMissingAlt` | Image sans texte alternatif | L'image est invisible pour Google Images et pour les lecteurs d'écran. | Ajouter un attribut alt qui décrit l'image. | SiteOne |
| `imageTooHeavy` | Image lourde (plus de 200 Ko) | L'image ralentit le chargement de la page. | Compresser l'image ou la convertir en WebP ou AVIF. | nous |
| `brokenExternalLink` | Lien externe cassé | Une page du site pointe vers une adresse d'un autre site qui n'existe plus. Le visiteur qui clique tombe sur une erreur. | Corriger le lien ou le retirer. | nous |

## Basse

| Identifiant | Titre | Pourquoi | Correction | Source |
|---|---|---|---|---|
| `titleTooLong` | Title trop long (plus de 60 caractères) | Google le coupe dans les résultats. | Raccourcir en gardant les mots importants au début. | nous (à ajouter) |
| `titleTooShort` | Title trop court (moins de 30 caractères) | Le titre décrit mal la page. | Le compléter : sujet, lieu, marque. | nous (à ajouter) |
| `metaDescriptionTooLong` | Meta description trop longue (plus de 160 caractères) | Google la coupe dans les résultats. | La raccourcir. | nous (à ajouter) |
| `metaDescriptionTooShort` | Meta description trop courte (moins de 70 caractères) | L'extrait donne peu envie de cliquer. | La compléter. | nous (à ajouter) |
| `metaDescriptionDuplicate` | Meta description en double | Plusieurs pages ont le même extrait. | Écrire une description propre à chaque page. | SiteOne |
| `h1Multiple` | Plusieurs H1 | Le sujet principal de la page est flou. | Garder un seul H1 et passer les autres en H2. | nous (à ajouter) |
| `headingStructure` | Niveaux de titres sautés | La page passe par exemple d'un H1 à un H3. Sa structure est moins claire. | Respecter l'ordre H1, H2, H3. | SiteOne |
| `jsonLdMissing` | Aucune donnée structurée | La page ne peut pas obtenir de résultat enrichi (FAQ, avis, entreprise locale). | Ajouter le JSON-LD adapté au type de page. | nous |
| `htmlLangMissing` | Langue de la page non déclarée | Google et les lecteurs d'écran devinent la langue. | Ajouter `lang="fr"` sur la balise html. | SiteOne |
| `securityHeadersMissing` | En-têtes de sécurité manquants | Le site est moins protégé contre certaines attaques (HSTS, CSP, X-Frame-Options…). | Ajouter les en-têtes manquants dans la configuration du serveur. | nous |
| `phoneNotClickable` | Numéro de téléphone non cliquable | Sur mobile, le visiteur ne peut pas appeler en un geste. | Mettre le numéro dans un lien `tel:`. | SiteOne |
| `modernCompressionMissing` | Compression Brotli absente | Les pages pèsent plus lourd que nécessaire. | Activer Brotli sur le serveur. | SiteOne |

## Information

| Identifiant | Titre | Ce que ça veut dire | Source |
|---|---|---|---|
| `canonicalToOtherUrl` | Canonical vers une autre page | La page désigne une autre page du site comme référence. C'est normal pour une variante, à vérifier sinon. | nous |
| `noindex` | Page en noindex | La page demande à ne pas apparaître dans Google. C'est normal pour les mentions légales ou un panier, à vérifier sinon. | nous |
| `notInSitemap` | Page absente du sitemap | La page est indexable mais le sitemap ne l'annonce pas. | nous |
| `thinContent` | Contenu court (moins de 200 mots) | Peu de texte. C'est normal pour une page contact, à enrichir pour une page de service. | nous |

## Écartés du catalogue

Ces analyses de SiteOne restent visibles dans son rapport technique, mais ne deviennent pas des problèmes Arpenteur. Elles visent la qualité du code plus que le référencement, ou produiraient trop de bruit pour un site de TPE :
- SVG intégrés trop gros, dupliqués ou invalides ;
- profondeur du DOM ;
- guillemets manquants dans les attributs ;
- formulaires sans libellé, liens et boutons sans nom, absence de zone `main`, structure HTML ;
- WebP et AVIF : couverts par `imageTooHeavy` ;
- cache, DNS, en-têtes et types de contenu : données utiles dans le détail d'une URL, pas des problèmes.

## Points à trancher

1. **Le canonical absent en moyenne**, et non en basse. C'est le défaut qui a motivé l'outil sur crocweb.fr.
2. **Le noindex et le contenu court en information**, pour éviter les faux positifs sur les mentions légales et les pages contact.
3. **L'accessibilité presque entièrement écartée**, sauf le texte alternatif des images et la langue de la page. C'est un choix d'outil SEO. Un audit d'accessibilité serait un autre outil.
4. **Le téléphone non cliquable en basse.** Pour une entreprise locale, il pourrait mériter la moyenne.
