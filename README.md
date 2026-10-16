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
| **Alertes** | Échéances dépassées ou à moins de 30 jours, permis de conduire qui expirent ; lien vers la fiche concernée et, pour une échéance, « Marquer traitée » sans quitter la page. |
| **Véhicules** | Liste, fiche (statut, numéro d'identification (VIN), conducteur actuel, échéances, entretien, historique, documents), création (avec la photo du véhicule, si on en a une sous la main), modification, suppression. Les véhicules réformés sont repliés sous la liste (« Réformés (N) »), où l'administrateur peut les supprimer définitivement ou les remettre en service. |
| **Conducteurs** | Liste, fiche (affectations, documents), création, modification, suppression. Créer un conducteur de même nom et prénom qu'un autre avertit (casse, accents et trait d'union ignorés) puis laisse « Créer quand même » : deux personnes peuvent porter le même nom. |
| **Affectations** | Qui conduit quoi, depuis quand ; clôture en un clic. Un véhicule peut avoir plusieurs conducteurs, un conducteur plusieurs véhicules. Un véhicule réformé, un conducteur inactif ou au permis expiré ne peut pas être affecté. |
| **Entretien** | Interventions (révision, réparation, pneus…) avec date, kilométrage, coût, prestataire, et **ce qui a été fait** : des cases à cocher (vidange, filtre à huile, bougies, courroie de distribution, plaquettes… ou « autre », précisé), chacune avec, si on veut, la référence de la pièce, la quantité (décimale au besoin : 5,4 L d'huile) et le prix, pour les retrouver la prochaine fois. La fiche et la liste les résument (« Vidange · Filtre à huile (réf. …, 12,50 €) »). Un **contrôle technique** porte son résultat : favorable, ou défavorable avec la contre-visite à faire sous deux mois (date affichée) ; la **contre-visite** se saisit comme une intervention, rattachée au contrôle qu'elle suit. **La facture, ou le procès-verbal du contrôle technique,** se joint dans la foulée (photo ou PDF ; le type se choisit, et celui qui va avec l'intervention est proposé d'office) : le document est relié à l'intervention, et la ligne de l'intervention y mène. Le kilométrage du véhicule suit. |
| **Échéances** | Contrôle technique, assurance, entretien périodique : date limite ; statut « à traiter », « à venir » dans les 30 jours qui précèdent la date limite, « dépassée » le lendemain de la date limite, ou « traitée ». « Marquer traitée » propose aussitôt la suivante, pré-remplie : l'assurance un an après la date limite (l'anniversaire du contrat) ; le contrôle technique deux ans et l'entretien un an après la date limite ou, si elle est passée, après le jour même, pour ne jamais proposer une date déjà dépassée. La date proposée se corrige avant d'enregistrer. |
| **Documents** | Fichiers (10 Mo max) rattachés à un véhicule ou un conducteur, avec leur type (photo du véhicule, facture, contrôle technique, carte grise, assurance, facture d'achat, certificat de cession, garantie, permis…), stockés sur disque. Sur la fiche du véhicule, ils se rangent en dossiers déduits du type : Achat, Administratif, Entretien, Autres ; chaque dossier a son bouton « Ajouter », qui ne propose que ses types. Une facture jointe depuis le formulaire d'intervention reste reliée à cette intervention : sa page la nomme. La photo du véhicule choisie (« Mettre en tête », dans le bloc « Photos » de la fiche, à partir de deux photos), à défaut la plus récente, sert de vignette (calculée par le serveur) à sa fiche et à sa carte d'accueil. Chaque document a sa page : une image s'y affiche et s'imprime, un PDF s'ouvre dans le lecteur du navigateur, tout fichier se télécharge, et l'administrateur peut l'y supprimer. Les fiches, les listes d'échéances et d'interventions et les alertes ont un bouton « Imprimer » : le papier ne garde ni menu, ni boutons. |
| **Déposer un document** | Pensée pour le téléphone ou la tablette : on choisit le véhicule et le type, on prend la photo, elle est enregistrée sans autre geste. |

Deux rôles : **administrateur** (tout) et **lecteur** (consultation seule). Une API JSON
(`/api/v1/…`) expose les mêmes opérations pour les scripts.

Les gestes du quotidien : le kilométrage se relève en un geste depuis la carte d'accueil ou la
fiche ; chaque action est confirmée par un message en tête de la page suivante ; une suppression
demande confirmation dans une boîte de l'application ; chaque liste s'exporte en CSV (lien
« Exporter » chez une entreprise, masqué sur un téléphone ; adresses `/vehicules.csv`,
`/conducteurs.csv`, `/affectations.csv`, `/interventions.csv`, `/echeances.csv`).

Deux profils d'installation, choisis par la variable `PROFIL` : **entreprise** (par défaut) montre
tout ce qui précède ; **particulier**, pour qui suit ses propres véhicules, masque l'écran
« Affectations », les blocs d'affectations des fiches et le service des conducteurs.

Chaque profil a sa navigation. **Autoclair Pro** (entreprise) : une barre latérale avec le menu
complet, une recherche (immatriculation, marque, modèle, VIN, nom d'un conducteur ; touche `/`
pour y aller) et le compte connecté, repliée derrière un bouton « Menu » sur un écran étroit.
**Autoclair** (particulier) : quatre onglets, Véhicules · Échéances · Entretien · Plus, en bas du
téléphone ; « Plus » rassemble le reste (alertes, conducteurs, dépôt d'un document, recherche,
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
  d'actions collée au bord droit (#78).
- La photo d'un document passe par le champ de fichier du navigateur, qui ouvre l'appareil photo
  du téléphone (décidé le 11/10/2026). Chaque document porte un type, choisi au dépôt, et se
  range sur la fiche du véhicule ou du conducteur. L'application ne lit pas le contenu du
  document : une ligne d'entretien ou une échéance se saisit toujours à la main.
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
- Les comptes se créent en ligne de commande ; pas d'inscription ni de changement de mot de passe
  depuis l'interface en v1. Un mot de passe oublié se remplace sur la machine du serveur :
  `cargo run -- changer-mot-de-passe LOGIN`.
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
de passe depuis l'interface, export CSV des listes, import en masse de véhicules, gestion des
contraventions, suivi du carburant, lecture automatique d'une facture photographiée (date,
montant, garage), réduction des photos avant leur envoi.
