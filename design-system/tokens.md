# Jetons de la charte — du rôle à la variable CSS

> **État au 13/10/2026**, d'après `static/style.css` (286 lignes, 20 variables). Les numéros de
> ligne valent pour cette version ; les commandes du § 5 disent si elle a bougé. Ce fichier
> **recopie** la feuille de style : en cas d'écart, c'est elle qui a raison. Les intentions et
> les règles d'usage sont dans [CHARTE.md](CHARTE.md).

Un **jeton** (« token ») est une valeur de la charte qui porte un nom de rôle. Une **variable
CSS** est la façon de l'écrire dans la feuille de style : on la définit une fois
(`--accent: #1f5f99;`) et on l'utilise avec `var(--accent)`. Un **sélecteur** désigne les
éléments de la page auxquels une règle s'applique (`.bouton` : tout élément de classe `bouton`).

## 1. Couleurs : les 20 variables du bloc `:root`

Le bloc `:root` (lignes 6 à 32) définit les variables valables pour toute la page. Aucune
couleur n'est écrite ailleurs dans la feuille de style.

| Rôle | Variable CSS | Valeur | Sélecteur principal | Autres usages |
| --- | --- | --- | --- | --- |
| Fond de page | `--fond` | `#f5f6f8` (l. 8) | `body` · fond (l. 42) | `.menu a:hover` l. 69 ; `.tableau th` l. 91 ; `.bouton-discret:hover` l. 200 |
| Surface | `--papier` | `#ffffff` (l. 9) | `.tableau` · fond (l. 85) | `.entete` l. 58 ; `.formulaire` l. 137 ; champs l. 151 ; `.tableau tr` sous 700 px l. 243 |
| Texte principal | `--encre` | `#1f2933` (l. 10) | `body` · texte (l. 41) | `.marque` l. 61 |
| Texte secondaire | `--encre-douce` | `#52606d` (l. 11) | `.sous-titre`, `.vide`, `.aide` · texte (l. 78-80) | `h1 small` l. 49 ; `.menu a` l. 67 ; `.session` l. 71 ; `.tableau th` l. 91 ; `.pastille` l. 107 ; pastilles neutres l. 130 ; `.fiche dt` l. 134 ; `.formulaire label small` l. 139 ; `.tableau td[data-label]::before` l. 258 |
| Trait | `--trait` | `#d9dee3` (l. 12) | `.tableau th, .tableau td` · bord bas (l. 90) | `.entete` l. 59 ; `.tableau` l. 86 ; `.formulaire` l. 137 ; `.tableau tr` sous 700 px l. 244 ; cellule des actions sous 700 px l. 265 |
| Trait marqué | `--trait-fort` | `#818d9a` (l. 13) | `.formulaire input, .formulaire select, .formulaire textarea` · bord (l. 149) | — |
| Accent | `--accent` | `#1f5f99` (l. 16) | `.bouton` · fond (l. 192) | `a` l. 45 ; `.menu a.actif` l. 70 ; bord de `.bouton, .bouton-discret` l. 191 ; texte de `.bouton-discret` l. 199 |
| Accent appuyé | `--accent-fonce` | `#164a78` (l. 17) | `.bouton:hover` · fond (l. 198) | `a:hover` l. 46 ; contour de l'élément actif au clavier l. 208 |
| Texte sur accent | `--sur-accent` | `#ffffff` (l. 18) | `.bouton` · texte (l. 193) | `.menu a.actif` l. 70 ; `.bouton:hover` l. 198 |
| Texte de réussite | `--ok` | `#1f6e3c` (l. 21) | `.statut-en_service, .statut-actif, .etat-en_cours` · texte (l. 115) | `.message-succes` l. 223 |
| Fond de réussite | `--ok-fond` | `#e6f4ea` (l. 22) | `.message-succes` · fond (l. 222) | `.tableau .ligne-active` l. 96 ; pastilles de réussite l. 113 |
| Trait de réussite | `--ok-trait` | `#2f8b53` (l. 23) | `.message-succes` · bord (l. 224) | pastilles de réussite l. 114 |
| Texte d'avertissement | `--alerte` | `#8a5300` (l. 24) | `.statut-en_maintenance, .etat-a_venir, .pastille.alerte-a-venir, .echeance-a_traiter` · texte (l. 120) | — |
| Fond d'avertissement | `--alerte-fond` | `#fff4e0` (l. 25) | `.tableau .alerte-a-venir` · fond (l. 98) | pastilles d'avertissement l. 118 |
| Trait d'avertissement | `--alerte-trait` | `#b37018` (l. 26) | pastilles d'avertissement · bord (l. 119) | — |
| Danger | `--danger` | `#b42318` (l. 27) | `.message-erreur` · texte (l. 216) | `.pastille.alerte-depassee, .echeance-depassee, .permis-expire` l. 125 ; `.bouton-discret.danger` l. 201 |
| Fond de danger | `--danger-fond` | `#fde8e6` (l. 28) | `.message-erreur` · fond (l. 215) | `.tableau .alerte-depassee` l. 97 ; pastilles de danger l. 123 ; `.bouton-discret.danger:hover` l. 202 |
| Trait de danger | `--danger-trait` | `#c94c42` (l. 29) | `.message-erreur` · bord (l. 217) | pastilles de danger l. 124 |
| Fond neutre | `--neutre-fond` | `#e4e8ec` (l. 30) | `.pastille` · fond (l. 105) | `.statut-reforme, .statut-inactif, .etat-terminee, .echeance-traitee` l. 128 |
| Trait neutre | `--neutre-trait` | `#6b7886` (l. 31) | `.pastille` · bord (l. 106) | pastilles neutres l. 129 ; `.bouton-discret` l. 199 |

