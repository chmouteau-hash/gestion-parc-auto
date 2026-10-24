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
 actif (vrai par défaut)
                                 kilometrage (CHECK >= 0)           statut (CHECK)
                                 statut (CHECK)                     cree_le, modifie_le
                                 vin (17 car., unique si renseigné)
                                 couleur (CHECK, vide si aucune)
                                 crit_air (CHECK, vide si aucune)
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
                                   │                                 intervention_id, depense_id
                                   │                                 cree_le
                            lignes_intervention
                            ───────────────────
                            id
                            intervention_id
                            operation (CHECK)
                            libelle (« autre »)
                            reference_piece
                            quantite_centiemes (>= 1)
                            prix_centimes

 depenses (vehicule_id → vehicules, ON DELETE CASCADE)
 ────────
 id, vehicule_id, type_depense (CHECK), date_depense, montant_centimes (>= 0),
 carburant (CHECK, vide pour un péage), quantite_centiemes (>= 0, 0 = non notée),
 kilometrage (NULL = non relevé), plein_complet, libelle, notes, cree_le

 plan_entretien (vehicule_id → vehicules, ON DELETE CASCADE)
 ──────────────
 id, vehicule_id, operation (ni vide ni 'autre'), periodicite_km (> 0, NULL = sans),
 periodicite_mois (> 0, NULL = sans), cree_le ; au moins une périodicité

 releves_kilometrage (vehicule_id → vehicules, ON DELETE CASCADE)
 ───────────────────
 id, vehicule_id, date_releve, kilometrage (>= 0), cree_le
