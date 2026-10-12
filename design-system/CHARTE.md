# Charte graphique — gestion-parc-auto

> **État au 12/10/2026**, d'après `static/style.css` (230 lignes, 15 variables). Trois décisions
> du fondateur y sont inscrites (§ 9). Ce qui n'est pas encore dans le code est marqué
> « proposé » et attend sa validation.

## 1. À quoi sert ce document

Il dit, en clair, à quoi sert chaque couleur, chaque taille de texte et chaque forme de
l'interface, pour qu'une nouvelle page ressemble aux anciennes sans avoir à deviner.

La **feuille de style** (`static/style.css`, le fichier qui décrit l'apparence des pages) reste
la source de vérité : c'est elle que voit l'utilisateur. Si ce document et la feuille de style
se contredisent, c'est ce document qui est à corriger — ou la feuille de style à modifier, par
décision.

Quatre mots reviennent :

- **variable CSS** : une valeur à laquelle la feuille de style donne un nom (`--accent`), pour
  l'écrire une seule fois et la réutiliser partout avec `var(--accent)` ;
- **jeton** (« token ») : une valeur de la charte qui porte un nom de rôle. Ici, les jetons de
  couleur sont les variables CSS ; leur liste est dans [tokens.md](tokens.md) ;
- **gabarit** : un modèle de page HTML à trous, que le serveur remplit avec les données
  (dossier `templates/`) ;
- **contraste** : l'écart de luminosité entre un texte et son fond, noté comme un rapport
  (`4,5:1`). Plus il est grand, plus le texte se lit facilement.

## 2. Intentions visuelles

| Intention | Ce que cela veut dire | Ce qui le porte dans le code |
| --- | --- | --- |
| **Sérieux** | Un outil de travail, pas une vitrine : pas d'effet, pas d'image décorative, une seule couleur vive. | Une seule couleur d'accent (`--accent`, `style.css:11`). Aucune ombre, aucun dégradé, aucune animation dans la feuille de style. |
| **Lisibilité** | On lit des listes et des dates toute la journée : texte foncé sur fond clair, états dits en toutes lettres. | Texte principal à 13,6:1 sur le fond de page (§ 6). Chaque état coloré porte aussi un libellé (§ 7). |
| **Public** | Des professionnels de la gestion de flotte, au bureau comme sur le terrain, téléphone en main. | Sous 700 px de large, chaque ligne de tableau devient une fiche (`style.css:187-230`) ; champs à 16 px sur téléphone (`style.css:228`). |
| **Sobriété technique** | Rien à télécharger ailleurs : ni police, ni bibliothèque. | Commentaire d'en-tête de la feuille de style (`style.css:1-2`) ; police du système (`style.css:26`). |

Les trois premières intentions sont celles données par le fondateur ; la quatrième est écrite
dans le code.

## 3. Palette par rôle

Quinze couleurs, toutes définies au même endroit (`style.css:4-20`), et **aucune ailleurs**.
Leur nom dit leur **rôle**, pas leur teinte : on écrit `--danger`, pas `--rouge`.

| Rôle | Variable | Valeur | Où on la voit |
| --- | --- | --- | --- |
| Fond de page | `--fond` | `#f5f6f8` | Derrière toute la page ; en-têtes de tableau ; pastille neutre ; survol du menu et des boutons discrets |
| Surface | `--papier` | `#ffffff` | En-tête, tableaux, formulaires et leurs champs : tout ce qui « pose » un contenu sur le fond |
| Texte principal | `--encre` | `#1f2933` | Titres, texte courant, cellules de tableau |
| Texte secondaire | `--encre-douce` | `#52606d` | Explications, intitulés, états vides, menu, en-têtes de tableau |
| Trait | `--trait` | `#d9dee3` | Bordures et séparateurs ; **aussi** le fond des pastilles « terminé / inactif / réformé » |
| Trait marqué | `--trait-fort` | `#818d9a` | Bord d'un champ de formulaire |
| Accent | `--accent` | `#1f5f99` | Bouton principal, liens, page en cours dans le menu |
| Accent appuyé | `--accent-fonce` | `#164a78` | Survol d'un lien ou du bouton principal ; contour de l'élément actif au clavier |
| Texte sur accent | `--sur-accent` | `#ffffff` | Texte du bouton principal et de la page en cours du menu |
| Danger | `--danger` | `#b42318` | Texte d'un message d'erreur, bouton « Supprimer » |
| Fond de danger | `--danger-fond` | `#fde8e6` | Message d'erreur, ligne d'alerte dépassée |
| Trait de danger | `--danger-trait` | `#f3b4ae` | Bord d'un message d'erreur |
| Fond d'avertissement | `--alerte-fond` | `#fff4e0` | Ligne d'alerte à venir, pastilles « en maintenance / à venir » |
| Fond de réussite | `--ok-fond` | `#e6f4ea` | Message de succès, ligne active, pastilles « en service / actif / en cours » |
| Trait de réussite | `--ok-trait` | `#b7dfc2` | Bord d'un message de succès |

Le sens des quatre fonds d'état :

| Fond | Sens | Classes concernées (`style.css`) |
| --- | --- | --- |
| Vert (`--ok-fond`) | En cours, actif, réussi | `.ligne-active` (l. 84), `.statut-en_service`, `.statut-actif`, `.etat-en_cours` (l. 96), `.message-succes` (l. 183) |
| Orangé (`--alerte-fond`) | À surveiller, pas encore là | `.alerte-a-venir` (l. 86), `.statut-en_maintenance`, `.etat-a_venir` (l. 97) |
| Rouge (`--danger-fond`) | Dépassé, refusé | `.alerte-depassee` (l. 85), `.message-erreur` (l. 176) |
| Gris (`--trait`) | Terminé, hors service | `.statut-reforme`, `.statut-inactif`, `.etat-terminee` (l. 98) |

## 4. Typographie

**Police : celle du système.** La feuille de style demande, dans l'ordre, `system-ui`,
`-apple-system`, `"Segoe UI"`, `Roboto`, puis `sans-serif` (`style.css:26`) : chaque appareil
affiche sa police habituelle. Aucun fichier de police n'est téléchargé ni livré avec
l'application — donc aucune licence de police à vérifier, et rien à charger au premier affichage.

| Usage | Taille écrite | Taille à l'écran | Graisse | Ligne |
| --- | --- | --- | --- | --- |
| Titre de page (`h1`) | `1.6rem` | 25,6 px | gras du navigateur | 36 |
| Précision dans le titre (`h1 small`) | `1rem` | 16 px | normale | 37 |
| Titre de section (`h2`) | `1.15rem` | 18,4 px | gras du navigateur | 38 |
| Texte courant (`body`) | `15px` | 15 px | normale | 27 |
| Texte d'aide, en-tête de tableau, bouton discret | `0.9rem` | 14,4 px | normale (600 pour l'en-tête de tableau) | 68, 79, 167 |
| Pastille | `0.85rem` | 13,6 px | normale | 92 |
| Champ de formulaire | hérite du texte courant | 15 px ; **16 px sous 700 px de large** | normale | 114, 228 |
| Intitulé de champ | hérite | 15 px | 600 | 105 |
| Nom de l'application (`.marque`) | hérite | 15 px | 700 | 49 |

