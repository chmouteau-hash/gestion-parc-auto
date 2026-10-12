# Jetons de la charte — du rôle à la variable CSS

> **État au 12/10/2026**, d'après `static/style.css` (235 lignes, 15 variables). Les numéros de
> ligne valent pour cette version ; les commandes du § 5 disent si elle a bougé. Ce fichier
> **recopie** la feuille de style : en cas d'écart, c'est elle qui a raison. Les intentions et
> les règles d'usage sont dans [CHARTE.md](CHARTE.md).

Un **jeton** (« token ») est une valeur de la charte qui porte un nom de rôle. Une **variable
CSS** est la façon de l'écrire dans la feuille de style : on la définit une fois
(`--accent: #1f5f99;`) et on l'utilise avec `var(--accent)`. Un **sélecteur** désigne les
éléments de la page auxquels une règle s'applique (`.bouton` : tout élément de classe `bouton`).

## 1. Couleurs : les 15 variables du bloc `:root`

Le bloc `:root` (lignes 4 à 20) définit les variables valables pour toute la page. Aucune
couleur n'est écrite ailleurs dans la feuille de style.

| Rôle | Variable CSS | Valeur | Sélecteur principal | Autres usages |
| --- | --- | --- | --- | --- |
| Fond de page | `--fond` | `#f5f6f8` (l. 5) | `body` · fond (l. 30) | `.menu a:hover` l. 57 ; `.tableau th` l. 79 ; `.pastille` l. 93 ; `.bouton-discret:hover` l. 168 |
| Surface | `--papier` | `#ffffff` (l. 6) | `.tableau` · fond (l. 73) | `.entete` l. 46 ; `.formulaire` l. 104 ; champs l. 118 ; `.tableau tr` sous 700 px l. 203 |
| Texte principal | `--encre` | `#1f2933` (l. 7) | `body` · texte (l. 29) | `.marque` l. 49 |
| Texte secondaire | `--encre-douce` | `#52606d` (l. 8) | `.sous-titre`, `.vide`, `.aide` · texte (l. 66-68) | `h1 small` l. 37 ; `.menu a` l. 55 ; `.session` l. 59 ; `.tableau th` l. 79 ; pastilles grises l. 98 ; `.fiche dt` l. 101 ; `.formulaire label small` l. 106 ; `.tableau td[data-label]::before` l. 218 |
| Trait | `--trait` | `#d9dee3` (l. 9) | `.tableau th, .tableau td` · bord bas (l. 78) | `.entete` l. 47 ; `.tableau` l. 74 ; `.pastille` l. 94 ; **fond** des pastilles grises l. 98 ; `.formulaire` l. 104 ; `.bouton-discret` l. 167 ; `.tableau tr` sous 700 px l. 204 ; cellule des actions sous 700 px l. 225 |
| Trait marqué | `--trait-fort` | `#818d9a` (l. 10) | `.formulaire input, .formulaire select, .formulaire textarea` · bord (l. 116) | — |
| Accent | `--accent` | `#1f5f99` (l. 11) | `.bouton` · fond (l. 160) | `a` l. 33 ; `.menu a.actif` l. 58 ; bord de `.bouton, .bouton-discret` l. 159 ; texte de `.bouton-discret` l. 167 |
| Accent appuyé | `--accent-fonce` | `#164a78` (l. 12) | `.bouton:hover` · fond (l. 166) | `a:hover` l. 34 ; `.depot-boutons label:focus-within` l. 136 |
| Texte sur accent | `--sur-accent` | `#ffffff` (l. 13) | `.bouton` · texte (l. 161) | `.menu a.actif` l. 58 ; `.bouton:hover` l. 166 |
| Danger | `--danger` | `#b42318` (l. 14) | `.message-erreur` · texte (l. 177) | `.bouton-discret.danger` l. 169 |
| Fond de danger | `--danger-fond` | `#fde8e6` (l. 15) | `.message-erreur` · fond (l. 176) | `.tableau .alerte-depassee` l. 85 ; `.bouton-discret.danger:hover` l. 170 |
| Trait de danger | `--danger-trait` | `#f3b4ae` (l. 16) | `.message-erreur` · bord (l. 178) | — |
| Fond d'avertissement | `--alerte-fond` | `#fff4e0` (l. 17) | `.tableau .alerte-a-venir` · fond (l. 86) | `.statut-en_maintenance, .etat-a_venir` l. 97 |
| Fond de réussite | `--ok-fond` | `#e6f4ea` (l. 18) | `.message-succes` · fond (l. 183) | `.tableau .ligne-active` l. 84 ; `.statut-en_service, .statut-actif, .etat-en_cours` l. 96 |
| Trait de réussite | `--ok-trait` | `#b7dfc2` (l. 19) | `.message-succes` · bord (l. 184) | — |

Total : 56 usages (`var(--…)`) pour 15 variables. Aucune variable n'est définie sans être
utilisée, aucune n'est utilisée sans être définie.

