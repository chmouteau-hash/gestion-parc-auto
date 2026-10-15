# Charte graphique — comment l'application applique Autoclair

> **État au 14/10/2026.** L'application porte la charte **Autoclair** (direction « A-route »,
> marine et cyan, paquet du 13/10/2026) : identité, polices, couleurs, boutons, statuts, et les
> deux accueils avec la borne et la barre de trajet. La navigation propre à chaque profil, la
> photo du véhicule et l'onglet daté des documents restent à faire (§ 10).

## 1. À quoi sert ce document

La charte elle-même est dans [autoclair/](autoclair/) : c'est le paquet reçu, **laissé tel
quel**. Elle fait foi pour tout ce qu'elle dit. Ce document dit autre chose : comment
l'application la met en œuvre, et où elle s'en écarte, avec la raison.

Si ce document et la feuille de style (`static/style.css`) se contredisent, la feuille de style
est ce que voit l'utilisateur : c'est ce document qu'il faut corriger, ou la feuille de style
qu'il faut modifier, par décision.

Trois mots reviennent :

- **jeton** (« token ») : une valeur de la charte qui porte un nom de rôle (`marine-700`,
  `space-4`). Les jetons sont dans `autoclair/code/tokens.css` ; le serveur les sert tels
  quels, sous `/static/tokens.css`, avant la feuille de style ;
- **contraste** : l'écart de luminosité entre un texte et son fond, noté comme un rapport
  (`4,5:1`). Plus il est grand, plus le texte se lit facilement ;
- **pastille** : la petite étiquette arrondie qui écrit un état en toutes lettres
  (« En service »), avec son icône.

## 2. Deux marques, deux profils

Une installation a un profil, réglé par `PROFIL` (voir le README). Chaque profil a sa marque :

| Profil | Marque | Logo de l'en-tête | Icône de l'onglet |
| --- | --- | --- | --- |
| `particulier` | **Autoclair** | `autoclair-principale.svg` | `autoclair-symbole-simple.svg` (A marine sur blanc) |
| `entreprise` | **Autoclair Pro** | `autoclair-pro-principale.svg` | `autoclair-pro-symbole-simple.svg` (A blanc sur marine) |

Le nom est écrit « Autoclair » ou « Autoclair Pro », jamais autrement. Le logo vient toujours
des fichiers de la charte, jamais recomposé en texte. `Profil::marque`, `Profil::logo` et
`Profil::icone` (`src/web/rendu.rs`) donnent le nom, le logo et l'icône ; le titre de chaque page
se termine par le nom (« Alertes · Autoclair Pro »).

La page d'erreur n'a pas accès au profil : elle porte le nom « Autoclair » et aucun logo.

Sur l'écran d'accueil d'un téléphone, chaque marque a son icône d'application : l'export PNG du
symbole avec pointillés, celui que la charte prévoit « à partir de 48 px »
(`autoclair/logos/png/autoclair-symbole-512.png`, 192 et 180 px ; `autoclair-pro-symbole-…` pour
Pro). Seules ces six icônes du groupe Exports-PNG sont dans le dépôt, servies sous
`/static/logos/`. Le manifest d'application web (`/manifest.webmanifest`, `Profil::manifeste`)
donne le nom de la marque et deux couleurs lues dans les jetons, jamais écrites : la barre d'état
prend `--marine-700` pour Autoclair Pro (sa barre latérale) et `--blanc` pour Autoclair (son
en-tête) ; l'écran d'attente a le fond des pages, `--fond`. Aucune icône « maskable » : elle
demanderait de recomposer le symbole avec une marge, ce que la charte interdit ; si Android rogne
l'icône, c'est à la charte de livrer cette variante.

## 3. Ce que l'application reprend