Interligne : `1.45` partout (`style.css:28`). Italique : uniquement pour un état vide (`.vide`,
l. 67).

Deux choses à savoir en lisant ce tableau :

- le `rem` est une unité relative à la taille de texte **de base du navigateur**, 16 px sauf
  réglage contraire de l'utilisateur. La feuille de style fixe 15 px sur `body`, pas sur la
  racine de la page : les tailles en `rem` se calculent donc sur 16 px et non sur 15. La colonne
  « à l'écran » est **déduite** de cette règle, pas mesurée dans un navigateur ;
- conséquence : la précision en petit dans un titre (`h1 small`, 16 px) est plus grande que le
  texte courant (15 px).

## 5. Formes, traits et largeurs

| Élément | Valeur | Lignes |
| --- | --- | --- |
| Arrondi des éléments de saisie et d'action (menu, champs, boutons, messages) | `6px` | 53, 117, 158, 175, 182 |
| Arrondi des surfaces (tableau, formulaire, fiche d'une ligne sur écran étroit) | `8px` | 75, 104, 205 |
| Arrondi de la pastille | `999px` (bords entièrement ronds) | 91 |
| Épaisseur d'un trait | `1px` | partout |
| Contour de l'élément actif au clavier | `2px`, décalé de `2px` | 136 |
| Largeur maximale du contenu | `1100px` | 61 |
| Largeur maximale d'un formulaire | `560px` | 104 |
| Largeur d'une page sans menu (connexion, erreur) | `420px` | 62 |
| Seuil « écran étroit » | `700px` | 187 |

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
| `--encre` | `--ok-fond` | 12,99:1 | oui | Ligne active, pastilles vertes, message de succès |
| `--encre` | `--alerte-fond` | 13,53:1 | oui | Ligne d'alerte à venir, pastilles orangées |
| `--encre` | `--danger-fond` | 12,55:1 | oui | Ligne d'alerte dépassée |
| `--encre-douce` | `--fond` | 5,97:1 | oui | Sous-titre, aide, état vide, en-tête de tableau, intitulés d'une fiche |
| `--encre-douce` | `--papier` | 6,45:1 | oui | Menu, compte connecté, aide dans un formulaire |
| `--encre-douce` | `--trait` | **4,76:1** | oui, de peu | Pastilles « terminé / inactif / réformé » — le rapport le plus faible de l'interface |
| `--encre-douce` | `--ok-fond` | 5,68:1 | oui | Intitulé d'une cellule sur écran étroit, ligne active |
| `--encre-douce` | `--alerte-fond` | 5,92:1 | oui | Idem, ligne d'alerte à venir |
| `--encre-douce` | `--danger-fond` | 5,49:1 | oui | Idem, ligne d'alerte dépassée |
| `--accent` | `--fond` | 6,15:1 | oui | Liens dans le texte, bouton discret hors d'une surface |
| `--accent` | `--papier` | 6,65:1 | oui | Liens dans un tableau, bouton discret dans un formulaire ou l'en-tête |
| `--accent` | `--ok-fond` | 5,86:1 | oui | Liens d'une ligne active, liens d'un message de succès |
| `--accent` | `--alerte-fond` | 6,10:1 | oui | Liens d'une ligne d'alerte à venir |
| `--accent` | `--danger-fond` | 5,66:1 | oui | Liens d'une ligne d'alerte dépassée |
| `--accent-fonce` | les cinq fonds ci-dessus | 7,81:1 à 9,19:1 | oui | Lien survolé |
| `--sur-accent` | `--accent` | 6,65:1 | oui | Bouton principal, page en cours dans le menu |
| `--sur-accent` | `--accent-fonce` | 9,19:1 | oui | Bouton principal survolé |
| `--danger` | `--danger-fond` | 5,59:1 | oui | Message d'erreur, bouton « Supprimer » survolé |
| `--danger` | `--papier` | 6,57:1 | oui | Bouton « Supprimer » dans un tableau |
| `--danger` | `--fond` | 6,07:1 | oui | Bouton « Supprimer » à côté d'un titre |
| `--danger` | `--ok-fond` | 5,78:1 | oui | Bouton « Supprimer » d'une ligne active |

Tous les textes passent le seuil AA.

### Éléments d'interface

| Élément | Couleurs | Rapport | Seuil 3:1 | Lecture |
| --- | --- | --- | --- | --- |
| Bord d'un champ de formulaire | `--trait-fort` sur `--papier` | 3,38:1 | oui | Le champ et le formulaire ont le même fond blanc : ce trait est le seul repère du champ. Il était à 1,35:1 avec `--trait` (§ 9, décision 3). |
| Bord d'un bouton discret | `--trait` sur `--papier` ou `--fond` | 1,35:1 / 1,25:1 | non | Le bouton reste reconnaissable par son texte en couleur d'accent ou de danger (5,59:1 au moins). À trancher (§ 9). |
| Bord d'un message d'erreur | `--danger-trait` sur `--danger-fond` | 1,49:1 | non | Décoratif : le message se reconnaît à son fond et à son texte. |
| Bord d'un message de succès | `--ok-trait` sur `--ok-fond` | 1,28:1 | non | Décoratif, même raison. |
| Contour des surfaces | `--trait` sur `--fond` | 1,25:1 | non | Décoratif : aucune information n'en dépend. |
| Bouton principal sur la page | `--accent` sur `--fond` / `--papier` | 6,15:1 / 6,65:1 | oui | |
| Contour de l'élément actif au clavier | `--accent-fonce` sur `--papier` | 9,19:1 | oui | Défini pour les boutons de dépôt (l. 136) ; ailleurs, c'est le contour par défaut du navigateur. |

### Limites de ces mesures

- Le fond réel de chaque texte est **déduit** de la lecture croisée de la feuille de style et
  des gabarits ; il n'a pas été relevé un à un dans un navigateur.
- La couleur du texte **saisi dans un champ** n'est pas fixée par la feuille de style : c'est
  celle du navigateur. Relevé dans Chromium le 12/10/2026 : noir, soit 21:1 sur blanc. Les
  autres navigateurs n'ont pas été essayés.
- Les états désactivés (boutons de dépôt pendant un envoi, opacité 50 %, l. 148) ne sont pas
  soumis aux seuils.

## 7. Règles d'usage

### Constatées dans le code

1. **Bouton principal** (`.bouton`) : l'action principale d'une page ou d'un formulaire —
   « Enregistrer », « Nouveau véhicule », « Modifier ». Au plus un par page, sauf sur une fiche
   vue sur écran tactile, où s'ajoute « Prendre une photo » (`templates/documents/depot.html:21`).
2. **Bouton discret** (`.bouton-discret`) : toute action secondaire — « Annuler »,
   « Se déconnecter », « Ajouter une échéance ».
3. **Suppression** : bouton discret en couleur de danger (`.bouton-discret.danger`), toujours
   dans un formulaire qui demande confirmation (`data-confirmer`). C'est le cas des six boutons
   « Supprimer » des gabarits.
4. **Un état coloré porte toujours son libellé.** La pastille écrit l'état en toutes lettres ;
   une ligne d'alerte a sa colonne « Niveau » (`templates/alertes.html:20`) ; une ligne active a
   sa pastille « En cours » (`templates/affectations/liste.html:24`). La couleur renforce, elle
   n'informe jamais seule.
5. **Texte secondaire** (`--encre-douce`) : pour ce qui accompagne — explication, intitulé,
   état vide. Jamais pour l'information principale.
6. **Liens** : en couleur d'accent et soulignés. Le soulignement n'est retiré que là où la forme
   dit déjà « cliquable » : nom de l'application, menu, boutons (l. 49, 54, 163).
7. **Champs à 16 px sous 700 px de large** : en dessous, un téléphone zoome sur le champ dès
   qu'on le touche (`style.css:227-228`).
8. **Aucune couleur dans les gabarits** : ni attribut `style`, ni bloc `<style>`. Toute
   l'apparence est dans `static/style.css`.
9. **Aucune couleur hors du bloc `:root`** : chaque couleur de la feuille de style passe par
   une variable (§ 9, décision 2).

### Proposées, à valider

10. Toute nouvelle couleur entre par une variable du bloc `:root`, avec un nom de rôle.
11. Tout nouveau couple texte / fond est mesuré avant d'être utilisé : `4,5:1` au moins.
12. Une nouvelle classe d'état reprend l'un des quatre fonds existants plutôt que d'en créer un
    cinquième.

## 8. Interdits

### Imposés par le code

L'application envoie au navigateur une consigne de sécurité (`Content-Security-Policy`, posée
dans `src/web/mod.rs:102`) qui lui interdit de charger quoi que ce soit hors du site lui-même.
Sont donc **bloqués par le navigateur**, pas seulement déconseillés :

