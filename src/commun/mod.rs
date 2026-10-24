//! Définitions partagées par les trois couches : vocabulaires fermés et mise en forme.
//!
//! Ce module ne dépend d'aucun autre : il peut être utilisé aussi bien par `donnees` (pour
//! écrire un code de statut) que par `web` (pour afficher son libellé) sans créer de dépendance
//! circulaire entre les couches.

pub mod csv;
pub mod dates;
pub mod format;
pub mod types;
