//! Échéances réglementaires et d'entretien : validation et opérations.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{StatutEcheance, TypeEcheance};
use crate::donnees::echeances::{self, Echeance, NouvelleEcheance};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::validation::{code_parmi, date_obligatoire, identifiant};

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
}
