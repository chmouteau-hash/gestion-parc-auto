//! Échéances réglementaires et d'entretien : validation et opérations.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{StatutEcheance, TypeEcheance};
use crate::donnees::echeances::{self, Echeance, NouvelleEcheance};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::alertes::{jusqu_au, niveau, Niveau};
use crate::metier::validation::{code_parmi, date_obligatoire, identifiant};

/// Ce que l'écran dit d'une échéance : son statut, précisé par la date du jour.
///
/// Il se calcule à l'affichage, il n'est pas stocké : une échéance « à traiter » devient « à venir »
/// quand sa date entre dans l'horizon des alertes, puis « dépassée » le lendemain de sa date
/// limite, sans que personne n'y touche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatEcheance {
    /// Ouverte, et sa date limite est passée.
    Depassee,
    /// Ouverte, pas encore dépassée (le jour même, elle ne l'est pas), et sa date limite tombe
    /// dans l'horizon des alertes : la vue alertes la montre.
    AVenir,
    /// Ouverte, et sa date limite est au-delà de l'horizon des alertes.
    ATraiter,
    Traitee,
}

impl EtatEcheance {
    pub fn libelle(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => Niveau::Depassee.libelle(),
            EtatEcheance::AVenir => Niveau::AVenir.libelle(),
            EtatEcheance::ATraiter => StatutEcheance::Ouverte.libelle(),
            EtatEcheance::Traitee => StatutEcheance::Traitee.libelle(),
        }
    }

    /// Classe CSS de la pastille.
    pub fn classe(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => "echeance-depassee",
            EtatEcheance::AVenir => "echeance-a_venir",
            EtatEcheance::ATraiter => "echeance-a_traiter",
            EtatEcheance::Traitee => "echeance-traitee",
        }
    }
}

/// État d'une échéance à la date donnée. « Dépassée » se décide par [`niveau`] et « à venir » par
/// [`jusqu_au`], les règles de la vue alertes : la liste, la fiche et les alertes ne peuvent pas
/// se contredire.
pub fn etat(echeance: &Echeance, aujourd_hui: NaiveDate, horizon_jours: i64) -> EtatEcheance {
    if !echeance.est_ouverte() {
        EtatEcheance::Traitee
    } else if niveau(echeance.date_limite, aujourd_hui) == Niveau::Depassee {
        EtatEcheance::Depassee
    } else if echeance.date_limite <= jusqu_au(aujourd_hui, horizon_jours) {
        EtatEcheance::AVenir
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

/// Combien de mois après une échéance tombe la suivante du même type : deux ans pour un contrôle
/// technique, un an pour l'assurance et l'entretien périodique (durées à confirmer par
/// Christophe, issue #39, bloc 5).
pub fn periode_mois(type_echeance: TypeEcheance) -> u32 {
    match type_echeance {
        TypeEcheance::ControleTechnique => 24,
        TypeEcheance::Assurance | TypeEcheance::EntretienPeriodique => 12,
    }
}

/// La prochaine échéance, pré-remplie après « Marquer traitée » : même véhicule, même type, la
/// date limite décalée d'une période. Elle n'est pas enregistrée : c'est l'utilisateur qui décide.
pub fn prochaine(echeance: &Echeance) -> SaisieEcheance {
    let mois = TypeEcheance::depuis_code(&echeance.type_echeance)
        .map(periode_mois)
        .unwrap_or(12);
    let date_limite = echeance
        .date_limite
        .checked_add_months(chrono::Months::new(mois))
        .unwrap_or(echeance.date_limite);
    SaisieEcheance {
        vehicule_id: echeance.vehicule_id.to_string(),
        type_echeance: echeance.type_echeance.clone(),
        date_limite: date_limite.to_string(),
        statut: String::new(),
    }
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
    fn la_prochaine_echeance_garde_le_vehicule_et_le_type_et_decale_la_date() {
        let mut e = echeance(NaiveDate::from_ymd_opt(2027, 1, 31).unwrap(), "traitee");
        let suivante = prochaine(&e);
        assert_eq!(suivante.vehicule_id, "1");
        assert_eq!(suivante.type_echeance, "assurance");
        assert_eq!(suivante.date_limite, "2028-01-31");
        assert_eq!(suivante.statut, "");

        // Deux ans pour un contrôle technique ; une fin de mois reste une fin de mois.
        e.type_echeance = "controle_technique".into();
        e.date_limite = NaiveDate::from_ymd_opt(2028, 2, 29).unwrap();
        assert_eq!(prochaine(&e).date_limite, "2030-02-28");
    }

    #[test]
    fn etat_selon_la_date_et_le_statut() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let hier = aujourd_hui.pred_opt().unwrap();
        let demain = aujourd_hui.succ_opt().unwrap();

        // Comme sur la vue alertes, une échéance n'est dépassée que le lendemain de sa date, et
        // « à venir » jusqu'au dernier jour de l'horizon compris.
        let ouverte = |date| etat(&echeance(date, "ouverte"), aujourd_hui, 30);
        assert_eq!(ouverte(hier), EtatEcheance::Depassee);
        assert_eq!(ouverte(aujourd_hui), EtatEcheance::AVenir);
        assert_eq!(ouverte(demain), EtatEcheance::AVenir);
        let dernier_jour = jusqu_au(aujourd_hui, 30);
        assert_eq!(ouverte(dernier_jour), EtatEcheance::AVenir);
        assert_eq!(
            ouverte(dernier_jour.succ_opt().unwrap()),
            EtatEcheance::ATraiter
        );

        // Traitée, elle le reste quelle que soit sa date.
        let traitee = etat(&echeance(hier, "traitee"), aujourd_hui, 30);
        assert_eq!(traitee, EtatEcheance::Traitee);

        assert_eq!(EtatEcheance::Depassee.libelle(), "Dépassée");
        assert_eq!(EtatEcheance::AVenir.libelle(), "À venir");
        assert_eq!(EtatEcheance::ATraiter.libelle(), "À traiter");
        assert_eq!(EtatEcheance::Traitee.libelle(), "Traitée");
    }
}
