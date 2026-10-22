//! Logique métier : validation des saisies et règles de gestion.
//!
//! Garanties de cette couche :
//! - toute saisie (formulaire ou JSON) entre sous forme de chaînes (`SaisieXxx`) et en sort
//!   validée (`NouveauXxx`, défini dans `donnees`) ou avec une [`ErreurApp::Metier`] lisible ;
//! - les règles qui dépendent de l'état de la base (qui peut être affecté à quoi, unicité de
//!   l'immatriculation) sont vérifiées ici **et** traduites quand la base les refuse ;
//! - rien ici ne connaît le HTTP.
//!
//! [`ErreurApp::Metier`]: crate::erreurs::ErreurApp::Metier

pub mod accueil;
pub mod affectations;
pub mod alertes;
pub mod authentification;
pub mod conducteurs;
pub mod consommation;
pub mod depenses;
pub mod documents;
pub mod echeances;
pub mod interventions;
pub mod recherche;
pub mod utilisateurs;
pub mod validation;
pub mod vehicules;
pub mod vignette;
