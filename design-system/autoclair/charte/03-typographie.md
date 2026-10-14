# Typographie

Deux familles, aux rôles nets :
- **Montserrat**, géométrique, porte la voix de la marque : le nom du logo, les titres, les chiffres.
- **Instrument Sans**, plus étroite et très lisible, porte les textes et les données denses.

Montserrat a été choisie parce qu'elle est la plus proche, parmi les polices à licence vérifiée, du lettrage de l'image d'origine. La police exacte de cette image n'a pas pu être identifiée avec certitude.

## Sources et licences

| Police | Source officielle | Licence | Vérification |
|---|---|---|---|
| Montserrat (variable, graisses 100 à 900) | Google Fonts, dépôt `github.com/google/fonts/tree/main/ofl/montserrat` ; projet d'origine `github.com/JulietaUla/Montserrat` | SIL Open Font License 1.1 | 13/10/2026 |
| Instrument Sans (variable, graisses 400 à 700) | Google Fonts, dépôt `github.com/google/fonts/tree/main/ofl/instrumentsans` ; projet d'origine `github.com/Instrument/instrument-sans` | SIL Open Font License 1.1 | 13/10/2026 |

La licence OFL 1.1 permet l'usage commercial, l'intégration dans une application ou un site, l'impression, et la vectorisation du nom dans un logo. Elle interdit de vendre la police seule et de réutiliser son nom pour une version modifiée.

## Polices de remplacement

- Titres : `Montserrat, 'Segoe UI', Arial, sans-serif`
- Textes : `'Instrument Sans', system-ui, -apple-system, 'Segoe UI', Roboto, Arial, sans-serif`
- Les applications **embarquent** les polices : auto-hébergement des fichiers WOFF2 conseillé plutôt qu'un appel à Google Fonts. C'est plus rapide, et cela respecte mieux le RGPD, puisque l'appel à un serveur tiers transmet l'adresse IP du visiteur.
- Courriels et signatures : **Arial**. Bureautique sans les polices de la marque : **Arial** ou **Segoe UI**.

## Hiérarchie

| Style | Police | Taille / interligne | Graisse | Usage |
|---|---|---|---|---|
| `display` | Montserrat | 42 / 48 px | 700 | Accroche de page produit, couverture, diapositive de titre |
| `titre-1` | Montserrat | 26 / 32 px | 700 | Titre d'écran (un seul par écran) |
| `titre-2` | Montserrat | 20 / 26 px | 600 | Titre de section |
| `titre-3` | Montserrat | 16 / 22 px | 600 | Titre de carte, nom de véhicule |
| `chiffre-xl` | Montserrat | 32 / 38 px | 700, chiffres tabulaires | Indicateurs du tableau de bord |
| `chiffre` | Montserrat | 20 / 24 px | 600, chiffres tabulaires | Chiffres dans les cartes et tableaux |
| `corps` | Instrument Sans | 15 / 22 px | 400 | Texte courant |
| `corps-fort` | Instrument Sans | 15 / 22 px | 600 | Mise en avant, libellés |
| `petit` | Instrument Sans | 13 / 18 px | 400 | Aide, métadonnées |
| `legende` | Instrument Sans | 12 / 16 px | 500, +0,04 em | Immatriculations, légendes |
| `surtitre` | Instrument Sans | 11 / 16 px | 700, capitales, +0,08 em | Surtitres, en-têtes de colonnes |

Chaque style existe en classe CSS (`.titre-1`, `.corps`…) dans `tokens.css`.

## Règles

- Montserrat ne descend pas sous 14 px : en dessous, utiliser Instrument Sans.
- Deux graisses de titre au plus sur un même écran ; pas d'italique dans l'interface.
- Chiffres alignés (tabulaires) dans les tableaux et indicateurs.
- Lignes de 45 à 75 caractères pour le texte courant ; aligné à gauche, jamais justifié.
- Texte courant jamais sous 13 px à l'écran, ni sous 7 pt à l'impression.
- Impression : `display` 28 à 36 pt, `titre-1` 18 pt, `corps` 9,5 à 10,5 pt.