- une police téléchargée depuis un autre site ;
- une feuille de style ou un script hébergés ailleurs ;
- un style écrit dans la page (`style="…"` ou `<style>`).

### Proposés, à valider

- Une couleur écrite en dur (`#…`) hors du bloc `:root`.
- Un état signalé par la couleur seule, sans libellé.
- `--sur-accent` ailleurs que sur `--accent` ou `--accent-fonce` : ce blanc tombe à 1,08:1 sur
  le fond de page.
- `--encre-douce` ou `--danger` sur `--accent` : 1,03:1 et 1,01:1, illisible.
- `--trait` comme couleur de texte, ou comme bord d'un champ : 1,35:1 sur blanc. Pour un bord
  qui doit se voir, `--trait-fort`.
- Retirer le contour de l'élément actif au clavier (`outline: none`) sans le remplacer.
- Un champ à moins de 16 px sur téléphone.

## 9. Décisions prises et points ouverts

### Décisions du fondateur, le 12/10/2026

1. **La couleur d'accent est le bleu `#1f5f99`** (`--accent`). Il tient 6,65:1 avec le texte
   blanc des boutons, et 5,66:1 au moins sur les cinq fonds où apparaissent des liens. Une
   autre teinte devrait tenir `4,5:1` sur ces six couples.