Total : 66 usages (`var(--…)`) pour 20 variables. Aucune variable n'est définie sans être
utilisée, aucune n'est utilisée sans être définie.

La ligne 193 vaut pour `.bouton` et `.bouton-discret`, écrits ensemble ; pour le bouton discret,
la ligne 199 remplace aussitôt ce texte blanc par `--accent`, et le bord d'accent de la ligne 191
par `--neutre-trait`.

## 2. Pourquoi ces variables-là

### Première série, le 12/10/2026

Quatre variables. Trois donnent un nom à des couleurs jusque-là écrites en dur, sans changer le
rendu ; la quatrième corrige un contraste.

- **`--sur-accent`, et non `--papier`.** Les deux valent `#ffffff`, mais ne jouent pas le même
  rôle : l'une est un texte posé sur la couleur d'accent, l'autre une surface. Les garder à
  part évite que le texte des boutons change le jour où les surfaces changent de teinte (un
  blanc cassé, un thème sombre).
- **`--danger-trait` et `--ok-trait`.** Aucune variable existante n'avait ces valeurs. Les noms
  suivent le modèle déjà en place : `--danger-fond`, `--ok-fond`.
- **`--trait-fort`.** Un champ a le même fond que son formulaire : son bord est son seul
  repère, et `--trait` n'y tenait que 1,35:1 de contraste. `--trait-fort` tient 3,38:1 sur
  `--papier` et 3,12:1 sur `--fond`.

Le fond d'un champ, lui, n'a pas eu besoin de variable nouvelle : il reprend `--papier`, comme
le formulaire qui le contient (l. 137 et 151).

### Deuxième série, le 12/10/2026 : quatre états complets

Cinq variables s'ajoutent, deux changent de valeur. Chaque état a désormais trois jetons : un
texte, un fond, une bordure.

- **`--ok`, `--alerte`.** La réussite et l'avertissement n'avaient pas de couleur de texte : leurs
  pastilles étaient écrites dans la couleur du texte courant. Les noms suivent `--danger`.
- **`--alerte-trait`.** L'avertissement n'avait pas de bordure.
- **`--ok-trait` et `--danger-trait` changent de valeur.** À 1,28:1 et 1,49:1 sur leur fond, ces
  deux traits ne se voyaient pas. Ils tiennent maintenant 3:1 au moins, sur leur fond comme sur
  le blanc. C'est ce qui permet à une pastille de garder le fond de la ligne teintée où elle est
  posée : sa bordure suffit à la détacher.
- **`--neutre-fond`, et non `--trait`.** Les pastilles « terminée / inactif / réformé » prenaient
  `--trait` pour fond : un même nom pour un trait et pour un fond. Le gris de ces pastilles est
  le fond d'un **état**, comme `--ok-fond`, d'où son nom. Un peu plus clair que `--trait`, il
  porte `--encre-douce` à 5,24:1 au lieu de 4,76:1.
- **`--neutre-trait`, et non `--trait-fort`.** Les deux gris sont voisins, mais `--trait-fort`
  tombe à 2,97:1 sur `--ok-fond` et 2,87:1 sur `--danger-fond`. `--neutre-trait` tient 3,66:1 au
  moins sur les six fonds de l'interface : il sert de bord aux pastilles neutres et aux boutons
  discrets, qui sont posés sur plusieurs de ces fonds. `--trait-fort` reste le bord des champs,
  toujours posés sur le blanc d'un formulaire.
