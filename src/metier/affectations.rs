//! Affectations : règle « une seule affectation active par véhicule ».
//!
//! La règle est tenue deux fois :
//! 1. ici, avant d'écrire : on cherche l'affectation en cours du véhicule et on refuse avec un
//!    message précis (fonction pure [`verifier_aucune_active`], testée sans base) ;
//! 2. en base, par l'index unique partiel `affectations_une_seule_active_par_vehicule` : si deux
//!    requêtes passent la vérification 1 en même temps, PostgreSQL refuse la seconde et
//!    [`traduire`] rend le même message.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::donnees::affectations::{
    self, Affectation, NouvelleAffectation, INDEX_UNE_SEULE_ACTIVE,
};
use crate::erreurs::{
    a_le_code, contrainte_violee, ErreurApp, Resultat, CODE_VIOLATION_CHECK,
    CODE_VIOLATION_CLE_ETRANGERE, CODE_VIOLATION_UNICITE,
};
use crate::metier::validation::{date_facultative, date_obligatoire, identifiant};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieAffectation {
    #[serde(default)]
    pub vehicule_id: String,
    #[serde(default)]
    pub conducteur_id: String,
    #[serde(default)]
    pub date_debut: String,
    #[serde(default)]
    pub date_fin: String,
}

impl SaisieAffectation {
    pub fn depuis(a: &Affectation) -> SaisieAffectation {
        SaisieAffectation {
            vehicule_id: a.vehicule_id.to_string(),
            conducteur_id: a.conducteur_id.to_string(),
            date_debut: a.date_debut.to_string(),
            date_fin: a.date_fin.map(|d| d.to_string()).unwrap_or_default(),
        }
    }
}

pub fn valider(s: &SaisieAffectation) -> Resultat<NouvelleAffectation> {
    let date_debut = date_obligatoire(&s.date_debut, "début")?;
    let date_fin = date_facultative(&s.date_fin, "fin")?;
    if let Some(fin) = date_fin {
        if fin < date_debut {
            return Err(ErreurApp::metier(
                "La date de fin ne peut pas précéder la date de début.",
            ));
        }
    }
    Ok(NouvelleAffectation {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        conducteur_id: identifiant(&s.conducteur_id, "conducteur")?,
        date_debut,
        date_fin,
    })
}

/// Règle pure : refuse si le véhicule a déjà une affectation en cours, sauf si c'est celle
/// qu'on est en train de modifier (`ignorer`).
pub fn verifier_aucune_active(active: Option<&Affectation>, ignorer: Option<i64>) -> Resultat<()> {
    match active {
        Some(a) if Some(a.id) != ignorer => Err(ErreurApp::metier(format!(
            "Le véhicule {} est déjà affecté à {} depuis le {}. Clôturez cette affectation d'abord.",
            a.immatriculation,
            a.conducteur_nom_complet(),
            a.date_debut_fr()
        ))),
        _ => Ok(()),
    }
}

/// Traduit les refus de la base en messages lisibles.
fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_UNICITE)
        && contrainte_violee(&erreur).as_deref() == Some(INDEX_UNE_SEULE_ACTIVE)
    {
        ErreurApp::metier("Ce véhicule a déjà une affectation en cours.")
    } else if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule ou le conducteur choisi n'existe plus.")
    } else if a_le_code(&erreur, CODE_VIOLATION_CHECK) {
        ErreurApp::metier("La date de fin ne peut pas précéder la date de début.")
    } else {
        erreur.into()
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Affectation>> {
    Ok(affectations::lister(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Affectation>> {
    Ok(affectations::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn lister_par_conducteur(
    pool: &PgPool,
    conducteur_id: i64,
) -> Resultat<Vec<Affectation>> {
    Ok(affectations::lister_par_conducteur(pool, conducteur_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Affectation> {
    affectations::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn creer(pool: &PgPool, saisie: &SaisieAffectation) -> Resultat<Affectation> {
    let nouvelle = valider(saisie)?;
    if nouvelle.date_fin.is_none() {
        let active = affectations::active_pour_vehicule(pool, nouvelle.vehicule_id).await?;
        verifier_aucune_active(active.as_ref(), None)?;
    }
    let id = affectations::inserer(pool, &nouvelle)
        .await
        .map_err(traduire)?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieAffectation) -> Resultat<Affectation> {
    let nouvelle = valider(saisie)?;
    if nouvelle.date_fin.is_none() {
        let active = affectations::active_pour_vehicule(pool, nouvelle.vehicule_id).await?;
        verifier_aucune_active(active.as_ref(), Some(id))?;
    }
    if !affectations::mettre_a_jour(pool, id, &nouvelle)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

/// Met fin à une affectation en cours à la date donnée (en général aujourd'hui).
pub async fn cloturer(pool: &PgPool, id: i64, date_fin: NaiveDate) -> Resultat<Affectation> {
    let actuelle = obtenir(pool, id).await?;
    if !actuelle.est_active() {
        return Err(ErreurApp::metier("Cette affectation est déjà terminée."));
    }
    if date_fin < actuelle.date_debut {
        return Err(ErreurApp::metier(
            "La date de fin ne peut pas précéder la date de début.",
        ));
    }
    affectations::cloturer(pool, id, date_fin)
        .await
        .map_err(traduire)?;
    obtenir(pool, id).await
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if affectations::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn active(id: i64) -> Affectation {
        Affectation {
            id,
            vehicule_id: 1,
            conducteur_id: 2,
            date_debut: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            date_fin: None,
            cree_le: Utc::now(),
            immatriculation: "AB-123-CD".into(),
            conducteur_nom: "Martin".into(),
            conducteur_prenom: "Léa".into(),
        }
    }

    #[test]
    fn aucune_active_accepte() {
        assert!(verifier_aucune_active(None, None).is_ok());
    }

    #[test]
    fn une_active_refuse_avec_message() {
        let erreur = verifier_aucune_active(Some(&active(7)), None).unwrap_err();
        match erreur {
            ErreurApp::Metier(m) => {
                assert!(m.contains("AB-123-CD"));
                assert!(m.contains("Léa Martin"));
                assert!(m.contains("10/01/2026"));
            }
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[test]
    fn modifier_la_meme_affectation_est_permis() {
        assert!(verifier_aucune_active(Some(&active(7)), Some(7)).is_ok());
        assert!(verifier_aucune_active(Some(&active(7)), Some(8)).is_err());
    }

    #[test]
    fn fin_avant_debut_refusee() {
        let s = SaisieAffectation {
            vehicule_id: "1".into(),
            conducteur_id: "2".into(),
            date_debut: "2026-05-10".into(),
            date_fin: "2026-05-01".into(),
        };
        assert!(valider(&s).is_err());
    }
}
