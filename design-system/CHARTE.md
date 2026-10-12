# Charte graphique — gestion-parc-auto

> **État au 12/10/2026**, d'après `static/style.css` (270 lignes, 20 variables). Dix décisions
> du fondateur y sont inscrites (§ 9). Ce qui est décidé sans être encore dans le code est
> marqué « à venir ».

## 1. À quoi sert ce document

Il dit, en clair, à quoi sert chaque couleur, chaque taille de texte et chaque forme de
l'interface, pour qu'une nouvelle page ressemble aux anciennes sans avoir à deviner.

La **feuille de style** (`static/style.css`, le fichier qui décrit l'apparence des pages) reste
la source de vérité : c'est elle que voit l'utilisateur. Si ce document et la feuille de style
se contredisent, c'est ce document qui est à corriger — ou la feuille de style à modifier, par
décision.

Six mots reviennent :

- **variable CSS** : une valeur à laquelle la feuille de style donne un nom (`--accent`), pour
  l'écrire une seule fois et la réutiliser partout avec `var(--accent)` ;
- **jeton** (« token ») : une valeur de la charte qui porte un nom de rôle. Ici, les jetons de
  couleur sont les variables CSS ; leur liste est dans [tokens.md](tokens.md) ;
- **gabarit** : un modèle de page HTML à trous, que le serveur remplit avec les données
  (dossier `templates/`) ;
- **contraste** : l'écart de luminosité entre un texte et son fond, noté comme un rapport
  (`4,5:1`). Plus il est grand, plus le texte se lit facilement ;
- **état** : ce qu'une couleur veut dire. Il y en a quatre : réussite, avertissement, danger,
  neutre (§ 3) ;
- **pastille** : la petite étiquette arrondie qui écrit un état en toutes lettres
  (« En service »).

## 2. Intentions visuelles

| Intention | Ce que cela veut dire | Ce qui le porte dans le code |
| --- | --- | --- |
| **Sérieux** | Un outil de travail, pas une vitrine : pas d'effet, pas d'image décorative, une seule couleur vive. | Une seule couleur d'accent (`--accent`, `style.css:16`). Aucune ombre, aucun dégradé, aucune animation dans la feuille de style. |
| **Lisibilité** | On lit des listes et des dates toute la journée : texte foncé sur fond clair, états dits en toutes lettres, sous la même forme d'un écran à l'autre. | Texte principal à 13,64:1 sur le fond de page (§ 6). Chaque état coloré porte aussi un libellé, dans une pastille (§ 7). |
| **Public** | Des professionnels de la gestion de flotte, au bureau comme sur le terrain, téléphone en main. | Sous 700 px de large, chaque ligne de tableau devient une fiche (`style.css:227-270`) ; champs à 16 px sur téléphone (`style.css:268`). |
| **Sobriété technique** | Rien à télécharger ailleurs : ni police, ni bibliothèque. | Commentaire d'en-tête de la feuille de style (`style.css:1-2`) ; police du système (`style.css:38`). |

Les trois premières intentions sont celles données par le fondateur ; la quatrième est écrite
dans le code.

## 3. Palette par rôle

Vingt couleurs, toutes définies au même endroit (`style.css:6-32`), et **aucune ailleurs**.
Leur nom dit leur **rôle**, pas leur teinte : on écrit `--danger`, pas `--rouge`.

| Rôle | Variable | Valeur | Où on la voit |
| --- | --- | --- | --- |
| Fond de page | `--fond` | `#f5f6f8` | Derrière toute la page ; en-têtes de tableau ; survol du menu et des boutons discrets |
| Surface | `--papier` | `#ffffff` | En-tête, tableaux, formulaires et leurs champs : tout ce qui « pose » un contenu sur le fond |
| Texte principal | `--encre` | `#1f2933` | Titres, texte courant, cellules de tableau |
| Texte secondaire | `--encre-douce` | `#52606d` | Explications, intitulés, états vides, menu, en-têtes de tableau ; texte de l'état neutre |
| Trait | `--trait` | `#d9dee3` | Contour des surfaces et séparateurs. Décoratif : aucune information n'en dépend |
| Trait marqué | `--trait-fort` | `#818d9a` | Bord d'un champ de formulaire |
| Accent | `--accent` | `#1f5f99` | Bouton principal, liens, page en cours dans le menu |
| Accent appuyé | `--accent-fonce` | `#164a78` | Survol d'un lien ou du bouton principal ; contour de l'élément actif au clavier |
| Texte sur accent | `--sur-accent` | `#ffffff` | Texte du bouton principal et de la page en cours du menu |
| Texte de réussite | `--ok` | `#1f6e3c` | Texte des pastilles « en service / actif / en cours » et d'un message de succès |
| Fond de réussite | `--ok-fond` | `#e6f4ea` | Message de succès, ligne active, pastilles « en service / actif / en cours » |
| Trait de réussite | `--ok-trait` | `#2f8b53` | Bord d'un message de succès et de ces pastilles |
| Texte d'avertissement | `--alerte` | `#8a5300` | Texte des pastilles « en maintenance / à venir » |
| Fond d'avertissement | `--alerte-fond` | `#fff4e0` | Ligne d'alerte à venir, pastilles « en maintenance / à venir » |
| Trait d'avertissement | `--alerte-trait` | `#b37018` | Bord de ces pastilles |
| Danger | `--danger` | `#b42318` | Texte d'un message d'erreur, de la pastille « Dépassée » et du bouton « Supprimer » |
| Fond de danger | `--danger-fond` | `#fde8e6` | Message d'erreur, ligne d'alerte dépassée, pastille « Dépassée », bouton « Supprimer » survolé |
| Trait de danger | `--danger-trait` | `#c94c42` | Bord d'un message d'erreur et de la pastille « Dépassée » |
| Fond neutre | `--neutre-fond` | `#e4e8ec` | Pastilles « terminée / inactif / réformé » |
| Trait neutre | `--neutre-trait` | `#6b7886` | Bord de ces pastilles ; bord d'un bouton discret |

Les quatre **états** ont la même structure : un texte, un fond, une bordure. Une nouvelle classe
d'état n'a donc rien à inventer.

| État | Sens | Texte | Fond | Bordure | Classes concernées (`style.css`) |
| --- | --- | --- | --- | --- | --- |
| Réussite | En cours, actif, réussi | `--ok` | `--ok-fond` | `--ok-trait` | `.statut-en_service`, `.statut-actif`, `.etat-en_cours` (l. 112) ; `.message-succes` (l. 219). Fond seul : `.ligne-active` (l. 96) |
| Avertissement | À surveiller, pas encore là | `--alerte` | `--alerte-fond` | `--alerte-trait` | `.statut-en_maintenance`, `.etat-a_venir`, `.pastille.alerte-a-venir` (l. 117). Fond seul : `.alerte-a-venir` (l. 98) |
| Danger | Dépassé, refusé | `--danger` | `--danger-fond` | `--danger-trait` | `.pastille.alerte-depassee` (l. 122) ; `.message-erreur` (l. 212). Fond seul : `.alerte-depassee` (l. 97) |
| Neutre | Terminé, hors service | `--encre-douce` | `--neutre-fond` | `--neutre-trait` | `.statut-reforme`, `.statut-inactif`, `.etat-terminee` (l. 127) ; `.pastille` sans seconde classe (l. 100) |

Trois choses à savoir en lisant ces tableaux :

- les textes d'état (`--ok`, `--alerte`, `--danger`) sont des teintes sombres, pas des couleurs
  vives : la seule couleur vive de l'interface reste `--accent` ;
- l'état neutre n'a pas de jeton de texte à lui : il reprend `--encre-douce`. Un jeton de même
  valeur n'aurait servi qu'à la symétrie ;
- les bordures d'état **se voient** : chacune tient 3:1 au moins sur son propre fond comme sur
  le blanc (§ 6). C'est ce qui détache une pastille de la ligne teintée sur laquelle elle est
  posée, sans lui donner un second fond.

## 4. Typographie

**Police : celle du système.** La feuille de style demande, dans l'ordre, `system-ui`,
`-apple-system`, `"Segoe UI"`, `Roboto`, puis `sans-serif` (`style.css:38`) : chaque appareil
affiche sa police habituelle. Aucun fichier de police n'est téléchargé ni livré avec
l'application — donc aucune licence de police à vérifier, et rien à charger au premier affichage.

| Usage | Taille écrite | Taille à l'écran | Graisse | Ligne |
| --- | --- | --- | --- | --- |
| Titre de page (`h1`) | `1.6rem` | 25,6 px | gras du navigateur | 48 |
| Précision dans le titre (`h1 small`) | `0.9375rem` | 15 px | normale | 49 |
| Titre de section (`h2`) | `1.15rem` | 18,4 px | gras du navigateur | 50 |
| Texte courant (`body`) | `0.9375rem` | 15 px | normale | 39 |
| Bouton de dépôt | `1rem` | 16 px | normale | 165 |
| Texte d'aide, en-tête de tableau, bouton discret, intitulé d'une cellule sur écran étroit | `0.9rem` | 14,4 px | normale (600 pour l'en-tête de tableau) | 80, 91, 199, 259 |
| Pastille | `0.85rem` | 13,6 px | normale | 104 |
| Champ de formulaire | hérite du texte courant | 15 px ; **16 px sous 700 px de large** | normale | 147, 268 |
| Intitulé de champ | hérite | 15 px | 600 | 138 |
| Nom de l'application (`.marque`) | hérite | 15 px | 700 | 61 |

Interligne : `1.45` partout (`style.css:40`). Italique : uniquement pour un état vide (`.vide`,
l. 79).

Deux choses à savoir en lisant ce tableau :

- le `rem` est une unité relative à la taille de texte **de base du navigateur**, 16 px sauf
  réglage contraire de l'utilisateur. Toutes les tailles sont écrites en `rem`, texte courant
  compris : une personne qui agrandit le texte dans son navigateur voit tout grandir ensemble.
  La colonne « à l'écran » vaut pour 16 px de base. Elle a été relevée dans Chromium le
  12/10/2026, dans le catalogue d'interface ; seul l'intitulé d'une cellule sur écran étroit
  est **déduit** ;
- une seule taille reste en `px` : les 16 px d'un champ sous 700 px de large (l. 268). C'est le
  seuil en dessous duquel un téléphone zoome sur le champ, et ce seuil se compte en `px`.

## 5. Formes, traits et largeurs

| Élément | Valeur | Lignes |
| --- | --- | --- |
| Arrondi des éléments de saisie et d'action (menu, champs, boutons, messages) | `6px` | 65, 150, 190, 214, 221 |
| Arrondi des surfaces (tableau, formulaire, fiche d'une ligne sur écran étroit) | `8px` | 87, 137, 245 |
| Arrondi de la pastille | `999px` (bords entièrement ronds) | 103 |
| Épaisseur d'un trait | `1px` | partout |
| Contour de l'élément actif au clavier (lien, bouton, champ, bouton de dépôt) | `2px`, décalé de `2px` | 206-210 |
| Largeur maximale du contenu | `1100px` | 73 |
| Largeur maximale d'un formulaire | `560px` | 137 |
| Largeur d'une page sans menu (connexion, erreur) | `420px` | 74 |
| Seuil « écran étroit » | `700px` | 227 |

Il n'existe pas d'échelle d'espacement nommée : marges et retraits sont écrits au cas par cas,
en `rem`.

## 6. Contrastes

Référence : **WCAG 2.2, niveau AA** — les règles internationales d'accessibilité des sites web.
Elles demandent `4,5:1` pour un texte normal, `3:1` pour un grand texte (24 px et plus, ou
18,66 px et plus en gras) et `3:1` pour les éléments d'interface (bord d'un champ, contour
d'un bouton).

**Méthode de calcul.** Chaque couleur hexadécimale donne trois nombres de 0 à 255 (rouge, vert,
bleu). Pour chacun : `c = valeur / 255`, puis `c / 12,92` si `c ≤ 0,04045`, sinon
`((c + 0,055) / 1,055) ^ 2,4`. La luminance est `L = 0,2126 × R + 0,7152 × V + 0,0722 × B`. Le
rapport est `(L la plus claire + 0,05) / (L la plus sombre + 0,05)`. Les résultats sont
**tronqués** à deux décimales, jamais arrondis vers le haut. Toutes les couleurs de la feuille
de style sont écrites en hexadécimal : aucune conversion depuis un autre format n'a été
nécessaire.

### Textes

| Texte | Fond | Rapport | AA | Où |
| --- | --- | --- | --- | --- |
| `--encre` | `--fond` | 13,64:1 | oui | Texte courant, titres |
| `--encre` | `--papier` | 14,75:1 | oui | Cellules de tableau, intitulés de champ, nom de l'application |
| `--encre` | `--ok-fond` | 12,99:1 | oui | Ligne active |
| `--encre` | `--alerte-fond` | 13,53:1 | oui | Ligne d'alerte à venir |
| `--encre` | `--danger-fond` | 12,55:1 | oui | Ligne d'alerte dépassée |
| `--encre-douce` | `--fond` | 5,97:1 | oui | Sous-titre, aide, état vide, en-tête de tableau, intitulés d'une fiche |
| `--encre-douce` | `--papier` | 6,45:1 | oui | Menu, compte connecté, aide dans un formulaire |
| `--encre-douce` | `--neutre-fond` | **5,24:1** | oui | Pastilles « terminée / inactif / réformé » — le rapport le plus faible de l'interface |
| `--encre-douce` | `--ok-fond` | 5,68:1 | oui | Intitulé d'une cellule sur écran étroit, ligne active |
| `--encre-douce` | `--alerte-fond` | 5,92:1 | oui | Idem, ligne d'alerte à venir |
| `--encre-douce` | `--danger-fond` | 5,49:1 | oui | Idem, ligne d'alerte dépassée |
| `--accent` | `--fond` | 6,15:1 | oui | Liens dans le texte, bouton discret hors d'une surface |
| `--accent` | `--papier` | 6,65:1 | oui | Liens dans un tableau, bouton discret dans un tableau, un formulaire ou l'en-tête |
| `--accent` | `--ok-fond` | 5,86:1 | oui | Liens et bouton discret d'une ligne active, liens d'un message de succès |
| `--accent` | `--alerte-fond` | 6,10:1 | oui | Liens d'une ligne d'alerte à venir |
| `--accent` | `--danger-fond` | 5,66:1 | oui | Liens d'une ligne d'alerte dépassée |
| `--accent-fonce` | les cinq fonds ci-dessus | 7,81:1 à 9,19:1 | oui | Lien survolé |
| `--sur-accent` | `--accent` | 6,65:1 | oui | Bouton principal, page en cours dans le menu |
| `--sur-accent` | `--accent-fonce` | 9,19:1 | oui | Bouton principal survolé |
| `--ok` | `--ok-fond` | 5,51:1 | oui | Pastilles « en service / actif / en cours », message de succès |
| `--alerte` | `--alerte-fond` | 5,80:1 | oui | Pastilles « en maintenance / à venir » |
| `--danger` | `--danger-fond` | 5,59:1 | oui | Message d'erreur, pastille « Dépassée », bouton « Supprimer » survolé |
| `--danger` | `--papier` | 6,57:1 | oui | Bouton « Supprimer » dans un tableau |
| `--danger` | `--fond` | 6,07:1 | oui | Bouton « Supprimer » à côté d'un titre |
| `--danger` | `--ok-fond` | 5,78:1 | oui | Bouton « Supprimer » d'une ligne active |

Tous les textes passent le seuil AA.

### Éléments d'interface

| Élément | Couleurs | Rapport | Seuil 3:1 | Lecture |
| --- | --- | --- | --- | --- |
| Bord d'un champ de formulaire | `--trait-fort` sur `--papier` | 3,38:1 | oui | Le champ et le formulaire ont le même fond blanc : ce trait est le seul repère du champ. |
| Bord d'un bouton discret | `--neutre-trait` sur `--papier`, `--fond` ou `--ok-fond` | 4,51:1 / 4,17:1 / 3,97:1 | oui | Dans un tableau, un formulaire ou l'en-tête ; à côté d'un titre ; sur une ligne active (« Terminer aujourd'hui »). Il était à 1,35:1 avec `--trait` (§ 9, décision 4). |
| Bord d'une pastille de réussite | `--ok-trait` sur `--ok-fond`, `--papier` ou `--fond` | 3,74:1 / 4,25:1 / 3,93:1 | oui | Contre son propre fond, qui est aussi celui d'une ligne active ; dans un tableau blanc ; dans une fiche posée sur la page. |
| Bord d'une pastille d'avertissement | `--alerte-trait` sur `--alerte-fond`, `--papier` ou `--fond` | 3,67:1 / 4,00:1 / 3,70:1 | oui | Mêmes trois cas ; le fond est aussi celui d'une ligne d'alerte à venir. |
| Bord de la pastille « Dépassée » | `--danger-trait` sur `--danger-fond` | 3,88:1 | oui | Elle n'existe que sur une ligne d'alerte dépassée, dont elle a le fond. |
| Bord d'une pastille neutre | `--neutre-trait` sur `--neutre-fond`, `--papier` ou `--fond` | 3,66:1 / 4,51:1 / 4,17:1 | oui | Contre son propre fond ; dans un tableau blanc ; dans une fiche posée sur la page. |
| Bord d'un message d'erreur | `--danger-trait` sur `--danger-fond` ou `--fond` | 3,88:1 / 4,22:1 | oui | Côté message ; côté page. Il était à 1,49:1 (§ 9, décision 6). |
| Bord d'un message de succès | `--ok-trait` sur `--ok-fond` ou `--fond` | 3,74:1 / 3,93:1 | oui | Côté message ; côté page. Il était à 1,28:1. |
| Contour des surfaces | `--trait` sur `--fond` | 1,25:1 | non | Décoratif : aucune information n'en dépend. |
| Contour d'une fiche teintée, sur écran étroit | `--trait` sur `--ok-fond`, `--alerte-fond` ou `--danger-fond` | 1,19:1 / 1,24:1 / 1,15:1 | non | Décoratif : la fiche se reconnaît à son fond et à sa pastille. |
| Bouton principal sur la page | `--accent` sur `--fond` ou `--papier` | 6,15:1 / 6,65:1 | oui | |
| Contour de l'élément actif au clavier | `--accent-fonce` sur `--papier` ou `--fond` | 9,19:1 / 8,49:1 | oui | 7,81:1 au moins sur une ligne teintée. Le même pour tout lien, bouton ou champ (l. 206-210). |

### Limites de ces mesures

- Le fond réel de chaque texte est **déduit** de la lecture croisée de la feuille de style et
  des gabarits. Ont été **relevés** dans un navigateur (Chromium, le 12/10/2026, dans le
  catalogue d'interface) : le fond, la bordure et le texte des pastilles des quatre états, des
  deux messages et des boutons discrets, et le contour de l'élément actif au clavier. Ils
  valent les jetons du § 3. Le catalogue recopie les gabarits : l'application elle-même n'a
  pas été relevée.
- La couleur du texte **saisi dans un champ** n'est pas fixée par la feuille de style : c'est
  celle du navigateur. Relevé dans Chromium le 12/10/2026 : noir, soit 21:1 sur blanc. Les
  autres navigateurs n'ont pas été essayés.
- Les états désactivés (boutons de dépôt pendant un envoi, opacité 50 %, l. 180) ne sont pas
  soumis aux seuils.

## 7. Règles d'usage

### Constatées dans le code

1. **Bouton principal** (`.bouton`) : l'action principale d'une page ou d'un formulaire —
   « Enregistrer », « Nouveau véhicule », « Modifier ». Au plus un par page, sauf sur une fiche
   vue sur écran tactile, où s'ajoute « Prendre une photo » (`templates/documents/depot.html:21`).
2. **Bouton discret** (`.bouton-discret`) : toute action secondaire — « Annuler »,
   « Se déconnecter », « Ajouter une échéance ». Son bord est `--neutre-trait`, visible sur
   tous les fonds où il est posé (§ 6).
3. **Suppression** : bouton discret en couleur de danger (`.bouton-discret.danger`), toujours
   dans un formulaire qui demande confirmation (`data-confirmer`). C'est le cas des six boutons
   « Supprimer » des gabarits.
4. **Un état coloré porte toujours son libellé.** La pastille écrit l'état en toutes lettres ;
   une ligne d'alerte a sa colonne « Niveau » (`templates/alertes.html:20`) ; une ligne active a
   sa pastille « En cours » (`templates/affectations/liste.html:24`). La couleur renforce, elle
   n'informe jamais seule.
5. **Un état a un fond, une bordure et un texte** (§ 3), et un seul fond : posée sur une ligne
   teintée, la pastille a le fond de la ligne, et c'est sa bordure qui la détache
   (`style.css:109-131`).
6. **Texte secondaire** (`--encre-douce`) : pour ce qui accompagne — explication, intitulé,
   état vide. Jamais pour l'information principale.
7. **Liens** : en couleur d'accent et soulignés. Le soulignement n'est retiré que là où la forme
   dit déjà « cliquable » : nom de l'application, menu, boutons (l. 61, 66, 195).
8. **Élément actif au clavier** : le même contour pour un lien, un bouton ou un champ
   (`style.css:206-210`), à la place de celui du navigateur.
9. **Champs à 16 px sous 700 px de large** : en dessous, un téléphone zoome sur le champ dès
   qu'on le touche (`style.css:267-268`).
10. **Aucune couleur dans les gabarits** : ni attribut `style`, ni bloc `<style>`. Toute
    l'apparence est dans `static/style.css`.
11. **Aucune couleur hors du bloc `:root`** : chaque couleur de la feuille de style passe par
    une variable (§ 9, décision 2).

### Validées le 12/10/2026, pour toute nouvelle page

12. Toute nouvelle couleur entre par une variable du bloc `:root`, avec un nom de rôle.
13. Tout nouveau couple de couleurs est mesuré avant d'être utilisé : `4,5:1` au moins pour un
    texte sur son fond, `3:1` au moins pour une bordure qui doit se voir.
14. Une nouvelle classe d'état reprend l'un des quatre états. Un cinquième état est une
    décision de charte, pas un choix de gabarit.
15. **Un état est une pastille, partout.** À venir pour le statut d'une échéance, encore écrit
    en texte simple (`templates/echeances/liste.html:23`, `templates/vehicules/fiche.html:56`) :
    c'est l'objet de l'issue #23.

## 8. Interdits

### Imposés par le code

L'application envoie au navigateur une consigne de sécurité (`Content-Security-Policy`, posée
dans `src/web/mod.rs:102`) qui lui interdit de charger quoi que ce soit hors du site lui-même.
Sont donc **bloqués par le navigateur**, pas seulement déconseillés :

- une police téléchargée depuis un autre site ;
- une feuille de style ou un script hébergés ailleurs ;
- un style écrit dans la page (`style="…"` ou `<style>`).

### Validés le 12/10/2026

- Une couleur écrite en dur (`#…`) hors du bloc `:root`.
- Un état signalé par la couleur seule, sans libellé.
- Une pastille sans classe d'état : elle prendrait l'état neutre, quel que soit son libellé.
- Le texte d'un état (`--ok`, `--alerte`, `--danger`) sur le fond d'un autre état : ces couples
  ne sont pas mesurés.
- `--sur-accent` ailleurs que sur `--accent` ou `--accent-fonce` : ce blanc tombe à 1,08:1 sur
  le fond de page.
- `--encre-douce` ou `--danger` sur `--accent` : 1,03:1 et 1,01:1, illisible.
- `--trait` comme couleur de texte, comme fond, ou comme bord qui doit se voir : 1,35:1 sur
  blanc. Pour le bord d'un champ, `--trait-fort` ; pour un fond neutre, `--neutre-fond`.
- `--trait-fort` comme bord d'un élément posé sur une ligne teintée : 2,97:1 sur `--ok-fond`,
  2,87:1 sur `--danger-fond`. C'est le rôle de `--neutre-trait`.
- Retirer le contour de l'élément actif au clavier (`outline: none`) sans le remplacer.
- Un champ à moins de 16 px sur téléphone.

## 9. Décisions prises et points ouverts

### Décisions du fondateur, le 12/10/2026

1. **La couleur d'accent est le bleu `#1f5f99`** (`--accent`). Il tient 6,65:1 avec le texte
   blanc des boutons, et 5,66:1 au moins sur les cinq fonds où apparaissent des liens. Une
   autre teinte devrait tenir `4,5:1` sur ces six couples.
2. **Plus aucune couleur écrite en dur.** `#fff`, `#f3b4ae` et `#b7dfc2` sont devenues
   `--sur-accent`, `--danger-trait` et `--ok-trait` ; le fond des champs reprend `--papier`.
3. **Le bord des champs est plus marqué** : `--trait-fort` (`#818d9a`), à 3,38:1 sur le blanc
   d'un formulaire, au lieu de 1,35:1.
4. **Le bord des boutons discrets se voit** : `--neutre-trait` (`#6b7886`), à 3,97:1 au moins
   là où un bouton discret est posé, au lieu de 1,35:1. `--trait-fort` n'a pas été retenu : il
   tombe à 2,97:1 sur la ligne verte d'une affectation en cours, où se trouve « Terminer
   aujourd'hui ».
5. **`--trait` n'est plus un fond.** Les pastilles « terminée / inactif / réformé » ont leur
   état, le neutre, avec `--neutre-fond` (`#e4e8ec`) et `--neutre-trait`. Leur texte passe de
   4,76:1 à 5,24:1.
6. **Chaque état a un texte, un fond et une bordure qui se voit.** Trois jetons s'ajoutent
   (`--ok`, `--alerte`, `--alerte-trait`) ; `--ok-trait` et `--danger-trait` changent de valeur
   pour tenir 3:1 (`#2f8b53` et `#c94c42`, au lieu de `#b7dfc2` et `#f3b4ae`).
7. **Toutes les tailles sont en `rem`**, texte courant compris (`0.9375rem`, soit 15 px), sauf
   les 16 px d'un champ sur téléphone. La précision dans un titre (`h1 small`) passe de 16 à
   15 px : elle était plus grande que le texte courant.
8. **Pas de thème sombre pour l'instant.** Aucune règle ne réagit au réglage « sombre » de
   l'appareil. À rouvrir si des utilisateurs le demandent : toutes les couleurs étant dans
   `:root`, un thème tiendrait dans un seul bloc, mais ses vingt couleurs seraient à choisir et
   à mesurer.
9. **Ni logo ni icône pour l'instant** : le nom de l'application est du texte. À rouvrir quand
   le nom du produit sera arrêté.
10. **Les règles et interdits « proposés » sont validés**, avec ceux que la v2 ajoute (§ 7,
    règles 12 à 15, et § 8).

### Points ouverts

Ils attendent une décision ou un travail ; aucun n'est tranché ici.

1. **Statut d'une échéance en texte simple** : seul écart connu à la règle 15 (issue #23).
2. **Aucun contrôle automatique** ne compare ce document à la feuille de style (§ 10).
3. **La palette n'est pas montrée dans le catalogue d'interface** : aucune story ne l'affiche.
4. **Pas d'échelle d'espacement nommée** (§ 5).
5. **Un seul navigateur essayé** : Chromium. Ni Firefox, ni Safari, ni un vrai téléphone.

## 10. Tenir ce document à jour

Cette charte et [tokens.md](tokens.md) **recopient à la main** des valeurs de
`static/style.css`. Aucun contrôle automatique ne signale un écart. Quand la feuille de style
change :

1. relire le bloc `:root` et corriger les deux fichiers ;
2. refaire le calcul de contraste pour toute couleur modifiée ;
3. vérifier que les numéros de ligne cités sont toujours bons — les commandes de
   [tokens.md](tokens.md), § 5, disent si la feuille de style a bougé.

Le catalogue d'interface (`storybook/`) charge la vraie feuille de style : il montre toujours
les couleurs réelles, sans recopie.