La ligne 161 vaut pour `.bouton` et `.bouton-discret`, écrits ensemble ; pour le bouton discret,
la ligne 167 remplace aussitôt ce texte blanc par `--accent`.

## 2. Pourquoi ces quatre variables-là

Quatre variables ont été ajoutées le 12/10/2026. Trois donnent un nom à des couleurs jusque-là
écrites en dur, sans changer le rendu ; la quatrième corrige un contraste.

- **`--sur-accent`, et non `--papier`.** Les deux valent `#ffffff`, mais ne jouent pas le même
  rôle : l'une est un texte posé sur la couleur d'accent, l'autre une surface. Les garder à
  part évite que le texte des boutons change le jour où les surfaces changent de teinte (un
  blanc cassé, un thème sombre).
- **`--danger-trait` et `--ok-trait`.** Aucune variable existante n'avait ces valeurs. Les noms
  suivent le modèle déjà en place : `--danger-fond`, `--ok-fond`.
- **`--trait-fort`.** Un champ a le même fond que son formulaire : son bord est son seul
  repère, et `--trait` n'y tenait que 1,35:1 de contraste. `--trait-fort` tient 3,38:1 sur
  `--papier` et 3,12:1 sur `--fond` : il peut servir sur l'un comme sur l'autre.

Le fond d'un champ, lui, n'a pas eu besoin de variable nouvelle : il reprend `--papier`, comme
le formulaire qui le contient (l. 104 et 118).

## 3. Typographie (sans variable)

| Rôle | Valeur | Sélecteur (ligne) |
| --- | --- | --- |
| Famille de police | `system-ui, -apple-system, "Segoe UI", Roboto, sans-serif` | `body` (l. 26) |
| Taille du texte courant | `15px` | `body` (l. 27) |
| Interligne | `1.45` | `body` (l. 28) |
| Titre de page | `1.6rem` | `h1` (l. 36) |
| Précision dans un titre | `1rem` | `h1 small` (l. 37) |
| Titre de section | `1.15rem` | `h2` (l. 38) |
| Petit texte | `0.9rem` | `.aide` (l. 68), `.tableau th` (l. 79), `.bouton-discret` (l. 167), `.tableau td[data-label]::before` (l. 219) |
| Texte d'une pastille | `0.85rem` | `.pastille` (l. 92) |
| Texte d'un bouton de dépôt | `1rem` | `.depot-boutons label` (l. 132) |
| Champ sous 700 px de large | `16px` | `.formulaire input, .formulaire select, .formulaire textarea` (l. 228) |
| Graisse forte | `700` | `.marque` (l. 49) |
| Graisse moyenne | `600` | `.tableau th` (l. 79), `.formulaire label` (l. 105), première cellule sous 700 px (l. 208) |

## 4. Formes et dimensions (sans variable)

| Rôle | Valeur | Sélecteur (ligne) |
| --- | --- | --- |
| Arrondi d'un élément d'action ou de saisie | `6px` | `.menu a` (l. 53), champs (l. 117), `.bouton, .bouton-discret` (l. 158), `.message-erreur` (l. 175), `.message-succes` (l. 182) |
| Arrondi d'une surface | `8px` | `.tableau` (l. 75), `.formulaire` (l. 104), `.tableau tr` sous 700 px (l. 205) |
| Arrondi d'une pastille | `999px` | `.pastille` (l. 91) |
| Contour de l'élément actif au clavier | `2px solid`, décalage `2px` | `.depot-boutons label:focus-within` (l. 136) |
| Largeur maximale du contenu | `1100px` | `.contenu` (l. 61) |
| Largeur d'une page sans menu | `420px` | `.contenu-etroit` (l. 62) |
| Largeur maximale d'un formulaire | `560px` | `.formulaire` (l. 104) |
| Seuil « écran étroit » | `700px` | `@media (max-width: 700px)` (l. 187) |
| Appareil à souris | `(hover: hover) and (pointer: fine)` | `@media` (l. 151) : masque « Prendre une photo » |
| Lien d'évitement, hors de l'écran puis visible au clavier | `top: -4rem`, puis `0.5rem` | `.lien-evitement` (l. 234-235) |

## 5. Vérifier que ce fichier est à jour

Deux commandes, à lancer à la racine du dépôt :

```bash
grep -n -E "#[0-9a-fA-F]{3,6}" static/style.css   # toutes les couleurs : 15 lignes attendues
grep -o "var(--" static/style.css | wc -l         # usages d'une variable : 56 attendus
```

Quinze lignes de couleur, toutes entre les lignes 5 et 19 : ce sont les définitions du § 1. Une
couleur trouvée plus bas est une couleur écrite en dur, à faire entrer dans une variable.
Cinquante-six usages : le total du § 1. Si l'un des deux comptes change, ce fichier est à relire.

La feuille de style est intégrée à l'exécutable au moment de la compilation
(`src/web/mod.rs:37`) : une modification n'apparaît dans l'application qu'après une
recompilation. Le catalogue d'interface, lui, la montre aussitôt.
