# Gestion du parc automobile

Application web interne pour suivre le parc automobile d'une entreprise : les véhicules, qui les
conduit, leur entretien et leurs échéances réglementaires (contrôle technique, assurance,
entretien périodique, permis de conduire). Elle s'adresse à un gestionnaire de parc non
technicien : des listes, des fiches, des formulaires, et une vue « Alertes » qui dit ce qui est
dépassé ou arrive dans les 30 jours.

Elle est écrite en Rust (serveur Axum, base PostgreSQL via SQLx, pages HTML rendues côté serveur
par Askama). Le code et la documentation sont en français.

## Ce que fait l'application

| Écran | Contenu |
| --- | --- |
| **Accueil** | La page d'arrivée, selon le profil. Particulier : « une chose à faire » ou « tout est à jour », puis une carte par véhicule avec sa prochaine échéance et les jours restants. Entreprise : tableau de bord chiffré (dépassées, à venir, en maintenance, permis expirés, en service), la liste par urgence, les véhicules sans conducteur, les dernières interventions. |
| **Alertes** | Échéances dépassées ou à moins de 30 jours, permis de conduire qui expirent, et opérations du plan d'entretien dont la borne est passée ou à moins de 1 000 km ou de 30 jours (« Vidange dans 800 km ») ; rangées par urgence, la part de l'horizon qui reste ; lien vers la fiche concernée et, pour une échéance, « Marquer traitée » sans quitter la page, et pour une opération du plan, « C'est fait ». Les deux horizons se règlent par `ALERTES_HORIZON_JOURS` et `ALERTES_HORIZON_KM`. |
| **Véhicules** | Liste, fiche (statut, numéro d'identification (VIN), couleur de repère, vignette Crit'Air, conducteur actuel, échéances, entretien, historique, documents), création (avec la photo du véhicule, si on en a une sous la main), modification, suppression. Les véhicules réformés sont repliés sous la liste (« Réformés (N) »), où l'administrateur peut les supprimer définitivement ou les remettre en service. La **vignette Crit'Air** (E, 1 à 5, ou non classé) se choisit d'après le certificat qualité de l'air ; tant qu'aucune n'est choisie, le formulaire coche celle que donnent le carburant et la date de mise en service, et la fiche la propose « à vérifier sur le certificat ». Elle s'affiche en pastille aux couleurs officielles sur la fiche et dans la liste, et figure dans l'export et l'API. Le certificat se dépose dans le dossier « Administratif ». L'application dit la vignette, pas les règles de chaque zone à faibles émissions. |
| **Conducteurs** | Liste, fiche (affectations, documents), création, modification, suppression. Créer un conducteur de même nom et prénom qu'un autre avertit (casse, accents et trait d'union ignorés) puis laisse « Créer quand même » : deux personnes peuvent porter le même nom. |
| **Affectations** | Qui conduit quoi, depuis quand ; clôture en un clic. Un véhicule peut avoir plusieurs conducteurs, un conducteur plusieurs véhicules. Un véhicule réformé, un conducteur inactif ou au permis expiré ne peut pas être affecté. |
| **Entretien** | Interventions (révision, entretien courant, réparation, pneus…) avec date, kilométrage, coût, prestataire, et **ce qui a été fait** : des cases à cocher, pièces et consommables (vidange, filtre à huile, bougies, courroie de distribution, plaquettes…) puis contrôles et réglages (pression des pneus, niveaux, géométrie…), ou « autre », précisé, chacune avec, si on veut, la référence de la pièce, la quantité (décimale au besoin : 5,4 L d'huile) et le prix, pour les retrouver la prochaine fois. La fiche et la liste les résument (« Vidange · Filtre à huile (réf. …, 12,50 €) »). Un **contrôle technique** porte son résultat : favorable, ou défavorable avec la contre-visite à faire sous deux mois (date affichée) ; la **contre-visite** se saisit comme une intervention, rattachée au contrôle qu'elle suit. **La facture, ou le procès-verbal du contrôle technique,** se joint dans la foulée (photo ou PDF ; le type se choisit, et celui qui va avec l'intervention est proposé d'office) : le document est relié à l'intervention, et la ligne de l'intervention y mène. Le kilométrage du véhicule suit, jamais en arrière ; un saut de plus de 20 000 km au-dessus du compteur demande confirmation avant l'envoi (sur un plein aussi), et l'aide dit comment corriger une erreur. Sur un téléphone tenu en paysage, la liste masque les colonnes « Opérations » et « Prestataire » pour tenir dans l'écran ; la fiche du véhicule et le CSV les gardent. Chaque véhicule a son **plan d'entretien** : les opérations suivies (vidange, filtres, pression des pneus…), tous les combien de kilomètres ou de mois, la dernière fois, calculée à partir des interventions, et la prochaine borne, avec la part du trajet déjà faite. Une opération jamais notée n'est pas « en retard » : la fiche dit que sa dernière fois est inconnue. Une intervention datée dans le futur ne compte pas comme faite, et le formulaire le dit avant l'envoi. Une opération à moins de 1 000 km ou de 30 jours de sa borne est « à venir » : elle rejoint alors les alertes et l'accueil. Le kilométrage croît avec la date : une intervention plus haute qu'une plus récente, ou plus basse qu'une plus ancienne, demande « Enregistrer quand même » ; le kilométrage pré-rempli depuis la fiche s'efface quand on recule la date. **« C'est fait »**, sur une opération du plan (fiche, alertes, tableau de bord, carte d'accueil), ouvre le formulaire d'intervention pré-rempli : le véhicule, son compteur, la date du jour, le type (« Révision » pour une pièce, « Entretien courant » pour un contrôle) et l'opération cochée ; enregistrer suffit, la prochaine se recalcule. « Annuler » ramène à la page d'où l'on vient. **Le plan de départ** : sur un véhicule sans plan, « Proposer un plan » ouvre les opérations courantes pour son carburant, avec leur périodicité habituelle (vidange tous les 15 000 km ou 12 mois…), cochées et modifiables ; un électrique n'a ni vidange ni bougies, un diesel a un filtre à carburant, l'hybride et le GPL se suivent comme l'essence. La courroie de distribution et la permutation des pneus sont proposées sans être cochées : elles dépendent du véhicule. Ce sont des valeurs de départ, pas celles du constructeur : le carnet d'entretien prime. Rien n'est noté comme fait : le plan compte depuis la mise en service. |
| **Dépenses** | Ce que l'on paie pour rouler, véhicule par véhicule : les **pleins de carburant** (date, montant, carburant pris ce jour-là : SP95, SP95-E10, SP98, E85, gazole, GPL, GNV, électricité, proposé d'après le véhicule et modifiable ; quantité en litres, en kWh ou en kg selon ce carburant ; kilométrage au compteur, plein complet ou partiel, station) et les **tickets de péage** (date, montant, trajet). Un seul écran, un type à choisir ; le formulaire ne montre le détail du plein que pour le carburant. Un plein noté avec son compteur relève le kilométrage du véhicule, comme une intervention. La fiche du véhicule montre les dix dernières dépenses, avec « Noter un plein » et « Noter un péage », et renvoie à la liste pour le reste (`/depenses?vehicule_id=…`). Le ticket (photo ou PDF) se joint depuis le formulaire, comme la facture d'une intervention : il devient un document « Ticket de carburant ou de péage » du véhicule, rangé dans le dossier « Dépenses » de sa fiche, relié à la dépense (sa page la nomme) et conservé si elle est supprimée. **Ce que ça consomme et ce que ça coûte se calcule, rien n'est stocké** : un plein complet avec kilométrage clôt un intervalle entre deux pleins complets, et sa ligne affiche la consommation aux 100 km (L, kWh ou kg selon le carburant, jamais mélangés ; les pleins partiels et sans kilométrage comptent dans les quantités sans faire borne ; un intervalle dont le kilométrage ne croît pas ou dont une quantité manque n'a pas de consommation). Un seul bloc « Coût » sur la fiche, pour l'entretien et les dépenses, résume l'année en cours et les douze derniers mois (l'entretien, avec la part des pièces et le reste quand des prix de lignes sont saisis ; le carburant et les péages séparés), la consommation moyenne et le coût au kilomètre des douze derniers mois, entretien compris. Une intervention coûte sa facture, ou la somme des prix de ses lignes « ce qui a été fait » si elle est plus grande. La liste se filtre par véhicule et par période (« Ce mois », « Cette année », « Douze derniers mois », ou deux dates), additionne ce qui est affiché, et son export CSV (`/depenses.csv?vehicule_id=…&du=…&au=…`) est celui du filtre ; l'API accepte les mêmes paramètres et rend le résumé d'un véhicule (`/api/v1/vehicules/{id}/depenses/resume`). Le tableau de bord d'Autoclair Pro montre un chiffre « Dépenses du mois » qui ouvre la liste du mois. Dans Autoclair Pro, « Dépenses » est dans la barre latérale ; dans Autoclair, derrière « Plus ». Le coût s'obtient aussi par l'API (`/api/v1/vehicules/{id}/depenses/resume`, avec l'entretien et les pièces). |
| **Échéances** | Contrôle technique, assurance, entretien périodique : date limite ; statut « à traiter », « à venir » dans les 30 jours qui précèdent la date limite, « dépassée » le lendemain de la date limite, ou « traitée ». « Marquer traitée » propose aussitôt la suivante, pré-remplie : l'assurance un an après la date limite (l'anniversaire du contrat) ; le contrôle technique deux ans et l'entretien un an après la date limite ou, si elle est passée, après le jour même, pour ne jamais proposer une date déjà dépassée. La date proposée se corrige avant d'enregistrer. Une échéance **« À faire »** note ce qu'il y a à remplacer ou à faire une fois (balais usés, câble d'embrayage qui accroche) : on choisit dans la liste des opérations de l'entretien, ou « Autre » avec son libellé, on dit pourquoi dans le constat, et on fixe la date (proposée dans un mois). Elle s'affiche partout comme une échéance (« À faire : Câble d'embrayage », le constat dessous), rappelle comme elle, n'a pas de suivante une fois traitée. On la note depuis la fiche du véhicule (« Noter ce qu'il y a à faire ») ou depuis « Nouvelle intervention ». Sur un véhicule qui a un plan d'entretien, le plan remplace l'« Entretien périodique » : il ne compte plus dans les alertes ni sur l'accueil, la fiche le dit « remplacée par le plan d'entretien », et il ne s'en crée plus. **Enregistrer l'intervention solde l'échéance** qu'elle remplit : un contrôle technique favorable solde le « Contrôle technique » ouvert, une révision l'« Entretien périodique » (pas sur un véhicule qui a un plan d'entretien) ; le formulaire de la suivante s'ouvre aussitôt, daté de la visite (deux ans après pour le contrôle, un an pour la révision). Un contrôle défavorable laisse l'échéance ouverte : le véhicule n'est pas en règle. Une intervention ne solde que l'échéance de sa période, faite dans les deux ans (un an pour la révision) qui précèdent la date limite, ou après : un contrôle de 2022 saisi pour mémoire ne touche pas à l'échéance de 2026. **La contre-visite** : un contrôle technique défavorable crée d'office l'échéance « Contre-visite », à faire dans les deux mois (pas pour un contrôle saisi après ce délai), une seule à la fois par véhicule. Tant qu'elle est à faire, elle remplace l'échéance « Contrôle technique » : celle-ci reste en base, la fiche la dit « remplacée par la contre-visite », elle ne compte ni dans les alertes ni sur l'accueil. Après un contrôle « circulation interdite », l'alerte de la contre-visite le dit. Une contre-visite favorable faite dans les deux mois solde les deux échéances et propose le prochain contrôle, deux ans après le contrôle **initial** ; défavorable ou trop tardive, elle se solde et l'application dit qu'un contrôle complet est à refaire. Un contrôle complet favorable solde aussi une contre-visite encore ouverte. « Marquer traitée » ne propose pas de suivante pour une contre-visite. **Les défauts relevés au contrôle** : sur la fiche, la contre-visite à faire propose « Noter un défaut à corriger », qui ouvre une échéance « À faire » datée de la contre-visite, le contrôle nommé dans le constat ; on choisit la pièce. **Une chose à faire se solde** quand on enregistre une intervention qui contient la même opération, faite depuis le jour où la chose a été notée (pas pour « Autre », dont le libellé est libre) ; le message le dit, sauf quand l'intervention solde aussi une autre échéance, dont le message passe en premier. |
| **Documents** | Fichiers (10 Mo max) rattachés à un véhicule ou un conducteur, avec leur type (photo du véhicule, facture, contrôle technique, carte grise, assurance, facture d'achat, certificat de cession, garantie, ticket de carburant ou de péage, permis…), stockés sur disque. Sur la fiche du véhicule, ils se rangent en dossiers déduits du type : Achat, Administratif, Entretien, Dépenses, Autres ; chaque dossier a son bouton « Ajouter », qui ne propose que ses types. Une facture jointe depuis le formulaire d'intervention reste reliée à cette intervention, un ticket joint depuis le formulaire de dépense à cette dépense : leur page les nomme. La photo du véhicule choisie (« Mettre en tête », dans le bloc « Photos » de la fiche, à partir de deux photos), à défaut la plus récente, sert de vignette (calculée par le serveur) à sa fiche et à sa carte d'accueil. Chaque document a sa page : une image s'y affiche et s'imprime, un PDF s'ouvre dans le lecteur du navigateur, tout fichier se télécharge, et l'administrateur peut l'y renommer (un nom qui parle à la place de celui de l'appareil photo : seule la racine se saisit, l'extension du fichier reste) ou l'y supprimer. Les fiches, les listes d'échéances et d'interventions et les alertes ont un bouton « Imprimer » : le papier ne garde ni menu, ni boutons. |
| **Déposer un document** | Pensée pour le téléphone ou la tablette : on choisit le véhicule et le type, on prend la photo, elle est enregistrée sans autre geste. |
| **Comptes** | Pour l'administrateur : la liste des comptes, « Nouveau compte » (identifiant, rôle, mot de passe initial), « Désactiver » ou « Réactiver » (un compte désactivé ne peut plus se connecter et sa session s'arrête ; il n'est pas supprimé ; ni son propre compte ni le dernier administrateur), « Nouveau mot de passe » pour qui l'a oublié. Chacun change le sien dans « Mon compte ». |

Deux rôles : **administrateur** (tout) et **lecteur** (consultation seule). Une API JSON
(`/api/v1/…`) expose les mêmes opérations pour les scripts.

Les gestes du quotidien : le kilométrage se relève en un geste depuis la carte d'accueil ou la
fiche, et chaque relevé entre dans l'**historique du compteur** (lien « historique » à côté du
kilométrage, sur la fiche) : le compteur est le plus haut kilométrage des relevés, des interventions
et des pleins, et il redescend quand on corrige ou supprime la saisie fautive. Le kilométrage se
saisit à la création du véhicule, puis ne se change plus en le modifiant. Le **rythme** du véhicule
se mesure sur les trois derniers mois de cet historique (« environ 1 520 km par mois ») : les bornes
en kilomètres du plan d'entretien en tirent une date (« Vidange · dans 1 800 km, vers mi-janvier au
rythme actuel »). Un compteur qui n'a pas été relevé depuis plus d'un mois (ni relevé, ni
intervention, ni plein) s'affiche dans « Compteurs à relever », sous les alertes, et sur la carte
d'Autoclair ; il ne compte pas dans les alertes dépassées ou à venir. Une intervention qui contredit
l'historique demande confirmation, avec la fourchette : « le 10/03/2024 ce véhicule était entre
88 000 et 101 000 km » ; chaque action est confirmée par un message en tête de la page suivante ; une suppression
demande confirmation dans une boîte de l'application ; un formulaire déjà envoyé ne repart pas
une seconde fois (un second appui sur « Enregistrer », sur un téléphone en réseau lent, est
ignoré et le bouton s'éteint) ; chaque liste s'exporte en CSV (lien
« Exporter » chez une entreprise, masqué sur un téléphone ; adresses `/vehicules.csv`,
`/conducteurs.csv`, `/affectations.csv`, `/interventions.csv`, `/echeances.csv`,
`/depenses.csv`).

Deux profils d'installation, choisis par la variable `PROFIL` : **entreprise** (par défaut) montre
tout ce qui précède ; **particulier**, pour qui suit ses propres véhicules, masque l'écran
« Affectations », les blocs d'affectations des fiches et le service des conducteurs.

Chaque profil a sa navigation. **Autoclair Pro** (entreprise) : une barre latérale avec le menu
complet, une recherche (immatriculation, marque, modèle, VIN, nom d'un conducteur ; touche `/`
pour y aller) et le compte connecté, repliée derrière un bouton « Menu » sur un écran étroit.
**Autoclair** (particulier) : quatre onglets, Véhicules · Échéances · Entretien · Plus, en bas du
téléphone ; « Plus » rassemble le reste (alertes, dépenses, conducteurs, dépôt d'un document, recherche,
déconnexion). Sur le téléphone, chaque marque s'installe sur l'écran d'accueil comme une
application, avec son icône et en plein écran (`docs/exploitation.md`, « Depuis un téléphone ou
une tablette »).

## Lancer l'application en local

Prérequis : [Rust](https://rustup.rs) stable (1.89 ou plus) et PostgreSQL 16 ou plus
(par [Docker](https://docs.docker.com/get-docker/) ou installé sur la machine).

1. Démarrer PostgreSQL : `docker compose up -d` (le mot de passe vient de `POSTGRES_PASSWORD`
   dans `.env`, voir l'étape 2). Sans Docker : créer une base `gestion_parc_auto` sur votre
   PostgreSQL et adapter `DATABASE_URL`.
2. Copier `.env.example` vers `.env` et renseigner les valeurs (au minimum `DATABASE_URL` et
   `SESSION_SECRET`, une chaîne aléatoire de 64 caractères ou plus). `.env` n'est jamais versionné.
3. Appliquer les migrations : `cargo run -- migrer` (ou `sqlx migrate run` si sqlx-cli est
   installé). `cargo run` les applique aussi au démarrage : cette étape vérifie surtout la
   connexion à la base.
4. Créer le premier compte : `cargo run -- creer-utilisateur admin administrateur`, puis taper un
   mot de passe d'au moins 12 caractères.
5. Lancer le serveur : `cargo run`, puis ouvrir <http://localhost:3000> (ou le `PORT` choisi).

La première compilation prend quelques minutes (téléchargement et compilation des dépendances) ;
les suivantes sont rapides.

## Vérifier le projet

```bash
cargo fmt --check                               # formatage
cargo clippy --all-targets -- -D warnings       # linter
cargo test --lib                                # tests unitaires (sans base)
cargo test                                      # tous les tests ; DATABASE_URL doit pointer vers une base de TEST
cargo deny check                                # bibliothèques : failles connues et licences (deny.toml)
```

`cargo deny` ne vient pas avec Rust : l'installer une fois par `cargo install cargo-deny`.

Les tests d'intégration créent une base jetable par test à partir de `DATABASE_URL` : utiliser
une base dédiée (par exemple `gestion_parc_auto_test`) dont l'utilisateur a le droit `CREATEDB`.
Sous PowerShell : `$env:DATABASE_URL="postgres://parc:…@localhost:5432/gestion_parc_auto_test"; cargo test`.

La CI (`.github/workflows/ci.yml`) joue sur chaque pull request et sur `main` : formatage,
clippy, migrations sur base vide (deux fois), tests et compilation release ; la compilation avec
la version minimale de Rust annoncée (`rust-version` dans `Cargo.toml`) ; le contrôle des
bibliothèques (`cargo deny check`, puis `npm audit` pour le catalogue) ; la construction du
catalogue d'interface. Elle rejoue le tout chaque lundi : une faille publiée sur une
bibliothèque fait alors échouer la CI sans que le code ait changé.

Dependabot, le robot de GitHub (`.github/dependabot.yml`), propose une fois par mois les mises à
jour des bibliothèques Rust, de celles du catalogue et des actions de la CI : une pull request
par famille, contrôlée par la CI comme les autres.

Une étiquette de version (`v0.2.0`) poussée sur GitHub déclenche la livraison
(`.github/workflows/livraison.yml`) : l'exécutable est construit pour Linux et pour Windows,
puis attaché à une release. Voir [docs/exploitation.md](docs/exploitation.md), « Publier une
version ».

## Catalogue d'interface (Storybook)

[Storybook](https://storybook.js.org) affiche chaque élément d'interface isolément, dans chacun
de ses états, sans lancer le serveur ni la base : boutons, pastilles, tableaux, fiches,
formulaires, puis quelques écrans complets. Chaque cas présenté s'appelle une *story*. C'est un
outil de développement, rangé dans `storybook/` : l'application livrée n'en contient rien.

Prérequis : [Node.js](https://nodejs.org) 22.12 ou plus (ou 20.19 ou plus).

```bash
cd storybook
npm install               # la première fois : télécharge Storybook dans storybook/node_modules
npm run storybook         # ouvre http://localhost:6017
npm run build-storybook   # galerie à partager, dans storybook/storybook-static (non versionnée)
```

Dans la barre du haut, le bouton de taille d'écran passe d'une largeur à l'autre (téléphone,
tablette, bureau). L'onglet « Controls » change le texte, le rôle (administrateur ou lecteur) ou le profil
(entreprise ou particulier).
L'onglet « Actions » montre le lien ou le formulaire qui serait parti : rien n'est envoyé.
Comme l'application, le catalogue n'écoute que sur ce poste (`--host 127.0.0.1` dans
`storybook/package.json`) : sans cette option, Storybook répond sur toutes les cartes réseau.
Il ne demande rien non plus à son éditeur : ni statistiques d'usage, ni recherche de mise à jour
(`--no-version-updates`), ni page des nouveautés (refusée dans `storybook/.storybook/main.js`,
faute d'option pour la couper). `tests/catalogue.rs` fait échouer la CI si `--host 127.0.0.1`,
`--no-version-updates` ou la coupure des statistiques disparaît ; il se lance seul, sans base,
par `cargo test --test catalogue`.

Storybook ne sait pas exécuter les gabarits Askama. Ses stories contiennent donc du **HTML
recopié** de `templates/`, habillé par la vraie feuille de style et animé par le vrai script de
`static/`. Conséquence : une modification de `static/` se voit aussitôt dans le catalogue, mais
**un gabarit modifié doit être reporté à la main** dans sa copie. Chaque copie nomme son gabarit
d'origine : les morceaux partagés sont dans `storybook/stories/fragments.js`, les écrans dans
`storybook/stories/ecrans/`.

Écrans présents : connexion, accueil (les deux profils), alertes, véhicules (liste, fiche, formulaire), affectations (liste),
dépôt d'un document, page d'erreur. Les écrans des conducteurs, de l'entretien et des échéances,
ainsi que le formulaire d'affectation, n'y sont pas : ils reprennent les mêmes constructions.

## Structure du dépôt

```
src/
  main.rs       point d'entrée et sous-commandes (migrer, creer-utilisateur, changer-mot-de-passe,
                purger-conducteurs)
  web/          routes HTTP, session, pages et API JSON — aucune requête SQL ici
  metier/       validation des saisies et règles de gestion — ne connaît ni HTTP ni SQL
  donnees/      requêtes SQL paramétrées, une table par fichier
  commun/       vocabulaires (statuts, types) et mise en forme, partagés par les trois couches
templates/      pages HTML (Askama, compilées avec le programme) ; macros.html pour les morceaux
                partagés (écran vide, plaque, barre de trajet)
static/         feuille de style, script (confirmations, taille et envoi des fichiers à déposer,
                intitulés des tableaux sur écran étroit) et polices WOFF2 (polices/)
migrations/     schéma PostgreSQL, fichiers SQL horodatés
tests/          tests d'intégration sur base réelle ; catalogue.rs garde les réglages de Storybook
scripts/        sauvegarde de la base et des documents (sauvegarder.sh, sauvegarder.ps1) et tâche
                planifiée Windows (installer-sauvegarde-windows.ps1) ; voir docs/exploitation.md, § 5
storybook/      catalogue d'interface (Storybook) : outil de développement, hors de l'application
docs/           architecture.md, base-de-donnees.md, exploitation.md
design-system/  charte Autoclair (autoclair/ : jetons, logos, textes de la charte) et CHARTE.md,
                qui dit comment l'application l'applique et où elle s'en écarte
```

La dépendance va toujours dans le même sens : `web → metier → donnees`. Le détail et les choix
techniques sont dans [docs/architecture.md](docs/architecture.md) ; le schéma dans
[docs/base-de-donnees.md](docs/base-de-donnees.md) ; lancement, sauvegarde et RGPD dans
[docs/exploitation.md](docs/exploitation.md).

Les règles de travail sur les issues (modèles, cases à cocher, étiquette `à-tester`) sont dans
[AGENTS.md](AGENTS.md).

## Hypothèses retenues

- Rust + Axum + SQLx + Askama, PostgreSQL 16+, pas de framework JavaScript (seul un petit
  script : confirmation avant suppression, contrôle de la taille d'un fichier avant son envoi,
  envoi d'un document dès qu'il est choisi, intitulés des tableaux sur écran étroit, fiches pour un
  tableau trop large, ouverture d'une section repliée que vise l'adresse).
- Le catalogue d'interface (Storybook, dossier `storybook/`) est un outil de développement à
  part, avec ses propres dépendances Node : l'application n'en dépend pas et se compile sans lui.
  Ses stories recopient le HTML des gabarits (décidé le 12/10/2026).
- Sous 700 px de large (téléphone, panneau étroit), chaque ligne de tableau s'affiche comme une
  fiche : une information par ligne, les actions en dessous, rien à faire défiler de côté
  (décidé le 12/10/2026). Au-dessus, un tableau qui ne tient pas dans son cadre (texte agrandi,
  tablette, fenêtre réduite) se présente aussi en fiches, rangées sur plusieurs colonnes quand la
  place le permet ; un tableau qui tient reste un tableau (décidé le 15/10/2026, #83). Sur un
  écran de moins de 500 px de haut, un téléphone en paysage, des fiches de huit lignes ne se
  voient jamais en entier : il garde son tableau, qui défile dans son cadre, avec sa colonne
  d'actions collée au bord droit (#78). Une fiche trop étroite pour qu'une valeur tienne à côté de
  son intitulé (texte très agrandi, zoom de page) pose chaque intitulé au-dessus de sa valeur
  (#73). Jusqu'au zoom de page de 200 %, aucune page ne défile de côté : les plaques rétrécissent
  quand la place manque, et l'icône d'Autoclair Pro remplace son logo quand il passerait sous
  24 px de haut, comme le veut la charte (#145).
- La photo d'un document passe par le champ de fichier du navigateur, qui ouvre l'appareil photo
  du téléphone (décidé le 11/10/2026). Chaque document porte un type, choisi au dépôt, et se
  range sur la fiche du véhicule ou du conducteur. L'application ne lit pas le contenu du
  document : une ligne d'entretien ou une échéance se saisit toujours à la main. Seul geste
  automatique : une intervention enregistrée solde l'échéance qu'elle remplit (un contrôle
  technique favorable, une révision ; #94, bloc 3), et propose la suivante sans l'enregistrer.
- Les requêtes SQL sont vérifiées **à l'exécution** par les tests d'intégration, pas à la
  compilation par les macros `query!` (voir « Décisions à valider »).
- Une affectation est « en cours » quand la date du jour tombe dans sa période (décidé le
  11/10/2026) : à partir de son jour de début, et jusqu'à la veille de son jour de fin. Le jour
  de fin est celui où elle s'arrête. Sans date de fin, elle reste en cours. Avant son début elle
  est « à venir », ensuite « terminée ». L'état se calcule à l'affichage, il n'est pas stocké.
- Règles d'affectation (décidées le 11/10/2026) : un véhicule peut avoir plusieurs conducteurs
  en même temps, et un conducteur plusieurs véhicules. On ne peut pas affecter un véhicule
  réformé, ni un conducteur inactif ou dont le permis est expiré. Le même conducteur ne peut
  pas avoir, sur le même véhicule, deux affectations dont les périodes se chevauchent. Clôturer
  ou corriger une affectation existante reste toujours possible.
- Clôture automatique (décidée le 11/10/2026) : passer un véhicule à « Réformé » ou un conducteur
  à « Inactif » met fin, à la date du jour, à ses affectations encore ouvertes. L'historique est
  conservé. Réactiver ensuite ne rouvre rien.
- Profil d'installation (décidé le 12/10/2026) : une installation sert un seul parc, celui d'une
  entreprise ou celui d'un particulier. `PROFIL=particulier` masque les affectations et le
  service des conducteurs ; il ne supprime aucune donnée et n'interdit aucune adresse. Sans la
  variable, l'installation est celle d'une entreprise, comme avant. Servir une entreprise et un
  particulier, c'est lancer deux installations, chacune avec sa base, son dossier de documents
  et sa clé de session (voir [docs/exploitation.md](docs/exploitation.md)).
- Les montants sont stockés en centimes entiers ; la saisie se fait en euros (`149,90`).
- Le premier compte se crée en ligne de commande ; pas d'inscription. Ensuite l'administrateur
  gère les comptes depuis « Comptes » (créer, désactiver, nouveau mot de passe) et chacun change le
  sien dans « Mon compte ». Si plus aucun administrateur ne peut se connecter :
  `cargo run -- changer-mot-de-passe LOGIN` sur la machine du serveur.
- Le serveur écoute sur `127.0.0.1` uniquement ; un serveur web en HTTPS est à placer devant
  pour l'exposer à d'autres postes, ou à un téléphone.
- Le dépôt est public depuis le 12/10/2026, pour pouvoir être relu : tout ce qui y est versionné
  est lisible par tous, texte des pull requests et des issues compris. Aucun secret ni document
  réel n'y est versionné (`.env`, `documents/`, `sauvegardes/` sont exclus).

## Décisions à valider

1. **Vérification des requêtes SQL à la compilation ?** Le cahier des charges évoquait les macros
   `query!` de SQLx. Elles exigent une base accessible à chaque compilation (ou un cache `.sqlx`
   à régénérer après chaque requête modifiée), ce qui complique le quotidien d'un débutant et la
   CI. La v1 utilise `query_as` (vérifié à l'exécution) et compte sur les tests d'intégration.
   À confirmer ou à inverser.
2. **Durée de conservation des conducteurs inactifs.** 365 jours par défaut, purge par commande
   manuelle (`purger-conducteurs`). Valider la durée et décider si la purge doit être automatique.
3. **Suppression d'un véhicule ou d'un conducteur.** Elle emporte tout l'historique (cascade). À
   garder (correction d'erreur de saisie) ou à remplacer par une interdiction tant qu'il reste de
   l'historique ?
4. **Mot de passe en clair à la saisie dans le terminal.** La commande `creer-utilisateur` affiche
   ce qu'on tape. Une saisie masquée demande une dépendance supplémentaire (`rpassword`).
5. **Stockage des documents.** Sur le disque de la machine, sous `DOCUMENTS_DIR`. Si l'application
   doit tourner sur plusieurs machines ou être sauvegardée ailleurs, prévoir un stockage partagé.
6. **Permis expiré et affectations en cours.** Un permis expire avec le temps, sans que personne
   ne modifie la fiche : rien ne clôture alors les affectations en cours du conducteur. La vue
   Alertes signale le permis expiré, et l'affectation reste affichée jusqu'à ce qu'on la termine
   à la main. Les clôturer automatiquement (il faudrait une tâche planifiée) ou laisser le
   gestionnaire décider ?

## Évolutions possibles (hors périmètre v1)

Listées pour mémoire, non engagées : facturation et refacturation interne, géolocalisation et
télématique embarquée, application mobile, rappels par courriel des alertes, changement de mot
de passe depuis l'interface, import en masse de véhicules, gestion des contraventions, lecture
automatique d'une facture photographiée (date, montant, garage), réduction des photos avant leur
envoi.
