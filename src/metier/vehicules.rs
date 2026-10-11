//! Véhicules : validation et opérations.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{Carburant, StatutVehicule};
use crate::donnees::vehicules::{self, NouveauVehicule, Vehicule};
use crate::donnees::{affectations, documents};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_UNICITE};
use crate::metier::validation::{
    code_parmi, date_obligatoire, entier_positif, immatriculation, texte_obligatoire,
};

/// Saisie brute d'un formulaire ou d'un appel JSON : que des chaînes, validées par [`valider`].
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieVehicule {
    #[serde(default)]
    pub immatriculation: String,
    #[serde(default)]
    pub marque: String,
    #[serde(default)]
    pub modele: String,
    #[serde(default)]
    pub date_mise_en_service: String,
    #[serde(default)]
    pub carburant: String,
    #[serde(default)]
    pub kilometrage: String,
    #[serde(default)]
    pub statut: String,
}

impl SaisieVehicule {
    /// Pré-remplit un formulaire de modification.
    pub fn depuis(v: &Vehicule) -> SaisieVehicule {
        SaisieVehicule {
            immatriculation: v.immatriculation.clone(),
            marque: v.marque.clone(),
            modele: v.modele.clone(),
            date_mise_en_service: v.date_mise_en_service.to_string(),
            carburant: v.carburant.clone(),
            kilometrage: v.kilometrage.to_string(),
            statut: v.statut.clone(),
        }
    }
}

pub fn valider(s: &SaisieVehicule) -> Resultat<NouveauVehicule> {
    Ok(NouveauVehicule {
        immatriculation: immatriculation(&s.immatriculation)?,
        marque: texte_obligatoire(&s.marque, "marque")?,
        modele: texte_obligatoire(&s.modele, "modèle")?,
        date_mise_en_service: date_obligatoire(&s.date_mise_en_service, "mise en service")?,
        carburant: code_parmi(&s.carburant, "carburant", Carburant::depuis_code)?,
        kilometrage: entier_positif(&s.kilometrage, "kilométrage")?,
        statut: code_parmi(&s.statut, "statut", StatutVehicule::depuis_code)?,
    })
}

fn traduire(erreur: sqlx::Error, immatriculation: &str) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_UNICITE) {
        ErreurApp::metier(format!(
            "L'immatriculation « {immatriculation} » est déjà utilisée par un autre véhicule."
        ))
    } else {
        erreur.into()
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Vehicule>> {
    Ok(vehicules::lister(pool).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Vehicule> {
    vehicules::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn creer(pool: &PgPool, saisie: &SaisieVehicule) -> Resultat<Vehicule> {
    let nouveau = valider(saisie)?;
    vehicules::inserer(pool, &nouveau)
        .await
        .map_err(|e| traduire(e, &nouveau.immatriculation))
}

/// Modifie le véhicule.
///
/// Un véhicule réformé est sorti du parc : l'enregistrer avec ce statut met fin, à la date du
/// jour, à ses affectations encore ouvertes. Les deux écritures sont dans une même transaction :
/// soit les deux réussissent, soit aucune.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieVehicule,
    aujourd_hui: NaiveDate,
) -> Resultat<Vehicule> {
    let nouveau = valider(saisie)?;
    let mut transaction = pool.begin().await?;
    let vehicule = vehicules::mettre_a_jour(&mut *transaction, id, &nouveau)
        .await
        .map_err(|e| traduire(e, &nouveau.immatriculation))?
        .ok_or(ErreurApp::Introuvable)?;
    if vehicule.est_reforme() {
        affectations::cloturer_pour_vehicule(&mut *transaction, id, aujourd_hui).await?;
    }
    transaction.commit().await?;
    Ok(vehicule)
}

/// Supprime le véhicule, son historique (cascade en base) et les fichiers de ses documents.
pub async fn supprimer(pool: &PgPool, documents_dir: &Path, id: i64) -> Resultat<()> {
    let docs = documents::lister_par_vehicule(pool, id).await?;
    if !vehicules::supprimer(pool, id).await? {
        return Err(ErreurApp::Introuvable);
    }
    crate::metier::documents::supprimer_fichiers(documents_dir, &docs).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saisie_valide() -> SaisieVehicule {
        SaisieVehicule {
            immatriculation: "ab-123-cd".into(),
            marque: "Renault".into(),
            modele: "Clio".into(),
            date_mise_en_service: "2022-03-15".into(),
            carburant: "essence".into(),
            kilometrage: "45000".into(),
            statut: "en_service".into(),
        }
    }

    #[test]
    fn saisie_complete_acceptee() {
        let v = valider(&saisie_valide()).unwrap();
        assert_eq!(v.immatriculation, "AB-123-CD");
        assert_eq!(v.carburant, Carburant::Essence);
        assert_eq!(v.kilometrage, 45000);
    }

    #[test]
    fn statut_inconnu_refuse() {
        let mut s = saisie_valide();
        s.statut = "vendu".into();
        let erreur = valider(&s).unwrap_err();
        assert!(matches!(erreur, ErreurApp::Metier(m) if m.contains("statut")));
    }

    #[test]
    fn kilometrage_negatif_refuse() {
        let mut s = saisie_valide();
        s.kilometrage = "-5".into();
        assert!(valider(&s).is_err());
    }
}
