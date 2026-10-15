//! Véhicules : validation et opérations.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{Carburant, StatutVehicule};
use crate::donnees::vehicules::{self, NouveauVehicule, Vehicule};
use crate::donnees::{affectations, documents};
use crate::erreurs::{a_le_code, contrainte_violee, ErreurApp, Resultat, CODE_VIOLATION_UNICITE};
use crate::metier::documents::{self as metier_documents, Rattachement, SaisieDocument};
use crate::metier::validation::{
    code_parmi, date_obligatoire, entier_positif, immatriculation, texte_obligatoire, vin,
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
    /// Numéro d'identification (case E de la carte grise) ; vide si on ne l'a pas.
    #[serde(default)]
    pub vin: String,
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
            vin: v.vin.clone(),
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
        vin: vin(&s.vin)?,
    })
}

/// Deux colonnes sont uniques : l'immatriculation, et le VIN quand il est renseigné. Le nom de
/// l'index violé dit laquelle.
fn traduire(erreur: sqlx::Error, nouveau: &NouveauVehicule) -> ErreurApp {
    if !a_le_code(&erreur, CODE_VIOLATION_UNICITE) {
        return erreur.into();
    }
    if contrainte_violee(&erreur).as_deref() == Some(vehicules::INDEX_VIN_UNIQUE) {
        ErreurApp::metier(format!(
            "Le numéro d'identification « {} » est déjà celui d'un autre véhicule.",
            nouveau.vin
        ))
    } else {
        ErreurApp::metier(format!(
            "L'immatriculation « {} » est déjà utilisée par un autre véhicule.",
            nouveau.immatriculation
        ))
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
        .map_err(|e| traduire(e, &nouveau))
}

/// Refus d'une « photo » qui n'est pas une image, à la création du véhicule.
pub fn message_photo_creation_pas_une_image() -> &'static str {
    "Une photo du véhicule est une image (JPEG, PNG ou WebP). Un autre fichier se dépose depuis la fiche, une fois le véhicule enregistré."
}

/// Crée le véhicule et, s'il y en a une, dépose sa photo dans la foulée.
///
/// La photo est contrôlée (image lisible, taille) **avant** la création : une photo refusée ne
/// laisse ni véhicule ni fichier derrière elle, et le formulaire peut être ré-affiché avec le
/// motif. Un refus de la saisie du véhicule passe lui aussi avant tout travail sur l'image.
pub async fn creer_avec_photo(
    pool: &PgPool,
    documents_dir: &Path,
    saisie: &SaisieVehicule,
    photo: Option<&SaisieDocument>,
) -> Resultat<Vehicule> {
    let nouveau = valider(saisie)?;
    let photo_prete = match photo {
        Some(photo) => Some(metier_documents::preparer(photo).await.map_err(|erreur| {
            // Le formulaire de création n'a pas de choix de type : le motif du dépôt
            // (« choisissez un autre type ») n'y aurait pas de sens.
            match erreur {
                ErreurApp::Metier(m) if m == metier_documents::message_photo_pas_une_image() => {
                    ErreurApp::metier(message_photo_creation_pas_une_image())
                }
                autre => autre,
            }
        })?),
        None => None,
    };
    let vehicule = vehicules::inserer(pool, &nouveau)
        .await
        .map_err(|e| traduire(e, &nouveau))?;
    if let Some(prete) = photo_prete {
        metier_documents::ecrire(
            pool,
            documents_dir,
            Rattachement::Vehicule(vehicule.id),
            prete,
        )
        .await?;
    }
    Ok(vehicule)
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
        .map_err(|e| traduire(e, &nouveau))?
        .ok_or(ErreurApp::Introuvable)?;
    if vehicule.est_reforme() {
        affectations::cloturer_pour_vehicule(&mut *transaction, id, aujourd_hui).await?;
    }
    transaction.commit().await?;
    Ok(vehicule)
}

/// Remet en service un véhicule réformé par erreur (#87) : il redevient « En service », dans la
/// liste de travail et affectable.
///
/// Les affectations que la réforme a terminées ne sont **pas** rouvertes : la base ne garde pas
/// lesquelles l'ont été par la réforme, et en rouvrir au hasard de vieilles affectations serait
/// pire que de les recréer. Un véhicule qui n'est pas réformé (déjà remis en service par un autre
/// administrateur, ou jamais réformé) est rendu tel quel : le résultat voulu est déjà là.
pub async fn remettre_en_service(pool: &PgPool, id: i64) -> Resultat<Vehicule> {
    match vehicules::remettre_en_service(pool, id).await? {
        Some(vehicule) => Ok(vehicule),
        None => obtenir(pool, id).await,
    }
}

/// Relève le compteur en un geste, depuis la carte d'accueil ou la fiche : un entier, jamais en
/// dessous du compteur actuel (un compteur ne recule pas ; une faute de frappe se corrige par le
/// formulaire complet).
pub async fn relever_kilometrage(pool: &PgPool, id: i64, saisie: &str) -> Resultat<Vehicule> {
    let kilometrage = entier_positif(saisie, "kilométrage")?;
    let vehicule = obtenir(pool, id).await?;
    if kilometrage < vehicule.kilometrage {
        return Err(ErreurApp::metier(format!(
            "Le compteur est déjà à {} : le kilométrage ne peut pas baisser.",
            vehicule.kilometrage_fr()
        )));
    }
    vehicules::relever_kilometrage(pool, id, kilometrage).await?;
    obtenir(pool, id).await
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
            vin: String::new(),
        }
    }

    #[test]
    fn saisie_complete_acceptee() {
        let v = valider(&saisie_valide()).unwrap();
        assert_eq!(v.immatriculation, "AB-123-CD");
        assert_eq!(v.carburant, Carburant::Essence);
        assert_eq!(v.kilometrage, 45000);
        assert_eq!(v.vin, "");
    }

    #[test]
    fn vin_facultatif_mais_de_bonne_forme() {
        // Numéros fictifs. Minuscules, espaces et tirets de recopie sont tolérés.
        let mut s = saisie_valide();
        s.vin = " vf1r fb00-x123 45678 ".into();
        assert_eq!(valider(&s).unwrap().vin, "VF1RFB00X12345678");

        // 16 caractères, ou une lettre interdite : refusé, avec la règle dans le message.
        for mauvais in [
            "VF1RFB00X1234567",
            "VF1RFB00O12345678",
            "VF1RFB00X12345678Z",
        ] {
            s.vin = mauvais.into();
            let erreur = valider(&s).unwrap_err();
            assert!(
                matches!(&erreur, ErreurApp::Metier(m) if m.contains("17 caractères")),
                "{mauvais} : {erreur:?}"
            );
        }
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
