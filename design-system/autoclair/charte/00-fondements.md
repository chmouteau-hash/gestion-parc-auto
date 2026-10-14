Autoclair apporte de la clarté dans la gestion automobile : retrouver ses informations, comprendre ce qui demande une action et anticiper les échéances. Le logo est un **A en forme de route** : la route vue de face, qui file vers l'horizon, avec un sommet **cyan** là où le regard se porte. Toute la charte en découle : palette marine et cyan, route, barre de trajet, borne. Sections : Logo, Couleurs, Typographie, Langage graphique, Interface, Modèles, Transmission.

## Fondements de marque

**Présentation courte.** Autoclair aide les particuliers à suivre les véhicules de leur foyer : documents, entretien, échéances. Autoclair Pro aide les entreprises à gérer leur parc : véhicules, conducteurs, affectations, interventions, documents et échéances.

**Positionnement.** Autoclair se place du côté de la clarté et de l'anticipation : voir la route devant soi. Chaque écran répond d'abord à une question : qu'est-ce qui demande une action, et quand ?

**Promesse.** Savoir à tout moment où en sont vos véhicules, et ce qu'il faut faire ensuite. Ne jamais l'étayer par un chiffre, un témoignage, une certification ou un partenariat qui ne serait pas démontré et documenté.

**Personnalité.**
- **Claire** : une idée par phrase, une action principale par écran.
- **Fiable** : des dates précises, des statuts constants, aucune exagération.
- **Prévenante** : on prévient avant, on propose l'action utile.
- **Posée** : on signale sans alarmer. Le rouge est réservé à ce qui est vraiment dépassé.

**Ton.** Humain, direct et professionnel. Avec le particulier, Autoclair **accompagne** : vouvoiement chaleureux, explications courtes, rassurance (« Tout est à jour »). Avec le gestionnaire, Autoclair Pro **donne le contrôle** : faits, volumes, priorités, vocabulaire du métier (affectation, intervention, parc).

| | Autoclair (particuliers) | Autoclair Pro (entreprises) |
|---|---|---|
| Accroche commune | Chaque véhicule, son repère. | Chaque véhicule, son repère. |
| Accroche dédiée | Vos véhicules, leurs documents et leurs échéances, au même endroit. | Votre parc automobile, sous contrôle. |
| Bouton | Ajouter un véhicule | Planifier une intervention |
| Alerte | Contrôle technique dans 15 jours. Planifier | 2 échéances dépassées. Voir les véhicules |

## Écriture des noms

- Écrire **Autoclair** en un mot, avec un A majuscule. Jamais « AutoClair », « Auto Clair », « AUTOCLAIR » ni « autoclair » dans un texte.
- Écrire **Autoclair Pro** avec une espace et un P majuscule. Jamais « Autoclair-Pro », « AutoclairPro » ni « Autoclair PRO ».
- Dans une phrase, ne pas ajouter d'article devant le nom : « avec Autoclair », pas « avec l'Autoclair ».
- Le logo s'utilise toujours depuis les fichiers fournis (groupe **Logos**) et ne se recompose jamais en texte.

## Règles d'écriture

- Vouvoyer. Les boutons commencent par un verbe à l'infinitif (« Ajouter un véhicule », « Renouveler »).
- Une alerte dit quoi, quand, pour quel véhicule, puis l'action : « Contrôle technique dans 15 jours. Citadine bleue, le 28/10/2026. Planifier ».
- Dates au format JJ/MM/AAAA (JJ/MM dans les espaces étroits) ; délais en jours (« dans 15 jours », « en retard de 3 jours »).
- Typographie française : espace fine insécable avant « : ; ! ? », guillemets « ». Pas d'emoji.
- N'annoncer que les fonctions disponibles : véhicules, conducteurs, affectations, interventions, échéances, alertes, documents. Le suivi du carburant et des recharges, les rapports financiers et la lecture automatique des documents sont des évolutions envisagées : ils n'apparaissent dans aucun support.
- Maquettes et modèles : données fictives uniquement (noms, immatriculations, sociétés).

## Récapitulatif

| Élément | Règle |
|---|---|
| Couleurs principales | `marine-700` #062966 (logo, marque, liens), `marine-900` #03183D (Pro, fonds sombres), `cyan-400` #1FC2F6 (sommet du A, action principale avec texte marine) |
| Neutres | `encre` #0B1A33 (texte), `ardoise-600` #4A5568 (secondaire), `ligne` #D8DFEA, `fond` #F5F7FB, `blanc` |
| Statuts | `succes` #1F6F45, `attention` #8A4B00, `danger` #B42318, chacun avec son fond `*-fond` ; toujours icône + texte |
| Polices | Montserrat (titres, chiffres, nom du logo) et Instrument Sans (textes), toutes deux SIL OFL 1.1 ; remplacement Segoe UI, Arial |
| Hiérarchie | `display` 42, `titre-1` 26, `titre-2` 20, `titre-3` 16 (Montserrat) ; `corps` 15, `petit` 13, `legende` 12, `surtitre` 11 (Instrument Sans) |
| Espacements | grille de 4 px : `space-1` à `space-12` ; gouttière mobile `space-4` (16 px) |
| Angles | `radius-sm` 8 (champs), `radius-md` 12 (cartes, boutons), `radius-pill` (badges, libellé Pro) |
| Ombres | aucune au repos ; `ombre-1` survol, `ombre-2` menus, `ombre-3` modales |
| Contraste | texte ≥ 4,5:1, icônes et bordures de champs ≥ 3:1 ; le cyan n'est jamais un texte sur fond clair (2,1:1) |
| Distinction Pro | icône inversée (A blanc sur fond marine), libellé « Pro » marine sur pastille cyan |
| Motifs | le A-route en grand et rogné ; borne (compte à rebours, mobile) ; barre de trajet (tableaux, fiches) ; fiche à onglet daté (documents) |
| Composants | `components/bundle.css`, classes `ac-*` ; une seule action cyan par écran |

## Réserves

- **Origine du dessin.** Le logo provient d'une image générée par une IA, que nous avons redessinée en vectoriel avec des ajustements : arrondis, icône Pro, version simplifiée, libellé Pro. En France, la protection par le droit d'auteur d'un visuel généré par IA est incertaine. Il faut :
  - conserver les fichiers sources et l'historique des retouches humaines ;
  - vérifier les conditions d'utilisation de l'outil employé ;
  - envisager le dépôt du logo comme **marque figurative**, qui protège indépendamment du droit d'auteur.
- **Nom et logo non vérifiés juridiquement.** Le motif « A + route » est fréquent (auto-écoles, transporteurs), ce qui augmente le risque de ressemblance. Une recherche d'antériorité est indispensable avant tout usage public ou dépôt : base INPI et EUIPO (TMview), classes 9 et 42, idéalement avec un conseil en propriété industrielle. Une recherche web du 13/10/2026 n'a relevé qu'une marque proche par le nom, « AUTOKLER » (n° 1 329 706, produits alimentaires) ; elle ne remplace pas cette recherche.
- **Couleurs d'impression.** Les valeurs CMJN sont indicatives : à valider sur épreuve.
- **Archives.** Les versions précédentes (direction « Repère » à anneau, et l'exploration des trois directions) sont conservées dans `archived/`.
