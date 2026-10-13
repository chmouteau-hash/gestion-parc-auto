//! Échéances réglementaires et d'entretien : validation et opérations.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{StatutEcheance, TypeEcheance};
use crate::donnees::echeances::{self, Echeance, NouvelleEcheance};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::alertes::{niveau, Niveau};
use crate::metier::validation::{code_parmi, date_obligatoire, identifiant};

/// Ce que l'écran dit d'une échéance : son statut, précisé par la date du jour.
///
/// Il se calcule à l'affichage, il n'est pas stocké : une échéance « à traiter » devient
/// « dépassée » le lendemain de sa date limite, sans que personne n'y touche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatEcheance {
    /// Ouverte, et sa date limite est passée.
    Depassee,
    /// Ouverte, et sa date limite n'est pas passée : le jour même, elle ne l'est pas encore.
    ATraiter,
    Traitee,
}

impl EtatEcheance {
    pub fn libelle(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => Niveau::Depassee.libelle(),
            EtatEcheance::ATraiter => StatutEcheance::Ouverte.libelle(),
            EtatEcheance::Traitee => StatutEcheance::Traitee.libelle(),
        }
    }

    /// Classe CSS de la pastille.
    pub fn classe(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => "echeance-depassee",
            EtatEcheance::ATraiter => "echeance-a_traiter",
            EtatEcheance::Traitee => "echeance-traitee",
        }
    }
}

/// État d'une échéance à la date donnée. « Dépassée » se décide par [`niveau`], la règle de la
/// vue alertes : la liste, la fiche et les alertes ne peuvent pas se contredire.
pub fn etat(echeance: &Echeance, aujourd_hui: NaiveDate) -> EtatEcheance {
    if !echeance.est_ouverte() {
        EtatEcheance::Traitee
    } else if niveau(echeance.date_limite, aujourd_hui) == Niveau::Depassee {
        EtatEcheance::Depassee
    } else {
        EtatEcheance::ATraiter
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieEcheance {
    #[serde(default)]
    pub vehicule_id: String,
    #[serde(default)]
    pub type_echeance: String,
    #[serde(default)]
    pub date_limite: String,
    /// Vide = « ouverte ».
    #[serde(default)]
    pub statut: String,
}

impl SaisieEcheance {
    pub fn depuis(e: &Echeance) -> SaisieEcheance {
        SaisieEcheance {
            vehicule_id: e.vehicule_id.to_string(),
            type_echeance: e.type_echeance.clone(),
            date_limite: e.date_limite.to_string(),
            statut: e.statut.clone(),
        }
    }
}

pub fn valider(s: &SaisieEcheance) -> Resultat<NouvelleEcheance> {
    let statut = if s.statut.trim().is_empty() {
        StatutEcheance::Ouverte
    } else {
        code_parmi(&s.statut, "statut", StatutEcheance::depuis_code)?
    };
    Ok(NouvelleEcheance {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_echeance: code_parmi(&s.type_echeance, "type", TypeEcheance::depuis_code)?,
        date_limite: date_obligatoire(&s.date_limite, "date limite")?,
        statut,
    })
}

fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Echeance>> {
    Ok(echeances::lister(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Echeance>> {
    Ok(echeances::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Echeance> {
    echeances::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn creer(pool: &PgPool, saisie: &SaisieEcheance) -> Resultat<Echeance> {
    let nouvelle = valider(saisie)?;
    let id = echeances::inserer(pool, &nouvelle)
        .await
        .map_err(traduire)?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieEcheance) -> Resultat<Echeance> {
    let nouvelle = valider(saisie)?;
    if !echeances::mettre_a_jour(pool, id, &nouvelle)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

/// Marque l'échéance comme traitée : elle disparaît des alertes.
pub async fn traiter(pool: &PgPool, id: i64) -> Resultat<Echeance> {
    if !echeances::changer_statut(pool, id, StatutEcheance::Traitee).await? {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if echeances::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statut_vide_vaut_ouverte() {
        let s = SaisieEcheance {
            vehicule_id: "1".into(),
            type_echeance: "assurance".into(),
            date_limite: "2027-01-01".into(),
            statut: "".into(),
        };
        assert_eq!(valider(&s).unwrap().statut, StatutEcheance::Ouverte);
    }

    #[test]
    fn type_inconnu_refuse() {
        let s = SaisieEcheance {
            vehicule_id: "1".into(),
            type_echeance: "vignette".into(),
            date_limite: "2027-01-01".into(),
            statut: "".into(),
        };
        assert!(valider(&s).is_err());
    }

    fn echeance(date_limite: NaiveDate, statut: &str) -> Echeance {
        Echeance {
            id: 1,
            vehicule_id: 1,
            type_echeance: "assurance".into(),
            date_limite,
            statut: statut.into(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AA-001-AA".into(),
        }
    }

    #[test]
    fn etat_selon_la_date_et_le_statut() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let hier = aujourd_hui.pred_opt().unwrap();
        let demain = aujourd_hui.succ_opt().unwrap();

        // Comme sur la vue alertes, une échéance n'est dépassée que le lendemain de sa date.
        let ouverte = |date| etat(&echeance(date, "ouverte"), aujourd_hui);
        assert_eq!(ouverte(hier), EtatEcheance::Depassee);
        assert_eq!(ouverte(aujourd_hui), EtatEcheance::ATraiter);
        assert_eq!(ouverte(demain), EtatEcheance::ATraiter);

        // Traitée, elle le reste quelle que soit sa date.
        let traitee = etat(&echeance(hier, "traitee"), aujourd_hui);
        assert_eq!(traitee, EtatEcheance::Traitee);

        assert_eq!(EtatEcheance::Depassee.libelle(), "Dépassée");
        assert_eq!(EtatEcheance::ATraiter.libelle(), "À traiter");
        assert_eq!(EtatEcheance::Traitee.libelle(), "Traitée");
    }
}