```

`releves_kilometrage` (migration `20261024090000`, #94, bloc 4 ; retour #4) : les relevés du
compteur faits à la main (« Relever », le kilométrage saisi à la création du véhicule). Avec les
interventions et les pleins, qui portent déjà leur kilométrage et ne sont pas recopiés ici, ils
forment l'historique du compteur. `vehicules.kilometrage` est le plus haut kilométrage de cet
historique : l'application le recalcule dans la transaction de chaque écriture d'une des trois
sources, et il redescend quand on corrige ou supprime la saisie fautive. La migration a repris en
relevé, daté de sa dernière modification, le compteur d'un véhicule plus haut que tout son
historique.

`depenses` (migration `20261021150000`, #126) : les pleins de carburant et les tickets de péage,
une ligne par dépense, d'un seul type à la fois. Seul un plein porte le carburant pris ce jour-là
(`carburant`, migration `20261021160000`, #128, vocabulaire `CarburantPlein`, proposé par le
formulaire d'après `vehicules.carburant` à la création, obligatoire à l'enregistrement), une quantité (centièmes de litre, de kWh ou
de kg : l'unité se déduit du carburant du plein), le compteur et la case « plein complet » : les
contraintes `depenses_carburant_du_plein` et `depenses_detail_du_carburant` imposent leurs valeurs
par défaut sur un péage. La consommation et les totaux ne sont pas stockés : ils se
calculent à l'affichage (bloc 3).

`lignes_intervention` (migration `20261016120000`, #94) : ce qui a été fait pendant une
intervention, une ligne par opération, avec la pièce, sa quantité et son prix pour mémoire. Les
lignes partent avec leur intervention (`ON DELETE CASCADE`) et se remplacent en bloc quand on la
modifie. Le coût total reste sur l'intervention : c'est celui de la facture. La quantité est en
centièmes (`540` = 5,4 L d'huile), comme le prix en centimes : une vidange ne se compte pas en
entiers (migration `20261016150000`, retour #99).

`plan_entretien` (migration `20261023120000`, #94, bloc 2) : les opérations suivies d'un véhicule,
une ligne par opération, avec sa périodicité en kilomètres, en mois, ou les deux (contrainte
`plan_entretien_une_periodicite`). La dernière fois et la prochaine borne ne sont pas stockées :
la dernière fois est la plus récente intervention du véhicule qui contient une ligne de
l'opération, sans compter celles datées dans le futur. La « dernière fois » saisie en ajoutant une
opération au plan s'enregistre comme une intervention, avec sa ligne, pas dans cette table.

Toutes les clés étrangères vers `vehicules` et `conducteurs` sont `ON DELETE CASCADE` : supprimer
un véhicule supprime son historique. C'est voulu pour corriger une erreur de saisie ; pour un
véhicule qui a réellement existé, on passe son statut à `reforme` et on le garde.

### Vocabulaires (contraintes `CHECK`)

| Colonne | Valeurs |
| --- | --- |
| `utilisateurs.role` | `administrateur`, `lecteur` |
| `utilisateurs.actif` | booléen (migration `20261024170000`, #172) : faux, le compte ne peut plus se connecter et sa session s'arrête ; il n'est pas supprimé |
| `vehicules.carburant` | `essence`, `diesel`, `electrique`, `hybride`, `gpl`, `autre` |
| `vehicules.statut` | `en_service`, `en_maintenance`, `reforme` |
| `vehicules.couleur` | vide (aucune), `blanc`, `gris`, `noir`, `rouge`, `orange`, `jaune`, `vert`, `bleu`, `violet`, `marron` (migration `20261016230000`), plus `beige` et les niveaux `gris_clair`, `gris_fonce`, `rouge_clair`, `rouge_fonce`, `vert_clair`, `vert_fonce`, `bleu_clair`, `bleu_fonce`, `marron_clair` (migration `20261021100000`) |
| `vehicules.crit_air` | vide (aucune vignette choisie), `e`, `1`, `2`, `3`, `4`, `5`, `non_classe` (migration `20261024120000`, #165). La classe proposée d'après le carburant et la date de mise en service se calcule à la lecture (`CritAir::propose_pour`) : elle ne se stocke pas |
| `conducteurs.statut` | `actif`, `inactif` |
| `interventions.type_intervention` | `revision`, `entretien_courant` (migration `20261023100000`, #94), `reparation`, `pneumatiques`, `controle_technique`, `contre_visite` (migration `20261016200000`, #107), `carrosserie`, `autre` |
| `interventions.resultat_controle` | `favorable`, `defavorable`, `critique` pour un contrôle technique ou une contre-visite ; `''` pour les autres types (contrainte `interventions_resultat_d_un_controle`) et pour les contrôles saisis avant la migration `20261016200000` (#107) |
| `lignes_intervention.operation` | `vidange`, `filtre_huile`, `filtre_air`, `filtre_habitacle`, `filtre_carburant`, `bougies`, `courroie_distribution`, `courroie_accessoires`, `plaquettes`, `disques`, `liquide_frein`, `liquide_refroidissement`, `huile_boite`, `batterie`, `pneus`, `amortisseurs`, `echappement`, `balais`, `adblue`, `climatisation` (pièces et consommables), `pression_pneus`, `permutation_pneus`, `niveaux`, `geometrie`, `eclairage` (contrôles et réglages ; ces neuf codes depuis la migration `20261023100000`, #94), `autre` (avec un `libelle` non vide : contrainte `lignes_intervention_autre_precisee`) |
| `echeances.type_echeance` | `controle_technique`, `contre_visite`, `assurance`, `entretien_periodique`, `a_faire` (migrations `20261019100000`, #98, et `20261023160000`, #107) |
| `plan_entretien.operation` | un code d'opération (le vocabulaire de `lignes_intervention.operation`) sauf `autre`, dont le texte libre ne permet pas de retrouver la dernière fois. Vérifié par l'application, comme `echeances.operation` ; la base refuse seulement le vide et `autre` |
| `echeances.operation` | vide, ou un code d'opération (le vocabulaire de `lignes_intervention.operation`), obligatoire pour `a_faire` et seulement pour lui. Sa valeur est vérifiée par l'application, pas par un `CHECK` : la liste n'existe pas en deux exemplaires en base |
| `echeances.statut` | `ouverte`, `traitee` |
| `depenses.type_depense` | `carburant`, `peage` (migration `20261021150000`, #126) |
| `depenses.carburant` | `sp95`, `sp95_e10`, `sp98`, `e85`, `gazole`, `gazole_b10`, `gpl`, `gnv`, `electricite`, `autre` pour un plein ; `''` pour un péage (contrainte `depenses_carburant_du_plein`, migration `20261021160000`) |
| `documents.type_document` | `photo_vehicule` (migration `20261014180000`), `facture`, `controle_technique`, `carte_grise`, `assurance`, `certificat_qualite_air` (migration `20261024120000`, #165), `facture_achat`, `certificat_cession`, `contrat_achat`, `garantie` (les quatre : migration `20261016220000`, #109), `ticket` (migration `20261021170000`, #126), `permis`, `autre` (valeur des documents déposés avant la migration `20261011193000`). Le dossier de la fiche (Achat, Administratif, Entretien, Dépenses, Autres) se déduit du type dans le code (`TypeDocument::dossier`) : pas de colonne. |

`documents.vignette` (migration `20261014180000`) : nom du fichier de la vignette dans
`DOCUMENTS_DIR`, calculée par le serveur au dépôt d'une photo du véhicule (`metier::vignette`,
JPEG de 640 px au plus) ; vide pour tout autre document. La photo la plus récente d'un véhicule qui
en a une sert de vignette à sa fiche et à sa carte d'accueil.

`documents.intervention_id` (migration `20261016170000`, #100) : l'intervention que le document
justifie, quand il a été joint depuis le formulaire d'intervention (sa facture, le plus souvent).
Facultatif, et toujours sur un document du véhicule (contrainte `documents_intervention_du_vehicule`) ;
`ON DELETE SET NULL` : supprimer l'intervention garde le document dans les documents du véhicule.
Que l'intervention soit bien celle du véhicule se vérifie dans le code (`web/documents.rs::enregistrer`),
pas par une clé composite.

`documents.depense_id` (migration `20261021170000`, #126, bloc 2) : la dépense que le document
justifie, quand il a été joint depuis le formulaire de dépense (son ticket de carburant ou de
péage). Mêmes règles que `intervention_id` : facultatif, toujours sur un document du véhicule
(contrainte `documents_depense_du_vehicule`), `ON DELETE SET NULL`, et la dépense doit être celle
du véhicule, vérifié dans le code. Un document n'est relié qu'à l'un des deux, par construction
des formulaires ; la base ne l'interdit pas.

Les mêmes listes sont déclarées dans `src/commun/types.rs`, avec leurs libellés d'affichage. Le
test `codes_acceptes_par_la_base` (`tests/vehicules.rs`) insère chaque code et échoue si les deux
listes divergent. Pour ajouter une valeur : une nouvelle migration qui remplace la contrainte, et
une ligne dans `types.rs`.

### Index

| Index | Rôle |
| --- | --- |
| `vehicules_immatriculation_key` (implicite, UNIQUE) | recherche par immatriculation, unicité |
| `vehicules_vin_unique` (UNIQUE partiel, `WHERE vin <> ''`) | deux véhicules ne partagent pas un numéro d'identification ; les VIN vides ne comptent pas (migration `20261014150000`) |
| `conducteurs_date_expiration_permis_idx` | vue alertes (permis) |
| `affectations_vehicule_id_idx`, `affectations_conducteur_id_idx` | historique par véhicule / conducteur |
| `affectations_pas_de_doublon_en_cours` (UNIQUE partiel sur `vehicule_id, conducteur_id`, `WHERE date_fin IS NULL`) | **règle métier** : pas deux affectations sans date de fin pour le même conducteur sur le même véhicule. Le chevauchement de périodes datées est vérifié par l'application. Un véhicule peut avoir plusieurs conducteurs : l'ancien index `affectations_une_seule_active_par_vehicule` a été retiré par la migration `20261011160000` |
| `interventions_vehicule_id_idx`, `interventions_date_intervention_idx` | fiche véhicule, liste par date |
| `lignes_intervention_intervention_id_idx` | les lignes d'une intervention (migration `20261016120000`) |
| `lignes_intervention_pas_de_doublon` (UNIQUE partiel sur `intervention_id, operation`, `WHERE operation <> 'autre'`) | **règle métier** : une opération n'apparaît qu'une fois par intervention, la quantité dit combien ; seule « autre » se répète, son libellé la distingue |
| `echeances_vehicule_id_idx` | fiche véhicule |
| `echeances_ouvertes_date_limite_idx` (partiel, `WHERE statut = 'ouverte'`) | vue alertes |
| `echeances_pas_de_doublon_ouverte` (UNIQUE partiel sur `vehicule_id, type_echeance, date_limite, operation, libelle`, `WHERE statut = 'ouverte'`) | **règle métier** : pas deux échéances ouvertes identiques pour un même véhicule (deux « à faire » à la même date pour deux choses différentes sont deux échéances ; pour les trois autres types, `operation` et `libelle` sont vides et la règle est celle d'avant). Une échéance traitée ne compte pas : elle ne bloque pas une nouvelle saisie à la même date. La migration `20261014190000` supprime les doublons ouverts déjà saisis, en gardant le plus ancien, avant de poser l'index |
| `echeances_une_contre_visite_ouverte` (UNIQUE partiel sur `vehicule_id`, `WHERE statut = 'ouverte' AND type_echeance = 'contre_visite'`) | **règle métier** : une seule contre-visite à faire par véhicule, celle du dernier contrôle défavorable ; l'application passe l'ancienne à « traitée » avant d'en créer une nouvelle, et rend le refus d'une saisie à la main en message lisible (#107) |
| `plan_entretien_une_ligne_par_operation` (UNIQUE sur `vehicule_id, operation`) | **règle métier** : une opération ne se suit qu'une fois par véhicule. Sert aussi à lire le plan d'une fiche (migration `20261023120000`) |
| `releves_kilometrage_par_vehicule` (sur `vehicule_id`) | lecture : l'historique du compteur d'un véhicule, et le recalcul de son compteur à chaque écriture (#94, bloc 4) |
| `depenses_vehicule_id_idx`, `depenses_date_depense_idx` | fiche véhicule, liste par date (migration `20261021150000`) |
| `documents_vehicule_id_idx`, `documents_conducteur_id_idx` | fiche véhicule / conducteur |
| `documents_intervention_id_idx` (partiel, `WHERE intervention_id IS NOT NULL`) | les factures des interventions d'une fiche ou de la liste « Entretien », en une requête |
| `documents_depense_id_idx` (partiel, `WHERE depense_id IS NOT NULL`) | les tickets des dépenses d'une fiche ou de la liste « Dépenses », en une requête (migration `20261021170000`) |

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
