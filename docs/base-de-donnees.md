# Base de données

PostgreSQL 16 ou plus récent. Le schéma est entièrement décrit par les fichiers du dossier
`migrations/`, appliqués dans l'ordre par SQLx.

## 1. Schéma

```
 utilisateurs                    vehicules                          conducteurs
 ─────────────                   ──────────                         ───────────
 id                              id                                 id
 login (unique)                  immatriculation (unique)           nom
 mot_de_passe_hash               marque, modele                     prenom
 role (CHECK)                    date_mise_en_service               service
 cree_le                         carburant (CHECK)                  date_expiration_permis
                                 kilometrage (CHECK >= 0)           statut (CHECK)
                                 statut (CHECK)                     cree_le, modifie_le
                                 cree_le, modifie_le
                                      │                                  │
          ┌───────────────────────────┼──────────────────┬───────────────┤
          │                           │                  │               │
   affectations               interventions          echeances       documents
   ────────────               ─────────────          ─────────       ─────────
   id                         id                     id              id
   vehicule_id  ──┐           vehicule_id            vehicule_id     vehicule_id   (l'un
   conducteur_id ─┘           type_intervention      type_echeance   conducteur_id  ou l'autre)
   date_debut                 date_intervention      date_limite     type_document (CHECK)
   date_fin (NULL=sans fin)   kilometrage            statut (CHECK)  nom_original
   cree_le                    cout_centimes          cree_le         chemin (unique)
                              prestataire, notes                     type_mime
                              cree_le                                taille_octets
                                                                     cree_le
```

Toutes les clés étrangères vers `vehicules` et `conducteurs` sont `ON DELETE CASCADE` : supprimer
un véhicule supprime son historique. C'est voulu pour corriger une erreur de saisie ; pour un
véhicule qui a réellement existé, on passe son statut à `reforme` et on le garde.

### Vocabulaires (contraintes `CHECK`)

| Colonne | Valeurs |
| --- | --- |
| `utilisateurs.role` | `administrateur`, `lecteur` |
| `vehicules.carburant` | `essence`, `diesel`, `electrique`, `hybride`, `gpl`, `autre` |
| `vehicules.statut` | `en_service`, `en_maintenance`, `reforme` |
| `conducteurs.statut` | `actif`, `inactif` |
| `interventions.type_intervention` | `revision`, `reparation`, `pneumatiques`, `controle_technique`, `carrosserie`, `autre` |
| `echeances.type_echeance` | `controle_technique`, `assurance`, `entretien_periodique` |
| `echeances.statut` | `ouverte`, `traitee` |
| `documents.type_document` | `facture`, `controle_technique`, `carte_grise`, `assurance`, `permis`, `autre` (valeur des documents déposés avant la migration `20261011193000`) |

Les mêmes listes sont déclarées dans `src/commun/types.rs`, avec leurs libellés d'affichage. Le
test `codes_acceptes_par_la_base` (`tests/vehicules.rs`) insère chaque code et échoue si les deux
listes divergent. Pour ajouter une valeur : une nouvelle migration qui remplace la contrainte, et
une ligne dans `types.rs`.

### Index

| Index | Rôle |
| --- | --- |
| `vehicules_immatriculation_key` (implicite, UNIQUE) | recherche par immatriculation, unicité |
| `conducteurs_date_expiration_permis_idx` | vue alertes (permis) |
| `affectations_vehicule_id_idx`, `affectations_conducteur_id_idx` | historique par véhicule / conducteur |
| `affectations_pas_de_doublon_en_cours` (UNIQUE partiel sur `vehicule_id, conducteur_id`, `WHERE date_fin IS NULL`) | **règle métier** : pas deux affectations sans date de fin pour le même conducteur sur le même véhicule. Le chevauchement de périodes datées est vérifié par l'application. Un véhicule peut avoir plusieurs conducteurs : l'ancien index `affectations_une_seule_active_par_vehicule` a été retiré par la migration `20261011160000` |
| `interventions_vehicule_id_idx`, `interventions_date_intervention_idx` | fiche véhicule, liste par date |
| `echeances_vehicule_id_idx` | fiche véhicule |
| `echeances_ouvertes_date_limite_idx` (partiel, `WHERE statut = 'ouverte'`) | vue alertes |
| `documents_vehicule_id_idx`, `documents_conducteur_id_idx` | fiche véhicule / conducteur |

### Conventions

- Identifiants `BIGINT GENERATED ALWAYS AS IDENTITY`.
- Montants en centimes entiers (`cout_centimes BIGINT`), jamais en virgule flottante.
- Horodatages `TIMESTAMPTZ` (`cree_le`, `modifie_le`), dates métier en `DATE`.
- Textes en `TEXT` ; la longueur est bornée côté application (200 caractères, 5 000 pour les notes).
- `modifie_le` est mis à jour par les requêtes `UPDATE` de l'application, pas par un
  déclencheur : il sert de référence à la purge RGPD des conducteurs inactifs.

## 2. Migrations

- Un fichier par changement, nommé `AAAAMMJJHHMMSS_description.sql`, dans `migrations/`. L'ordre
  chronologique du nom est l'ordre d'application.
- SQLx note chaque fichier appliqué dans la table `_sqlx_migrations` (nom, somme de contrôle,
  date). Un fichier déjà appliqué est ignoré ; un fichier **modifié après application** est
  refusé (somme de contrôle différente). Règle : on ne retouche jamais une migration appliquée,
  on en ajoute une nouvelle.
- Les migrations sont **embarquées dans l'exécutable** (`sqlx::migrate!` dans `src/lib.rs`) et
  appliquées au démarrage de `cargo run`, ainsi que par `cargo run -- migrer`. L'outil
  `sqlx migrate run` (sqlx-cli) fait la même chose depuis le dossier.
- Créer une migration : `sqlx migrate add description` crée le fichier horodaté vide.
- Les tests d'intégration (`#[sqlx::test]`) créent une base vide par test et y appliquent toutes
  les migrations : une migration qui ne s'applique pas sur une base vide casse les tests.

## 3. Connexion

L'application lit `DATABASE_URL` (`postgres://utilisateur:mot_de_passe@hote:port/base`) et ouvre
un pool de 10 connexions au plus. Pour les tests, `DATABASE_URL` doit pointer vers une base
**dédiée** (par exemple `gestion_parc_auto_test`) dont l'utilisateur a le droit `CREATEDB` :
SQLx y crée et détruit des bases jetables nommées `_sqlx_test_…`.
