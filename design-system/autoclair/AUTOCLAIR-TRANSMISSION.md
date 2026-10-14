# Transmission

Ce document résume l'identité Autoclair pour Claude Code, un autre outil de création ou un designer. Il se suffit à lui-même : on peut le copier tel quel dans un fichier `BRAND.md` ou `CLAUDE.md` de projet.

## 1. Identité en bref

- **Marque** : Autoclair (particuliers) et Autoclair Pro (entreprises). Idée centrale : la clarté dans la gestion automobile (retrouver, comprendre, anticiper).
- **Logo** : un **A en forme de route** marine (#062966), avec deux pointillés en perspective et un sommet cyan (#1FC2F6). Le nom est en Montserrat Bold vectorisé.
- **Motifs** : le A-route en grand et rogné ; la **borne** (compte à rebours, mobile) ; la **barre de trajet** (temps restant, tableaux) ; la **fiche à onglet daté** (documents).
- **Ton** : humain, direct, professionnel ; vouvoiement ; le particulier est accompagné, le gestionnaire est en contrôle.
- **Écriture** : « Autoclair », « Autoclair Pro » (jamais AutoClair, AUTOCLAIR, Autoclair PRO).

## 2. Fichiers

| Fichier | Rôle |
|---|---|
| `tokens.json` | Source de vérité des couleurs, typographie, espacements, angles, ombres, dimensions |
| `tokens.css` | Variables CSS (`--marine-700`…, `--font-titre`, `--font-sans`) et classes de texte (`.titre-1`…) générées depuis `tokens.json` |
| `components/bundle.css` | Composants `ac-*` (boutons, cartes, champs, badges, alertes, tableaux, borne, trajet, onglet, navigation) |
| Groupe **Logos** | SVG vectorisés : `autoclair[-pro]-principale[-negatif].svg`, `-verticale[-negatif]`, `-mono-encre`, `-mono-blanc`, `-symbole`, `-symbole-simple` ; symbole nu `autoclair-a-couleur.svg` / `-negatif` |
| Groupe **Exports-PNG** | PNG transparents ; favicons 16/32 (simplifiés), icônes 180/192/512 |

Intégration web minimale :

```html
<link rel="stylesheet" href="tokens.css">
<link rel="stylesheet" href="components/bundle.css">
<body class="ac"> … <button class="ac-btn ac-btn--principal">Ajouter un véhicule</button>
```

En production, auto-héberger Montserrat et Instrument Sans (WOFF2) et retirer l'`@import` Google Fonts de `bundle.css` : c'est plus rapide et cela évite de transmettre l'adresse IP des visiteurs à un tiers (RGPD).

## 3. Couleurs (thème clair)

| Rôle | Token | HEX |
|---|---|---|
| Marque, logo, liens, sélection, texte du bouton principal | `marine-700` | #062966 |
| Pro, fonds sombres | `marine-900` | #03183D |
| Action principale (fond, texte `marine-700` dessus), sommet du A, libellé Pro | `cyan-400` / survol `cyan-300` / pressé `cyan-500` | #1FC2F6 / #5ED3F9 / #0FAEE2 |
| Cyan en texte sur clair (rare) | `cyan-700` | #0A6E93 |
| Secondaires | `marine-500`, `marine-100`, `marine-50`, `cyan-100` | #2350A0, #E3EAF6, #F2F5FB, #DDF5FE |
| Texte, secondaire, bordure de champ | `encre`, `ardoise-600`, `ardoise-500` | #0B1A33, #4A5568, #697586 |
| Surfaces | `blanc`, `fond`, `ligne`, `desactive` | #FFFFFF, #F5F7FB, #D8DFEA, #EDF0F5 |
| Statuts (+ fond) | `succes`, `attention`, `danger` | #1F6F45 / #E7F4EC, #8A4B00 / #FFF4DC, #B42318 / #FDECEA |
| Focus | `focus` (cyan-400 sur fond sombre) | #1F5FCC |

## 4. Typographie, espacements, formes

- **Montserrat** (titres, chiffres) et **Instrument Sans** (textes), toutes deux SIL OFL 1.1. Repli : Segoe UI, Arial.
- Styles :
  - `display` 42/48 et `titre-1` 26/32, en graisse 700 ;
  - `titre-2` 20/26 et `titre-3` 16/22, en graisse 600, Montserrat ;
  - `corps` 15/22, `petit` 13/18, `legende` 12/16, `surtitre` 11/16 en capitales, Instrument Sans ;
  - chiffres tabulaires.
- Grille 4 px (`space-1`…`space-12`) ; angles `radius-sm` 8, `radius-md` 12, `radius-lg` 16, `radius-pill` ; contrôles de 44 px de haut.
- Bordures plutôt qu'ombres ; ombres seulement pour ce qui flotte.

## 5. Règles non négociables

1. Une seule action principale cyan par écran, avec texte marine.
2. Statut = couleur + icône + texte, avec le même sens dans les deux applications ; « bientôt » = moins de 30 jours.
3. Le cyan désigne l'action et la marque, jamais un statut ; jamais de texte cyan sur fond clair (sauf `cyan-700`), jamais de texte blanc sur cyan.
4. Texte ≥ 4,5:1 ; icônes et bordures de champ ≥ 3:1 ; focus toujours visible.
5. Autoclair Pro = icône inversée (A blanc sur fond marine) + libellé « Pro » marine sur pastille cyan.
6. Fonctions disponibles uniquement : véhicules, conducteurs, affectations, interventions, échéances, alertes, documents. Pas de carburant, ni de recharges, ni de rapports financiers, ni de lecture automatique des documents.
7. Aucun témoignage, chiffre de performance, certification ou partenariat non démontré ; données fictives dans les maquettes.
8. Logo depuis les fichiers uniquement ; zone de protection = ¼ de la hauteur du symbole ; minimum 24 px de haut ; favicon en version simplifiée sous 32 px.
9. Le A-route est le seul motif décoratif ; pas de dégradés, ni de bandes colorées sur le bord gauche, ni d'imagerie de garage, de course ou de concession.

## 6. Prompt de continuité de marque

Copier le bloc ci-dessous au début de toute nouvelle demande de création, puis remplir les crochets.

```text
Tu crées un support pour la marque Autoclair en respectant strictement sa charte (logo « A-route », marine et cyan).

SUPPORT : [type de support et format, ex. « publication LinkedIn 1080 × 1080 », « écran mobile 360 px », « courriel de bienvenue »]
APPLICATION : [Autoclair (particuliers) | Autoclair Pro (entreprises)]
PUBLIC : [qui va lire, dans quelle situation, ce qu'il doit comprendre ou faire]
CONTENU : [messages, textes, données ; si des données manquent, utilise des données fictives et signale-le]
CONTRAINTES : [délais, canaux, éléments imposés]

À respecter :
- Logo : utilise uniquement les fichiers fournis (autoclair-principale.svg, -negatif, -verticale, -symbole, -symbole-simple ; version « pro » pour Autoclair Pro). Ne redessine pas le A ni le nom. Zone de protection = ¼ de la hauteur du symbole. Hauteur minimale 24 px. Sous 32 px, icône simplifiée sans pointillés.
- Couleurs : marine-700 #062966 (marque, liens, texte du bouton principal), marine-900 #03183D (Pro, fonds sombres), cyan-400 #1FC2F6 (fond du bouton principal et sommet du A uniquement, jamais en texte sur fond clair), encre #0B1A33, ardoise-600 #4A5568, fond #F5F7FB, ligne #D8DFEA. Statuts : succès #1F6F45, attention #8A4B00, danger #B42318, toujours avec icône + texte.
- Typographie : Montserrat pour les titres et chiffres (600-700), Instrument Sans pour les textes (repli Arial).
- Motif : le A-route en grand, rogné par un bord (jambes marine, sommet cyan) ; aucun autre décor, pas de dégradé.
- Interface : composants ac-* de bundle.css ; une seule action cyan par vue ; borne sur les cartes mobiles, barre de trajet dans les tableaux, onglet daté pour les documents.
- Autoclair Pro : icône A blanc sur fond marine ; libellé « Pro » marine sur pastille cyan.
- Ton : humain, direct, professionnel, vouvoiement ; boutons à l'infinitif ; alertes « quoi, quand, quel véhicule, action » ; espace insécable avant « : ; ! ? ».
- Noms : « Autoclair » et « Autoclair Pro », jamais autrement.
- Interdits : fonctions non disponibles (carburant, recharges, rapports financiers, lecture automatique des documents), témoignages, chiffres de performance, certifications ou partenariats non démontrés, imagerie de concession, de course, de tuning ou de garage.
- Accessibilité : texte ≥ 4,5:1, icônes ≥ 3:1, aucune information par la couleur seule, textes alternatifs.

Avant de livrer, vérifie et indique : respect des couleurs et contrastes, distinction Autoclair / Autoclair Pro, lisibilité en petit format, absence d'affirmation non démontrée, et liste ce qui reste à valider.
```
