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
| **Alertes** | Échéances dépassées ou à moins de 30 jours, permis de conduire qui expirent ; lien vers la fiche concernée et, pour une échéance, « Marquer traitée » sans quitter la page. |
| **Véhicules** | Liste, fiche (statut, conducteur actuel, échéances, entretien, historique, documents), création, modification, suppression. |
| **Conducteurs** | Liste, fiche (affectations, documents), création, modification, suppression. |
| **Affectations** | Qui conduit quoi, depuis quand ; clôture en un clic. Un véhicule peut avoir plusieurs conducteurs, un conducteur plusieurs véhicules. Un véhicule réformé, un conducteur inactif ou au permis expiré ne peut pas être affecté. |
| **Entretien** | Interventions (révision, réparation, pneus…) avec date, kilométrage, coût, prestataire. Le kilométrage du véhicule suit. |
| **Échéances** | Contrôle technique, assurance, entretien périodique : date limite ; statut « à traiter », « à venir » dans les 30 jours qui précèdent la date limite, « dépassée » le lendemain de la date limite, ou « traitée ». |
| **Documents** | Fichiers (10 Mo max) rattachés à un véhicule ou un conducteur, avec leur type (facture, contrôle technique, carte grise, assurance, permis…), stockés sur disque. Une image ou un PDF s'ouvre à l'écran. |
| **Déposer un document** | Pensée pour le téléphone ou la tablette : on choisit le véhicule et le type, on prend la photo, elle est enregistrée sans autre geste. |

Deux rôles : **administrateur** (tout) et **lecteur** (consultation seule). Une API JSON
(`/api/v1/…`) expose les mêmes opérations pour les scripts.

Deux profils d'installation, choisis par la variable `PROFIL` : **entreprise** (par défaut) montre
tout ce qui précède ; **particulier**, pour qui suit ses propres véhicules, masque l'écran
« Affectations », les blocs d'affectations des fiches et le service des conducteurs.

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

Écrans présents : connexion, alertes, véhicules (liste, fiche, formulaire), affectations (liste),
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
templates/      pages HTML (Askama, compilées avec le programme)
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
  envoi d'un document dès qu'il est choisi, intitulés des tableaux sur écran étroit).
- Le catalogue d'interface (Storybook, dossier `storybook/`) est un outil de développement à
  part, avec ses propres dépendances Node : l'application n'en dépend pas et se compile sans lui.
  Ses stories recopient le HTML des gabarits (décidé le 12/10/2026).
- Sous 700 px de large (téléphone, panneau étroit), chaque ligne de tableau s'affiche comme une
  fiche : une information par ligne, les actions en dessous, rien à faire défiler de côté
  (décidé le 12/10/2026).
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
