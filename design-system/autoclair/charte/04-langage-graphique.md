# Langage graphique

## Le motif : la route

Le **A-route** du logo est le seul motif décoratif de la marque. Il s'utilise en grand, **rogné** par un bord, sur les couvertures, diapositives, bannières, publications et cartes de visite (fichiers `autoclair-a-couleur.svg` et `autoclair-a-negatif.svg`).

Associations permises :
- jambes `marine-700` sur fond `marine-900` ;
- jambes `marine-900` sur fond `marine-700` ;
- jambes `marine-100` sur fond `blanc` ;
- jambes `marine-700` sur fond `blanc` (documents).

Le sommet reste toujours `cyan-400`.

Le motif n'est jamais posé sous un texte de petite taille, ni répété en semis, ni déformé, ni incliné.

Dans l'interface, la route se décline en trois signes fonctionnels :
- **la barre de trajet** : d'aujourd'hui (cercle ouvert) à l'échéance (point plein) ;
- **la borne** : compte à rebours en forme de borne routière, dont la tête prend la couleur du statut ;
- **la fiche à onglet daté** : chaque document porte sa date d'expiration.

## Icônes

- Dessin **au trait**, épaisseur 2 px, extrémités et angles arrondis, sur une grille de 24 px (zone utile 20 px).
- Couleur héritée du texte (`currentColor`) ; tailles 16, 20 (par défaut) et 24 px. Les icônes ne sont jamais cyan sur fond clair.
- Une icône de navigation ou d'action est toujours accompagnée d'un libellé, sauf pour fermer, revenir ou rechercher, qui reçoivent alors un `aria-label`.
- Jeu de base, à étendre dans le même style : véhicule, calendrier, document, conducteur, conducteurs, affectation (flèches opposées), intervention (clé), alerte (cloche), coche, horloge, triangle, information, recherche, plus, chevrons.
- Bibliothèque compatible pour compléter : **Lucide** (licence ISC), au trait de 2 px. Vérifier sa licence et sa version au moment de l'ajout.
- Pas de voiture de sport, de volant, de drapeau à damier ni de pièces mécaniques détaillées : la route suffit à dire l'automobile.

## Illustrations

- Les illustrations sont des **mises en scène de l'interface** : cartes, bornes, barres de trajet et fiches posées sur `fond`, `cyan-100` ou `marine-700`.
- Pas de personnages dessinés, ni d'objets 3D, ni de dégradés.
- Quand un écran est vide, afficher le A-route en `marine-100`, une phrase utile et l'action principale (« Ajoutez votre premier véhicule »).

## Photographie

- Usage limité aux pages de présentation, aux réseaux sociaux et aux documents. Jamais dans l'interface.
- Scènes du quotidien, lumière naturelle : une route dégagée, un parking de quartier, une remise de clés, un carnet d'entretien sur une table, un gestionnaire devant son écran. Cadrages calmes, avec de l'espace libre pour le texte.
- À éviter : concession, salon automobile, circuit, tuning, garage mécanique, voitures de luxe ou de sport mises en scène.
- Personnes réelles uniquement avec une **autorisation de droit à l'image** écrite ; banques d'images avec une licence commerciale vérifiée. Aucune plaque d'immatriculation lisible : la flouter ou la remplacer par une plaque fictive.
- Traitement : légère désaturation ; un voile `marine-900` à 40-60 % pour poser un texte blanc.

## Formes, espacements, bordures, angles, ombres

| Sujet | Règle | Tokens |
|---|---|---|
| Grille | Multiples de 4 px | `space-1` (4) à `space-12` (48) |
| Gouttières | Mobile 16 px ; ordinateur 24 px ; page produit 48 px | `space-4`, `space-6`, `space-12` |
| Rembourrage | Cartes 16 px ; cellules de tableau 12 × 16 px ; boutons 0 × 20 px | `space-3`, `space-4`, `space-5` |
| Bordures | 1 px `ligne` pour les cartes et séparateurs ; 1,5 px `ardoise-500` pour les champs ; 2 px pour la sélection et l'erreur | — |
| Angles | 8 px pour les champs et alertes ; 12 px pour les cartes et boutons ; 16 px pour les modales ; pastille pour les badges et le libellé Pro ; 4 px pour les onglets | `radius-sm`, `radius-md`, `radius-lg`, `radius-pill`, `radius-xs` |
| Ombres | Aucune au repos ; survol de carte cliquable ; menus ; modales | `ombre-1`, `ombre-2`, `ombre-3` |
| Séparation | Préférer l'espace, puis la bordure, puis l'ombre | — |

À éviter : dégradés, verre dépoli, ombres portées décoratives, bandes de couleur sur le bord gauche des cartes, plus de deux niveaux d'imbrication de cartes.
