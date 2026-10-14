# Interface

Les composants sont décrits dans `components/bundle.css`, avec des classes préfixées `ac-`. Chaque composant a une carte d'aperçu et une notice. Le principe d'ensemble : **expressif dans les signes (route, cyan, bornes), sobre dans la structure**. Les informations et les actions restent toujours faciles à trouver.

## Composants

| Composant | Classes | Rôle |
|---|---|---|
| Bouton | `ac-btn` + `--principal` / `--secondaire` / `--discret` / `--danger`, `--sm`, `--bloc` | Déclencher une action |
| Carte | `ac-carte`, `ac-carte--action` | Résumer un objet et mener à sa fiche |
| Champ | `ac-champ`, `ac-label`, `ac-input`, `ac-aide`, `ac-erreur` | Saisir une information |
| Badge | `ac-badge--succes` / `--attention` / `--danger` / `--info` / `--neutre` | Afficher un statut |
| Alerte | `ac-alerte--attention` / `--danger` / `--succes` / `--info` | Signaler et proposer l'action |
| Tableau | `ac-tableau-cadre`, `ac-tableau`, `ac-num` | Comparer des objets (Pro) |
| Borne | `ac-borne--*` | Compte à rebours vers l'échéance, en forme de borne routière (cartes mobiles, fiche véhicule) |
| Barre de trajet | `ac-trajet--*` | Temps restant, d'aujourd'hui à l'échéance (tableaux, fiches) |
| Fiche à onglet daté | `ac-doc`, `ac-doc--attention` / `--danger` | Document et sa date d'expiration |
| Navigation | `ac-nav` (Pro, sombre), `ac-tabbar` (mobile) | Se repérer |

## États

| État | Rendu | Déclencheur |
|---|---|---|
| Normal | Couleurs de base | — |
| Survol | Bouton principal `cyan-300` ; secondaire `marine-50` ; carte bordure `ardoise-500` + `ombre-1` ; ligne de tableau `fond` ; champ bordure `encre` | `:hover` (souris uniquement) |
| Focus | Anneau 2 px `focus`, décalé de 2 px ; `cyan-400` sur fond sombre ; champ : bordure `marine-700` + anneau | `:focus-visible` |
| Pressé / actif | Principal `cyan-500` ; secondaire et discret `marine-100` ; danger plein `danger` + texte blanc | `:active`, `aria-pressed` |
| Sélection | Bordure 2 px `marine-700` + fond `marine-50` (carte) ou `marine-100` (ligne) **et** une coche ou une case cochée | `aria-selected`, `aria-current` |
| Erreur | Bordure 2 px `danger` + message avec icône qui dit comment corriger | `aria-invalid="true"`, `aria-describedby` |
| Désactivé | Fond `desactive`, texte `ardoise-500`, curseur interdit, raison expliquée à côté | `disabled`, `aria-disabled` |

Les classes `is-hover`, `is-focus`, `is-active`, `is-selected`, `is-disabled` ne servent qu'aux démonstrations et maquettes. En production, ce sont les pseudo-classes et attributs ARIA qui pilotent les états.

## Lisibilité et navigation

**Téléphone (Autoclair)**
- Largeur de référence : 360 px ; gouttière 16 px ; cibles tactiles d'au moins 44 × 44 px.
- Barre d'onglets en bas : 4 entrées (Véhicules, Échéances, Documents, Profil), icône et libellé toujours visibles.
- Un titre `titre-1` et une action principale par écran, placée en bas, à portée du pouce.
- Listes de cartes plutôt que tableaux ; borne sur chaque véhicule ; regroupement des alertes en un bandeau.
- Formulaires en étapes courtes, clavier adapté au champ (date, numérique), validation à la sortie du champ.

**Ordinateur (Autoclair Pro)**
- Barre latérale `marine-900` de 260 px, logo Pro négatif en tête, repliable en icônes sous 1 024 px.
- Ordre de lecture : indicateurs de risque → tableau trié par urgence → détails.
- Tableaux avec en-têtes figés, tri, filtres et recherche ; barre de trajet et badge dans chaque ligne d'échéance.
- Actions groupées sur sélection multiple ; confirmation explicite pour toute suppression.

## Accessibilité

Visée : niveau AA des **WCAG 2.1**, base du **RGAA 4** (référentiel français), choisi par qualité et non par obligation constatée. Depuis le 28/06/2025, les exigences européennes d'accessibilité s'appliquent à des services précis : commerce en ligne, banque aux particuliers, communications électroniques, médias audiovisuels, certains transports. Le RGAA, comme obligation légale, vise les organismes publics et les entreprises dont le chiffre d'affaires dépasse 250 M€ (source : Blog du Modérateur, 22/01/2025). Une application de suivi de véhicules ne figure pas explicitement dans ces catégories. L'obligation pour Autoclair est donc **à confirmer avec un juriste**, en particulier si l'application vend en ligne ou vise des clients publics, qui exigent souvent la conformité RGAA dans leurs marchés.

Règles concrètes :
- contrastes de la section Couleurs ;
- statut jamais porté par la couleur seule ;
- focus toujours visible ;
- ordre de tabulation logique ;
- libellés reliés aux champs ;
- `role="alert"` pour les erreurs bloquantes et les échéances dépassées ;
- textes alternatifs sur les images ;
- interface utilisable avec un zoom de 200 % et les animations réduites (`prefers-reduced-motion`).

## Mouvement

Transitions de 120 à 200 ms sur les couleurs et les ombres. La barre de trajet peut se remplir à l'ouverture (300 ms), sauf avec `prefers-reduced-motion`. Pas d'animation en boucle.

## Mode sombre

Il n'est pas défini dans cette version. Seul le thème clair est spécifié. Un thème sombre devra redéfinir chaque couple texte/fond et revérifier tous les contrastes avant usage.