2. **Plus aucune couleur écrite en dur.** `#fff`, `#f3b4ae` et `#b7dfc2` sont devenues
   `--sur-accent`, `--danger-trait` et `--ok-trait` ; le fond des champs reprend `--papier`.
   Le rendu n'a pas changé.
3. **Le bord des champs est plus marqué** : `--trait-fort` (`#818d9a`), à 3,38:1 sur le blanc
   d'un formulaire, au lieu de 1,35:1. C'est le seul changement visible de cette série.

### Points ouverts

Ils attendent une décision ; aucun n'est tranché ici.

1. **Bord des boutons discrets** à 1,35:1 sur une surface et 1,25:1 sur le fond de page.
   `--trait-fort` y tiendrait 3,38:1 et 3,12:1.
2. **`--trait` sert aussi de fond** (pastilles grises, l. 98) : un même nom pour deux rôles.
3. **États incomplets** : le danger a un texte, un fond et un trait ; la réussite, un fond et
   un trait ; l'avertissement, un fond seulement.
4. **Tailles** : mélange de `px` et de `rem` (§ 4).
5. **Pas de thème sombre** : aucune règle ne réagit au réglage « sombre » de l'appareil.
6. **Ni logo ni icône** : le nom de l'application est du texte.
7. **Règles et interdits « proposés »** des § 7 et § 8 : à valider ou à écarter.

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