| Élément de la charte | Dans l'application |
| --- | --- |
| Jetons (couleurs, espacements, angles, hauteur de contrôle, polices) | `autoclair/code/tokens.css`, servi tel quel ; aucune valeur recopiée |
| Polices Montserrat (titres) et Instrument Sans (textes) | `static/polices/*.woff2`, servies par l'application : aucune demande à un tiers |
| Logos | `autoclair/logos/svg/`, embarqués dans l'exécutable |
| Boutons principal (cyan, texte marine) et secondaire | `.bouton` et `.bouton-discret` |
| Statuts : couleur, icône, texte | `.pastille` et ses classes d'état (§ 5) |
| Champs de 44 px, bord `ardoise-500` de 1,5 px | `.formulaire input, select, textarea` |
| Tableaux : en-tête « surtitre », cellules de 12 × 16 px | `.tableau` |
| Cartes sur écran étroit : une ligne de tableau devient une fiche, sous 700 px, ou plus large quand le tableau déborde de son cadre et que l'écran fait 500 px de haut au moins (`app.js` pose `.en-fiches` d'après la mesure ; sur plusieurs colonnes au-dessus de 700 px) | `.tableau` sous 700 px, `.tableau-cadre.en-fiches` |
| Tableau plus large que la page (paysage, texte agrandi, fenêtre réduite) : il défile dans son cadre, la page reste en place, et la colonne des actions reste collée au bord droit | `.tableau-cadre` autour de chaque tableau (il porte le contour et les angles) ; `position: sticky` sur la dernière cellule sans intitulé, au-dessus de 700 px ; barre de défilement du cadre masquée sur écran tactile (`pointer: coarse`), gardée à la souris |
| Tableau au plus serré (700 à 1 280 px) : marges de cellule de 8 px, boutons de la colonne des actions à la ligne, pour que la colonne collée ne recouvre pas la pastille d'état | `@media (700px < width <= 1280px)` sur `.tableau` |
| Partie d'une liste qui n'est que de l'historique (affectations terminées, véhicules réformés) : repliée sous la liste, titre à toucher de 44 px, ouverte par un lien vers son `id` ; les gestes d'une ligne y restent à portée (supprimer, et « Remettre en service » pour un réformé) | `details.repliee` et son `summary` |
| Borne (compte à rebours, tête colorée) | `.borne` et `borne-depassee`, `-a-venir`, `-a-jour`, `-neutre` (accueil Autoclair) |
| Barre de trajet (d'aujourd'hui à l'échéance) | `.trajet` + `trajet-0` à `trajet-100` par dizaines (tableau de bord Pro) |
| Écran vide : A-route en `marine-100`, sommet cyan, phrase, action | macro `vide` de `templates/macros.html`, `.ecran-vide` |
| Indicateurs `chiffre-xl` | `.chiffres` (tableau de bord Pro) |

### Classes de l'application et composants de la charte

La charte décrit des classes `ac-*` (`autoclair/code/bundle.css`). L'application garde ses
propres noms de classe, que ses gabarits et ses tests emploient, et leur donne l'apparence du
composant correspondant :

| Classe de l'application | Composant de la charte |
| --- | --- |
| `.bouton` | `ac-btn--principal` |
| `.bouton-discret` | `ac-btn--secondaire` en petit (`--sm`) ; avec `.danger` : `ac-btn--danger` |
| `.pastille` + classe d'état | `ac-badge--succes`, `--attention`, `--danger`, `--neutre` |
| `.message-erreur`, `.message-succes` | `ac-alerte--danger`, `ac-alerte--succes` |
| `.tableau` | `ac-tableau` dans `ac-tableau-cadre` |
| `.formulaire` | `ac-champ`, `ac-label`, `ac-input` |
| `.menu a`, `.menu a.actif` | `ac-tabbar__item` (page en cours : fond `marine-100`, texte gras) |
| `.borne` | `ac-borne--*` |
| `.trajet` | `ac-trajet--*` |
| `.carte-vehicule`, `.chiffres a`, `.liste-simple` | `ac-carte` |
| `.message-attention` + `.bandeau` | `ac-alerte--attention`, avec le A-route en filigrane |

## 4. Polices et licences

Montserrat et Instrument Sans sont sous licence SIL OFL 1.1 : usage commercial, intégration et
auto-hébergement permis (textes dans `autoclair/licences/`). Les fichiers de `static/polices/`
sont les WOFF2 « latin » à graisse variable du paquet npm `@fontsource-variable` 5.3.0
(Montserrat 38 Ko, Instrument Sans 30 Ko) : ils couvrent tout le français (accents, œ, « », €).

Les icônes de navigation et de section (`data-ico`) viennent du jeu **Lucide** (licence ISC,
texte dans `static/LICENCE-lucide.txt`, version `lucide-static` 0.468.0) : recopiées en masques
CSS dans `static/style.css`, comme les icônes de statut, donc servies par l'application.

`autoclair/code/bundle.css` n'est **pas chargé** : sa première ligne importe les polices depuis
Google Fonts, ce qui transmettrait l'adresse IP de chaque visiteur à un tiers et que la
consigne de sécurité du serveur refuse (`default-src 'self'`). La charte conseille elle-même
l'auto-hébergement.

## 5. Statuts : quatre sens, les mêmes dans les deux marques

Une information n'est jamais portée par la couleur seule : chaque statut a son icône et ses mots.
Les icônes sont dessinées en CSS (masque d'image `data:`), au trait de 2 px de la charte, et
prennent la couleur du texte.

| Sens | Couleur | Icône | Classes de l'application |
| --- | --- | --- | --- |
| À jour | `succes` | coche | `statut-en_service`, `statut-actif`, `etat-en_cours` |
| Bientôt | `attention` | horloge | `statut-en_maintenance`, `etat-a_venir`, `echeance-a_traiter`, `echeance-a_venir`, `permis-a_venir`, `alerte-a-venir` |
| Dépassé | `danger` | triangle | `echeance-depassee`, `permis-expire`, `alerte-depassee` |
| Sans suite | `ardoise-600` sur `desactive` | tiret (réformé, inactif, terminée) ou coche (échéance traitée) | `statut-reforme`, `statut-inactif`, `etat-terminee`, `echeance-traitee` |

Le tiret n'est pas dans le jeu d'icônes de la charte, qui dit de l'étendre « dans le même
style » : c'est une extension, signalée ici. Le cyan n'est jamais un statut : il désigne
l'action.

## 6. Contrastes

Chaque couple de couleurs que la feuille de style pose est contrôlé par
`tests/contrastes.rs`, qui lit les jetons : 4,5:1 au moins pour un texte, 3:1 au moins pour un
bord de champ, une icône ou le contour du clavier. Le test échoue si une teinte change et fait
passer un couple sous son seuil. Les rapports mesurés le 14/10/2026 :

| Couple | Rapport | Où |
| --- | --- | --- |
| `encre` / `blanc` | 17,4:1 | texte courant |
| `ardoise-600` / `blanc`, `fond` | 7,5:1, 7,0:1 | texte secondaire, en-têtes de tableau |
| `marine-700` / `blanc` | 13,9:1 | liens, bouton secondaire |
| `marine-700` / `cyan-400` | 6,7:1 | bouton principal (5,4:1 pressé, 8,0:1 au survol) |
| `marine-700` / `marine-100` | 11,5:1 | page en cours du menu |
| `succes` / `succes-fond` | 5,4:1 | pastille « à jour » |
| `attention` / `attention-fond` | 6,2:1 | pastille « bientôt » |
| `danger` / `danger-fond` | 5,8:1 | pastille « dépassé » |
| `ardoise-500` / `blanc` | 4,7:1 | bord d'un champ (seuil 3:1) |
| `focus` / `blanc` | 5,9:1 | contour du clavier (seuil 3:1) |

Ces mesures disent que les couleurs tiennent les seuils. Elles ne disent pas que l'application
est conforme au RGAA ou aux WCAG : cela demande un audit (taille de texte agrandie, lecteur
d'écran, vrai téléphone), qui n'a pas été fait.

## 7. Règles d'usage

1. **Un seul bouton principal par écran** (`.bouton`, cyan). Les autres actions sont
   `.bouton-discret`. Le lien d'évitement « Aller au contenu » est blanc, pour ne pas faire un
   second bouton cyan à côté. Sur une liste vide, le bouton de l'écran vide est ce bouton : la
   barre de titre ne le répète pas.
2. **Du marine sur le cyan, jamais du blanc** (2,1:1). Jamais de cyan en texte sur fond clair.
3. **Suppression** : `.bouton-discret.danger`, dans un formulaire qui demande confirmation
   (`data-confirmer`).
4. **Un état coloré porte toujours son libellé et son icône** : la couleur renforce, elle
   n'informe jamais seule.
5. **Une pastille sans classe d'état** prend l'état neutre. Un nouvel état reprend l'un des
   quatre sens du § 5 ; un cinquième est une décision de charte.
6. **Aucune couleur écrite à la main** dans la feuille de style : chacune est un jeton
   (`var(--marine-700)`). Un test (`la_feuille_de_style_n_ecrit_aucune_couleur`) le vérifie.
   Aucune couleur non plus dans les gabarits : ni attribut `style`, ni bloc `<style>`.
7. **Tailles de texte en `rem`**, pour suivre le réglage du navigateur (15 px = `0.9375rem`),
   sauf les 16 px d'un champ sur téléphone, en dessous desquels il zoome sur le champ.
8. **Au doigt (sous 700 px)** : actions de 44 px de haut au moins, séparées de 8 px,
   « Supprimer » rangé à l'écart. 44 px est un repère de confort, pas une preuve de conformité.
9. **Contour du clavier** : le même pour un lien, un bouton ou un champ (`--focus`), jamais
   retiré sans être remplacé.
10. **Pas d'italique** dans l'interface, ni de bande de couleur sur le bord d'une carte (charte,
    « Langage graphique »). Pas de dégradé non plus, à une exception près, décidée le
    14/10/2026 : la barre latérale d'Autoclair Pro, de `marine-900` en haut à `marine-700` en bas
    (§ 9).
11. **Un nouveau couple de couleurs** s'ajoute à `tests/contrastes.rs` avant d'être utilisé.
12. **Une date limite s'accompagne de son délai** (« · dans 15 jours », classe `.delai`), partout :
    listes, fiches, accueils. Le calcul est celui de la page Alertes (`metier::alertes::delai_fr`).
13. **Les nombres se lisent** : `160 000 km`, `149,90 €` (`commun::format`).
14. **Une fiche ou un formulaire commence par un lien de retour** (`.retour`, « ← Véhicules »)
    vers la liste d'où l'on vient : sur un téléphone, la flèche du navigateur n'est pas toujours
    à portée (demandé par Christophe le 14/10/2026).
15. **Un véhicule réformé ne figure pas parmi « mes véhicules »** sur l'accueil : il reste dans la
    liste, et l'accueil dit combien il y en a.

## 8. Interdits

L'application envoie au navigateur une consigne de sécurité (`Content-Security-Policy`, dans
`src/web/mod.rs`) : `default-src 'self'; img-src 'self' data:; …`. Sont donc **bloqués par le
navigateur**, pas seulement déconseillés :

- une police, une feuille de style ou un script hébergés ailleurs que sur le site ;
- un style écrit dans la page (`style="…"` ou `<style>`).

Les images `data:` sont admises : c'est ce qui permet aux icônes de statut d'être dessinées en
CSS. Elles ne doivent servir qu'à cela.

La charte (`autoclair/charte/`) ajoute ses propres interdits : logo déformé, recoloré ou posé
sur un fond chargé ; fonctions non disponibles annoncées (carburant, recharges, rapports
financiers, lecture automatique des documents) ; témoignage, chiffre de performance,
certification ou partenariat non démontré ; données réelles dans une maquette.

## 9. Écarts et décisions

### Écarts avec le paquet de charte

| Écart | Raison |
| --- | --- |
| Classes `ac-*` non reprises ; `bundle.css` non chargé | Les gabarits, les tests et le catalogue emploient les classes de l'application ; réécrire les dix-neuf gabarits, leurs tests et les stories du catalogue n'apportait rien à l'écran. À revoir si la charte évolue souvent. |
| `.bouton-discret` a le bord du bouton secondaire | Le bord rend le bouton repérable sans lire son texte (décision du 12/10/2026, conservée). |
| Lignes de tableau teintées (en cours, dépassée, à venir) | La charte n'en prévoit pas. Gardées, avec une pastille blanche pour s'en détacher : un tableau de dix échéances se lit d'un coup d'œil. |
| Icône « tiret » ajoutée au jeu | Il fallait un signe pour « sans suite » (§ 5). |
| Tailles en `rem` | La charte les donne en pixels ; `rem` suit le réglage du navigateur. |
| **Deux navigations** (décidé le 14/10/2026 sur maquette). Autoclair Pro : barre latérale de 260 px, logo négatif en tête, recherche, menu complet avec icônes, page en cours en cyan ; repliée derrière un bouton « Menu » sous 1 024 px. Autoclair : quatre onglets Véhicules · Échéances · Entretien · Plus, en bas du téléphone, en tête sur un grand écran ; le reste du menu (Alertes, tous les véhicules, Conducteurs, Déposer, déconnexion) et la recherche sont sur la page « Plus ». | La charte décrit la barre latérale pour Pro ; les onglets viennent de la maquette validée. Un particulier a trois choses à faire, une entreprise en a huit. |
| **Dégradé** `marine-900` → `marine-700` sur la barre latérale Pro | Décidé le 14/10/2026 : la seule exception à « pas de dégradé » (règle 10). Doux, vertical, sur un seul élément ; le blanc tient 4,5:1 sur les deux teintes (`tests/contrastes.rs`). |
| **Icônes au trait devant chaque entrée de menu et chaque titre de section** (`data-ico`, jeu Lucide, `static/LICENCE-lucide.txt`) | La charte prévoit des icônes au trait, sans jeu attitré. Lucide (ISC) est dessiné sur la même grille de 24 que les icônes de statut, et recopié en masques CSS comme elles : aucune demande à un tiers. |
| **Photo du véhicule** (`.carte-photo`, `.fiche-photo`) : celle que l'utilisateur a choisie, à défaut la dernière déposée, en vignette de 640 px calculée par le serveur ; sans photo, une silhouette de voiture en `marine-100`. À partir de deux photos, un bloc « Photos » (`.photos`, `.photo-vignette`) les montre toutes en vignettes sur la fiche, la photo de la fiche en premier et cerclée de `marine-500` (#66) | Décidé le 14/10/2026 : la règle « pas de photo dans l'interface » vise les photos d'illustration ; la photo du véhicule de l'utilisateur est un **contenu**, comme sa plaque. Pas de photo d'ambiance, nulle part. |
| **Boîte de confirmation** (`<dialog class="confirmation">`, fond assombri en marine) pour les suppressions, avec au besoin un lien « autre choix » (`data-confirmer-alternative`) entre « Annuler » et « Supprimer » ; **message de réussite** en tête de la page qui suit une action | La boîte du navigateur n'a ni les couleurs ni les polices de la charte ; le message réemploie `.message-succes`. Décidé le 14/10/2026 (bloc 5). |
| **Initiales en rond** (`.initiales`, `marine-700` sur `marine-100`) devant le nom d'un conducteur | Extension validée le 14/10/2026 sur maquette : le pendant de la plaque pour une personne. Décoratives, le nom suit toujours. |
| Nom du profil : le logo, plus une pastille de texte | Le logo dit déjà Autoclair ou Autoclair Pro. C'était la pastille de la PR #38 (« la page de connexion dit le profil ») ; l'information reste dite, par le logo, son texte alternatif et le titre. |
| **Plaque d'immatriculation dessinée** (`.plaque` : fond blanc, contour `encre`, bande `marine-500` à gauche avec le cercle d'étoiles et un « F », bande étroite à droite) | Extension validée le 14/10/2026 sur maquette, redessinée à la française le 15/10/2026 sur demande de Christophe (« une vraie plaque »). Employée partout où l'immatriculation est un titre ou la clé d'une ligne : listes, fiches, accueil. Le décor est décoratif, le texte reste en encre ; pas de département ni de région, qu'on ne connaît pas. |
| **Bandeau d'accueil « attention »** (`.message-attention`) | La charte n'a que les messages d'erreur et de réussite ; le bandeau « une chose à faire » pour une échéance à venir a besoin de l'orange. Sert aussi, sans filigrane, à l'avertissement d'homonyme du formulaire de création d'un conducteur. |
| **Borne « à jour »** en vert au-delà de l'horizon, « neutre » sans échéance | La charte décrit la borne par statut ; les deux cas sans urgence sont précisés ici. |
| Barre de trajet remplie par dizaines | La consigne de sécurité interdit un style écrit dans la page : onze classes plutôt qu'une largeur calculée. L'œil ne voit pas la différence. |

