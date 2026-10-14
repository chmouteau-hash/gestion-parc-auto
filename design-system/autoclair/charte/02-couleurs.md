# Couleurs

La palette vient du logo : le **marine** des jambes du A et le **cyan** de son sommet. Les couleurs sont définies dans `tokens.json` et exposées en variables CSS (`var(--marine-700)`). Les valeurs CMJN sont **indicatives** : elles sont calculées mathématiquement depuis le RGB, sans profil ICC. Elles sont à remplacer par les valeurs de l'imprimeur, sur épreuve, pour le papier choisi (par exemple PSO Coated v3 ou FOGRA39 pour un couché). Le cyan vif #1FC2F6 est hors de la gamme de certains procédés d'impression : son rendu imprimé sera un peu plus terne qu'à l'écran.

## Principales

| Token | Nom | HEX | RGB | CMJN indicatif | Rôle |
|---|---|---|---|---|---|
| `marine-700` | Marine Autoclair | #062966 | 6, 41, 102 | 94 / 60 / 0 / 60 | Logo, marque, liens, sélection, texte du bouton principal |
| `marine-900` | Marine profond | #03183D | 3, 24, 61 | 95 / 61 / 0 / 76 | Icône et barre latérale Pro, fonds sombres |
| `cyan-400` | Cyan Autoclair | #1FC2F6 | 31, 194, 246 | 87 / 21 / 0 / 4 | Sommet du A, fond du bouton principal, libellé Pro, navigation active Pro |

## Secondaires

| Token | Nom | HEX | RGB | CMJN indicatif | Rôle |
|---|---|---|---|---|---|
| `marine-500` | Marine moyen | #2350A0 | 35, 80, 160 | 78 / 50 / 0 / 37 | Survol des liens, graphiques |
| `marine-100` | Marine pâle | #E3EAF6 | 227, 234, 246 | 8 / 5 / 0 / 4 | Sélection, information |
| `marine-50` | Marine très pâle | #F2F5FB | 242, 245, 251 | 4 / 2 / 0 / 2 | Survol secondaire |
| `cyan-300` | Cyan clair | #5ED3F9 | 94, 211, 249 | 62 / 15 / 0 / 2 | Bouton principal au survol |
| `cyan-500` | Cyan soutenu | #0FAEE2 | 15, 174, 226 | 93 / 23 / 0 / 11 | Bouton principal pressé |
| `cyan-700` | Cyan foncé | #0A6E93 | 10, 110, 147 | 93 / 25 / 0 / 42 | Seul cyan permis en texte sur clair, avec parcimonie |
| `cyan-100` | Cyan très pâle | #DDF5FE | 221, 245, 254 | 13 / 4 / 0 / 0 | Fonds d'accent légers |

## Neutres

| Token | Nom | HEX | RGB | CMJN indicatif | Rôle |
|---|---|---|---|---|---|
| `encre` | Encre | #0B1A33 | 11, 26, 51 | 78 / 49 / 0 / 80 | Texte principal |
| `ardoise-600` | Ardoise | #4A5568 | 74, 85, 104 | 29 / 18 / 0 / 59 | Texte secondaire, légendes |
| `ardoise-500` | Ardoise moyen | #697586 | 105, 117, 134 | 22 / 13 / 0 / 47 | Bordure des champs, texte désactivé |
| `ardoise-300` | Ardoise clair | #A9B2BE | 169, 178, 190 | 11 / 6 / 0 / 25 | Décor uniquement |
| `ligne` | Ligne | #D8DFEA | 216, 223, 234 | 8 / 5 / 0 / 8 | Bordures, séparateurs |
| `fond` | Fond | #F5F7FB | 245, 247, 251 | 2 / 2 / 0 / 2 | Fond de page |
| `blanc` | Blanc | #FFFFFF | 255, 255, 255 | 0 / 0 / 0 / 0 | Cartes, champs |
| `desactive` | Désactivé | #EDF0F5 | 237, 240, 245 | 3 / 2 / 0 / 4 | Fond désactivé |

## Fonctionnelles

