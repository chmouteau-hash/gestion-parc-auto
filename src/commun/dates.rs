//! La date du jour, en un seul endroit.
//!
//! Elle sert à dire qu'une échéance est dépassée, qu'une affectation est en cours, à dater un
//! relevé de compteur. C'est la date **du serveur**, dans son fuseau horaire : sur un poste réglé
//! à l'heure de Paris, celle de l'utilisateur ; sur un serveur hébergé, souvent réglé en UTC, une
//! date en retard d'une ou deux heures autour de minuit. Le remède est dans l'environnement du
//! service, pas dans le code : `TZ=Europe/Paris` (voir `docs/exploitation.md`, § 2).
//!
//! Les tests d'intégration ne passent pas par ici : ils fixent leur propre date
//! (`tests/commun/mod.rs`), pour donner le même résultat quel que soit le jour où ils tournent.

use chrono::NaiveDate;

/// La date du jour dans le fuseau horaire du serveur.
pub fn aujourd_hui() -> NaiveDate {
    chrono::Local::now().date_naive()
}