### Décisions du 12/10/2026 : ce qui change

La charte v2 de l'application (12 et 13/10/2026) avait dix décisions. La charte Autoclair en
remplace ou en lève certaines ; les autres tiennent encore.

| N° | Décision | Aujourd'hui |
| --- | --- | --- |
| 1 | Accent bleu `#1f5f99` | **Remplacée** : marine `#062966` pour la marque, cyan `#1FC2F6` pour l'action |
| 2 | Plus aucune couleur en dur | Tient (règle 6), et un test la garde |
| 3 | Bord des champs plus marqué | Tient : `ardoise-500`, 4,7:1 |
| 4 | Bord des boutons discrets visible | Tient : bord `marine-700` |
| 5 | `--trait` n'est plus un fond ; état neutre | Tient : le neutre est `desactive` + `ardoise-600` |
| 6 | Quatre états à trois jetons (texte, fond, bordure) | **Remplacée** : deux jetons (texte, fond) et une icône |
| 7 | Tailles en `rem` | Tient (règle 7) |
| 8 | Pas de thème sombre | Tient : la charte n'en définit pas non plus |
| 9 | Ni logo ni icône « tant que le nom n'est pas arrêté » | **Levée** : le nom est Autoclair, le logo est celui de la charte |
| 10 | Règles et interdits validés | Repris et mis à jour aux § 7 et 8 |