| Token | Nom | HEX | RGB | CMJN indicatif | Rôle |
|---|---|---|---|---|---|
| `succes` | Succès | #1F6F45 | 31, 111, 69 | 72 / 0 / 38 / 56 | À jour, enregistré |
| `succes-fond` | Fond succès | #E7F4EC | 231, 244, 236 | 5 / 0 / 3 / 4 |  |
| `attention` | Attention | #8A4B00 | 138, 75, 0 | 0 / 46 / 100 / 46 | Échéance sous 30 jours |
| `attention-fond` | Fond attention | #FFF4DC | 255, 244, 220 | 0 / 4 / 14 / 0 |  |
| `danger` | Danger | #B42318 | 180, 35, 24 | 0 / 81 / 87 / 29 | Dépassé, erreur, suppression |
| `danger-fond` | Fond danger | #FDECEA | 253, 236, 234 | 0 / 7 / 8 / 1 |  |
| `focus` | Focus | #1F5FCC | 31, 95, 204 | 85 / 53 / 0 / 20 | Anneau de focus clavier sur fond clair |

## Proportions

Sur un écran type : environ 70 % de neutres clairs (`fond`, `blanc`), 20 % de marine (texte de marque, navigation, fonds Pro), moins de 5 % de cyan (une action, un sommet), et les couleurs de statut seulement là où un statut existe.

## Associations autorisées

Rapports de contraste calculés selon WCAG 2.x. Seuils : 4,5:1 pour le texte, 3:1 pour le texte de 24 px et plus, les icônes et les bordures de champs.

| Texte ou élément | Fond | Contraste | Usage |
|---|---|---|---|
| `encre` | `blanc` / `fond` | 17,4 / 16,2 | Texte courant |
| `ardoise-600` | `blanc` / `fond` | 7,5 / 7,0 | Texte secondaire |
| `marine-700` | `blanc` / `fond` / `marine-100` | 13,9 / 12,9 / 11,5 | Liens, titres de marque, sélection |
| `marine-500` | `blanc` | 7,7 | Lien au survol |
| `blanc` | `marine-700` / `marine-900` | 13,9 / 17,5 | Texte sur fond sombre |
| `ligne` | `marine-900` | 13,0 | Navigation Pro inactive |
| `marine-700` | `cyan-400` / `cyan-300` / `cyan-500` | 6,7 / 8,0 / 5,4 | Bouton principal (normal, survol, pressé), libellé Pro |
| `cyan-400` | `marine-900` / `marine-700` | 8,4 / 6,7 | Sommet du A, élément actif, focus sur fond sombre |
| `cyan-700` | `blanc` | 5,7 | Rare texte ou icône cyan sur fond clair |
| `marine-700` | `cyan-100` | 12,2 | Encarts d'accent |
| `succes` | `blanc` / `succes-fond` | 6,1 / 5,4 | Statut à jour |
| `attention` | `blanc` / `attention-fond` | 6,8 / 6,2 | Échéance proche |
| `danger` | `blanc` / `danger-fond` | 6,6 / 5,8 | Dépassé, erreur |
| `focus` | `blanc` / `fond` / `marine-100` | 5,9 / 5,5 / 4,9 | Anneau de focus |
| `ardoise-500` (bordure) | `blanc` / `fond` | 4,7 / 4,4 | Bordure des champs (seuil 3:1) |

## Associations interdites

- `cyan-400` en texte ou icône sur fond clair (2,1:1) : le cyan est une surface. Pour un texte cyan, utiliser `cyan-700`.
- Texte blanc sur `cyan-400` (2,1:1) : toujours `marine-700` sur le cyan.
- `ardoise-300` pour du texte ou une bordure de champ (2,1:1).
- `marine-700` sur `marine-900` pour un texte : utiliser `blanc` ou `ligne`.
- Couleurs de statut employées pour décorer : `succes`, `attention` et `danger` ne servent qu'à des statuts.

## Statuts : une signification stable

| Statut | Couleur | Icône | Libellé type | Autoclair | Autoclair Pro |
|---|---|---|---|---|---|
| À jour | `succes` | coche | « À jour », « Planifiée » | identique | identique |
| Bientôt | `attention` | horloge | « Dans 15 jours » | identique | identique |
| Dépassé | `danger` | triangle | « Dépassée de 3 j » | identique | identique |
| Information | `marine-700` sur `marine-100` | i | « Bon à savoir » | identique | identique |

Une information importante n'est **jamais** transmise par la couleur seule : chaque statut a une icône et un texte, et la tête colorée de la borne est toujours accompagnée d'un badge. La règle tient aussi pour les personnes daltoniennes et à l'impression en noir et blanc. Le seuil « Bientôt » vaut 30 jours dans les deux applications.

Le `cyan-400` n'est **pas** une couleur de statut : il désigne l'action et la marque. Il ne sert jamais à dire « à jour » ou « information ».
