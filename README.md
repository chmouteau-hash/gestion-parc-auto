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
| **Alertes** | Échéances dépassées ou à moins de 30 jours, permis de conduire qui expirent ; lien vers la fiche concernée. |
| **Véhicules** | Liste, fiche (statut, conducteur actuel, échéances, entretien, historique, documents), création, modification, suppression. |
| **Conducteurs** | Liste, fiche (affectations, documents), création, modification, suppression. |
| **Affectations** | Qui conduit quoi, depuis quand ; clôture en un clic. Un véhicule peut avoir plusieurs conducteurs, un conducteur plusieurs véhicules. Un véhicule réformé, un conducteur inactif ou au permis expiré ne peut pas être affecté. |
| **Entretien** | Interventions (révision, réparation, pneus…) avec date, kilométrage, coût, prestataire. Le kilométrage du véhicule suit. |
| **Échéances** | Contrôle technique, assurance, entretien périodique : date limite, à traiter / traitée. |
| **Documents** | Fichiers (10 Mo max) rattachés à un véhicule ou un conducteur, stockés sur disque. |

Deux rôles : **administrateur** (tout) et **lecteur** (consultation seule). Une API JSON
(`/api/v1/…`) expose les mêmes opérations pour les scripts.

## Lancer l'application en local

Prérequis : [Rust](https://rustup.rs) stable (1.85 ou plus) et PostgreSQL 16 ou plus
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
5. Lancer le serveur : `cargo run`, puis ouvrir <http://localhost:3000>.

La première compilation prend quelques minutes (téléchargement et compilation des dépendances) ;
les suivantes sont rapides.

## Vérifier le projet

```bash
cargo fmt --check                               # formatage
cargo clippy --all-targets -- -D warnings       # linter
cargo test --lib                                # tests unitaires (sans base)
cargo test                                      # tous les tests ; DATABASE_URL doit pointer vers une base de TEST
```

Les tests d'intégration créent une base jetable par test à partir de `DATABASE_URL` : utiliser
une base dédiée (par exemple `gestion_parc_auto_test`) dont l'utilisateur a le droit `CREATEDB`.
Sous PowerShell : `$env:DATABASE_URL="postgres://parc:…@localhost:5432/gestion_parc_auto_test"; cargo test`.

La CI (`.github/workflows/ci.yml`) joue formatage, clippy, migrations sur base vide (deux fois),
tests et compilation release sur chaque pull request et sur `main`.

## Structure du dépôt

```
src/
  main.rs       point d'entrée et sous-commandes (migrer, creer-utilisateur, purger-conducteurs)
  web/          routes HTTP, session, pages et API JSON — aucune requête SQL ici
  metier/       validation des saisies et règles de gestion — ne connaît ni HTTP ni SQL
  donnees/      requêtes SQL paramétrées, une table par fichier
  commun/       vocabulaires (statuts, types) et mise en forme, partagés par les trois couches
templates/      pages HTML (Askama, compilées avec le programme)
static/         feuille de style et script de confirmation
migrations/     schéma PostgreSQL, fichiers SQL horodatés
tests/          tests d'intégration sur base réelle
docs/           architecture.md, base-de-donnees.md, exploitation.md
```

La dépendance va toujours dans le même sens : `web → metier → donnees`. Le détail et les choix
techniques sont dans [docs/architecture.md](docs/architecture.md) ; le schéma dans
[docs/base-de-donnees.md](docs/base-de-donnees.md) ; lancement, sauvegarde et RGPD dans
[docs/exploitation.md](docs/exploitation.md).

## Hypothèses retenues

- Rust + Axum + SQLx + Askama, PostgreSQL 16+, pas de framework JavaScript (seul un script de
  confirmation avant suppression).
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
- Les montants sont stockés en centimes entiers ; la saisie se fait en euros (`149,90`).
- Les comptes se créent en ligne de commande ; pas d'inscription ni de changement de mot de passe
  depuis l'interface en v1.
- Le serveur écoute sur `127.0.0.1` uniquement ; un serveur web en HTTPS est à placer devant
  pour l'exposer à d'autres postes.
- Le dépôt est privé ; aucun secret ni document réel n'y est versionné (`.env`, `documents/`,
  `sauvegardes/` sont exclus).

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
contraventions, suivi du carburant.