- **Pas de `--neutre` pour le texte.** L'état neutre écrit en `--encre-douce` : un jeton de même
  valeur n'aurait servi qu'à la symétrie.

## 3. Typographie (sans variable)

| Rôle | Valeur | Sélecteur (ligne) |
| --- | --- | --- |
| Famille de police | `system-ui, -apple-system, "Segoe UI", Roboto, sans-serif` | `body` (l. 38) |
| Taille du texte courant | `0.9375rem` | `body` (l. 39) |
| Interligne | `1.45` | `body` (l. 40) |
| Titre de page | `1.6rem` | `h1` (l. 48) |
| Précision dans un titre | `0.9375rem` | `h1 small` (l. 49) |
| Titre de section | `1.15rem` | `h2` (l. 50) |
| Petit texte | `0.9rem` | `.aide` (l. 80), `.tableau th` (l. 91), `.bouton-discret` (l. 199), `.tableau td[data-label]::before` (l. 259) |
| Texte d'une pastille | `0.85rem` | `.pastille` (l. 104) |
| Texte d'un bouton de dépôt | `1rem` | `.depot-boutons label` (l. 165) |
| Champ sous 700 px de large | `16px` | `.formulaire input, .formulaire select, .formulaire textarea` (l. 268) |
| Graisse forte | `700` | `.marque` (l. 61) |
| Graisse moyenne | `600` | `.tableau th` (l. 91), `.formulaire label` (l. 138), première cellule sous 700 px (l. 248) |

## 4. Formes et dimensions (sans variable)

| Rôle | Valeur | Sélecteur (ligne) |
| --- | --- | --- |
| Arrondi d'un élément d'action ou de saisie | `6px` | `.menu a` (l. 65), champs (l. 150), `.bouton, .bouton-discret` (l. 190), `.message-erreur` (l. 214), `.message-succes` (l. 221) |
| Arrondi d'une surface | `8px` | `.tableau` (l. 87), `.formulaire` (l. 137), `.tableau tr` sous 700 px (l. 245) |
| Arrondi d'une pastille | `999px` | `.pastille` (l. 103) |
| Contour de l'élément actif au clavier | `2px solid`, décalage `2px` | `a`, `button`, `input`, `select`, `textarea` en `:focus-visible`, et `.depot-boutons label:focus-within` (l. 206-210) |
| Largeur maximale du contenu | `1100px` | `.contenu` (l. 73) |
| Largeur d'une page sans menu | `420px` | `.contenu-etroit` (l. 74) |
| Largeur maximale d'un formulaire | `560px` | `.formulaire` (l. 137) |
| Seuil « écran étroit » | `700px` | `@media (max-width: 700px)` (l. 227) |
| Hauteur d'une action sous 700 px | `44px` au moins : `padding` vertical de `0.7rem`, ou `min-height: 2.75rem` pour un lien | boutons de `.contenu` (l. 273-276) ; liens de la cellule des actions (l. 278) |
| Écart entre deux actions d'une ligne sous 700 px | `0.5rem`, soit 8 px | cellule des actions (l. 277) ; « Supprimer » à l'écart (l. 280) |
| Appareil à souris | `(hover: hover) and (pointer: fine)` | `@media` (l. 183) : masque « Prendre une photo » |
| Lien d'évitement, hors de l'écran puis visible au clavier | `top: -4rem`, puis `0.5rem` | `.lien-evitement` (l. 285-286) |

`:focus-visible` désigne l'élément actif quand on se déplace au clavier : un clic de souris sur
un bouton ne dessine pas ce contour.

## 5. Vérifier que ce fichier est à jour

Deux commandes, à lancer à la racine du dépôt :

```bash
grep -n -E "#[0-9a-fA-F]{3,6}" static/style.css   # toutes les couleurs : 20 lignes attendues
grep -o "var(--" static/style.css | wc -l         # usages d'une variable : 66 attendus
```

Vingt lignes de couleur, toutes entre les lignes 8 et 31 : ce sont les définitions du § 1. Une
couleur trouvée plus bas est une couleur écrite en dur, à faire entrer dans une variable.
Soixante-six usages : le total du § 1. Si l'un des deux comptes change, ce fichier est à relire.

La feuille de style est intégrée à l'exécutable au moment de la compilation
(`src/web/mod.rs:37`) : une modification n'apparaît dans l'application qu'après une
recompilation. Le catalogue d'interface, lui, la montre aussitôt.
