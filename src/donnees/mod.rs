//! Accès aux données : une table par fichier, uniquement des requêtes SQL paramétrées.
//!
//! Garanties de cette couche :
//! - aucune requête n'est construite par concaténation : les valeurs passent par `.bind()`, donc
//!   par les paramètres `$1`, `$2`… du protocole PostgreSQL (injection SQL impossible) ;
//! - aucune règle de gestion : la validation et les messages sont dans `metier` ;
//! - les fonctions acceptent `impl PgExecutor` : on peut leur passer le pool (`&pool`) ou une
//!   transaction en cours (`&mut *transaction`) sans les réécrire.
//!
//! Les structures `Xxx` représentent une ligne lue ; les structures `NouveauXxx` représentent une
//! saisie déjà validée, prête à écrire.

pub mod affectations;
pub mod alertes;
pub mod conducteurs;
pub mod documents;
pub mod echeances;
pub mod interventions;
pub mod lignes_intervention;
pub mod utilisateurs;
pub mod vehicules;