## 10. Reste à faire et réserves

Ce qui n'est **pas** dans l'application, et que la charte demande (l'issue #39 porte le détail) :

1. **La fiche à onglet daté** (documents) : la table `documents` n'a pas de date d'expiration.
2. **Textes à la voix du profil** : « Autoclair accompagne, Autoclair Pro donne le contrôle »
   (la page de connexion dit encore « le suivi du parc » aux deux).
3. **Icône d'application (PNG 180 et 192 px)** pour l'écran d'accueil du téléphone : les PNG de
   la charte ne sont pas versionnés.

Réserves de la charte (`autoclair/charte/00-fondements.md`), reprises ici parce que le dépôt
est public : le nom et le logo ne sont **pas vérifiés juridiquement** (recherche d'antériorité
INPI et EUIPO à faire) ; le logo vient d'une image générée par une IA, que la charte a
redessinée, et sa protection est incertaine. L'image d'origine et le script de redessin du
paquet ne sont pas versionnés ici.

## 11. Tenir ce document à jour

- Une teinte ou un jeton change dans `autoclair/code/` : lancer `cargo test --test contrastes`,
  puis corriger le tableau du § 6.
- Un statut ou une classe d'état s'ajoute : compléter le § 5 et `tests/http.rs` ou
  `tests/alertes.rs` si la page le montre.
- Un gabarit change : reporter le HTML dans les stories du catalogue (`storybook/stories/`).
- Une décision de charte se prend : l'inscrire au § 9, avec sa date.

Le catalogue d'interface (`storybook/`) charge les vrais jetons et la vraie feuille de style : il
montre toujours les couleurs réelles, sans recopie.
