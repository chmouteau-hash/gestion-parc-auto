//! Affectations : qui peut conduire quoi.
//!
//! Un véhicule peut avoir plusieurs conducteurs en même temps, et un conducteur plusieurs
//! véhicules. Trois règles encadrent l'acte d'affecter :
//!
//! 1. un véhicule **réformé** ne reçoit plus d'affectation ;
//! 2. un conducteur **inactif**, ou dont le **permis est expiré**, ne reçoit pas de véhicule ;
//! 3. le même conducteur n'est pas affecté **deux fois en cours** au même véhicule.
//!
//! Chaque règle est une fonction pure, testée sans base. Les règles 1 et 2 sont vérifiées ici,
//! avant d'écrire. La règle 3 est tenue deux fois : ici, avec un message précis, et en base par
//! l'index unique partiel `affectations_pas_de_doublon_en_cours`, qui refuse le doublon même si
//! deux requêtes arrivent en même temps ([`traduire`] rend alors un message lisible).
//!
//! Les règles 1 et 2 portent sur l'acte d'affecter, pas sur l'historique : corriger les dates
//! d'une affectation existante, ou la clôturer, reste possible quand le permis du conducteur a
//! expiré entre-temps, ou sur l'historique d'un véhicule réformé.
//!
//! Leur pendant est la clôture automatique : réformer un véhicule ou désactiver un conducteur met
//! fin à ses affectations encore ouvertes. Elle vit là où le statut change, dans
//! `metier::vehicules::modifier` et `metier::conducteurs::modifier`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::donnees::affectations::{
    self, Affectation, NouvelleAffectation, INDEX_PAS_DE_DOUBLON_EN_COURS,
};
use crate::donnees::conducteurs::{self, Conducteur};
use crate::donnees::vehicules::{self, Vehicule};
use crate::erreurs::{
    a_le_code, contrainte_violee, ErreurApp, Resultat, CODE_VIOLATION_CHECK,
    CODE_VIOLATION_CLE_ETRANGERE, CODE_VIOLATION_UNICITE,
};
use crate::metier::validation::{date_facultative, date_obligatoire, identifiant};

const MESSAGE_REFERENCE_INCONNUE: &str = "Le véhicule ou le conducteur choisi n'existe plus.";

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

/// Règle pure : un véhicule réformé ne reçoit plus d'affectation.
pub fn verifier_vehicule_affectable(vehicule: &Vehicule) -> Resultat<()> {
    if vehicule.est_reforme() {
        return Err(ErreurApp::metier(format!(
            "Le véhicule {} est réformé : il ne peut plus être affecté.",
            vehicule.immatriculation
        )));
    }
    Ok(())
}

/// Règle pure : seul un conducteur actif, au permis non expiré, reçoit un véhicule.
pub fn verifier_conducteur_affectable(
    conducteur: &Conducteur,
    aujourd_hui: NaiveDate,
) -> Resultat<()> {
    if !conducteur.est_actif() {
        return Err(ErreurApp::metier(format!(
            "{} a le statut « {} » : aucun véhicule ne peut lui être affecté.",
            conducteur.nom_complet(),
            conducteur.statut_libelle()
        )));
    }
    if conducteur.permis_expire(aujourd_hui) {
        return Err(ErreurApp::metier(format!(
            "{} a un permis expiré depuis le {} : aucun véhicule ne peut lui être affecté.",
            conducteur.nom_complet(),
            conducteur.date_expiration_permis_fr()
        )));
    }
    Ok(())
}

/// Règle pure : refuse si ce conducteur a déjà une affectation en cours sur ce véhicule, sauf si
/// c'est celle qu'on est en train de modifier (`ignorer`).
pub fn verifier_pas_de_doublon(
    en_cours: Option<&Affectation>,
    ignorer: Option<i64>,
) -> Resultat<()> {
    match en_cours {
        Some(a) if Some(a.id) != ignorer => Err(ErreurApp::metier(format!(
            "Le véhicule {} est déjà affecté à {} depuis le {}.",
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
        && contrainte_violee(&erreur).as_deref() == Some(INDEX_PAS_DE_DOUBLON_EN_COURS)
    {
        ErreurApp::metier("Ce conducteur est déjà affecté à ce véhicule.")
    } else if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier(MESSAGE_REFERENCE_INCONNUE)
    } else if a_le_code(&erreur, CODE_VIOLATION_CHECK) {
        ErreurApp::metier("La date de fin ne peut pas précéder la date de début.")
    } else {
        erreur.into()
    }
}

/// Lit le véhicule et applique [`verifier_vehicule_affectable`].
async fn verifier_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<()> {
    let vehicule = vehicules::obtenir(pool, vehicule_id)
        .await?
        .ok_or_else(|| ErreurApp::metier(MESSAGE_REFERENCE_INCONNUE))?;
    verifier_vehicule_affectable(&vehicule)
}

/// Lit le conducteur et applique [`verifier_conducteur_affectable`].
async fn verifier_conducteur(
    pool: &PgPool,
    conducteur_id: i64,
    aujourd_hui: NaiveDate,
) -> Resultat<()> {
    let conducteur = conducteurs::obtenir(pool, conducteur_id)
        .await?
        .ok_or_else(|| ErreurApp::metier(MESSAGE_REFERENCE_INCONNUE))?;
    verifier_conducteur_affectable(&conducteur, aujourd_hui)
}

/// Lit l'éventuelle affectation en cours du couple véhicule/conducteur et applique
/// [`verifier_pas_de_doublon`]. Ne concerne que les affectations sans date de fin.
async fn verifier_doublon(
    pool: &PgPool,
    nouvelle: &NouvelleAffectation,
    ignorer: Option<i64>,
) -> Resultat<()> {
    if nouvelle.date_fin.is_some() {
        return Ok(());
    }
    let en_cours =
        affectations::en_cours_pour(pool, nouvelle.vehicule_id, nouvelle.conducteur_id).await?;
    verifier_pas_de_doublon(en_cours.as_ref(), ignorer)
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

/// Crée une affectation. `aujourd_hui` sert à juger si le permis du conducteur est expiré.
pub async fn creer(
    pool: &PgPool,
    saisie: &SaisieAffectation,
    aujourd_hui: NaiveDate,
) -> Resultat<Affectation> {
    let nouvelle = valider(saisie)?;
    verifier_vehicule(pool, nouvelle.vehicule_id).await?;
    verifier_conducteur(pool, nouvelle.conducteur_id, aujourd_hui).await?;
    verifier_doublon(pool, &nouvelle, None).await?;
    let id = affectations::inserer(pool, &nouvelle)
        .await
        .map_err(traduire)?;
    obtenir(pool, id).await
}

/// Modifie une affectation.
///
/// Les règles sur le véhicule et le conducteur ne rejouent que si la modification revient à
/// affecter : le véhicule change, le conducteur change, ou une affectation terminée est
/// rouverte (date de fin retirée). Corriger une date ou poser une date de fin passe toujours.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieAffectation,
    aujourd_hui: NaiveDate,
) -> Resultat<Affectation> {
    let nouvelle = valider(saisie)?;
    let actuelle = obtenir(pool, id).await?;
    let rouverte = !actuelle.est_active() && nouvelle.date_fin.is_none();
    if rouverte || nouvelle.vehicule_id != actuelle.vehicule_id {
        verifier_vehicule(pool, nouvelle.vehicule_id).await?;
    }
    if rouverte || nouvelle.conducteur_id != actuelle.conducteur_id {
        verifier_conducteur(pool, nouvelle.conducteur_id, aujourd_hui).await?;
    }
    verifier_doublon(pool, &nouvelle, Some(id)).await?;
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

    fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, j).unwrap()
    }

    fn message(resultat: Resultat<()>) -> String {
        match resultat.unwrap_err() {
            ErreurApp::Metier(m) => m,
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    fn vehicule(statut: &str) -> Vehicule {
        Vehicule {
            id: 1,
            immatriculation: "AB-123-CD".into(),
            marque: "Renault".into(),
            modele: "Kangoo".into(),
            date_mise_en_service: jour(2023, 6, 1),
            carburant: "diesel".into(),
            kilometrage: 12000,
            statut: statut.into(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }
    }

    fn conducteur(statut: &str, permis: NaiveDate) -> Conducteur {
        Conducteur {
            id: 2,
            nom: "Martin".into(),
            prenom: "Léa".into(),
            service: "Logistique".into(),
            date_expiration_permis: permis,
            statut: statut.into(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }
    }

    fn en_cours(id: i64) -> Affectation {
        Affectation {
            id,
            vehicule_id: 1,
            conducteur_id: 2,
            date_debut: jour(2026, 1, 10),
            date_fin: None,
            cree_le: Utc::now(),
            immatriculation: "AB-123-CD".into(),
            conducteur_nom: "Martin".into(),
            conducteur_prenom: "Léa".into(),
        }
    }

    #[test]
    fn vehicule_en_service_ou_en_maintenance_affectable() {
        assert!(verifier_vehicule_affectable(&vehicule("en_service")).is_ok());
        assert!(verifier_vehicule_affectable(&vehicule("en_maintenance")).is_ok());
    }

    #[test]
    fn vehicule_reforme_refuse() {
        let m = message(verifier_vehicule_affectable(&vehicule("reforme")));
        assert!(m.contains("AB-123-CD"), "message : {m}");
        assert!(m.contains("réformé"), "message : {m}");
    }

    #[test]
    fn conducteur_actif_au_permis_valide_affectable() {
        let c = conducteur("actif", jour(2031, 1, 1));
        assert!(verifier_conducteur_affectable(&c, jour(2026, 10, 11)).is_ok());
    }

    #[test]
    fn conducteur_inactif_refuse() {
        let c = conducteur("inactif", jour(2031, 1, 1));
        let m = message(verifier_conducteur_affectable(&c, jour(2026, 10, 11)));
        assert!(m.contains("Léa Martin"), "message : {m}");
        assert!(m.contains("Inactif"), "message : {m}");
    }

    #[test]
    fn permis_expire_refuse_mais_valable_le_jour_meme() {
        let aujourd_hui = jour(2026, 10, 11);
        let expire_hier = conducteur("actif", jour(2026, 10, 10));
        let m = message(verifier_conducteur_affectable(&expire_hier, aujourd_hui));
        assert!(m.contains("Léa Martin"), "message : {m}");
        assert!(m.contains("10/10/2026"), "message : {m}");

        let expire_aujourd_hui = conducteur("actif", aujourd_hui);
        assert!(verifier_conducteur_affectable(&expire_aujourd_hui, aujourd_hui).is_ok());
    }

    #[test]
    fn pas_de_doublon_quand_rien_en_cours() {
        assert!(verifier_pas_de_doublon(None, None).is_ok());
    }

    #[test]
    fn doublon_en_cours_refuse_avec_message() {
        let m = message(verifier_pas_de_doublon(Some(&en_cours(7)), None));
        assert!(m.contains("AB-123-CD"), "message : {m}");
        assert!(m.contains("Léa Martin"), "message : {m}");
        assert!(m.contains("10/01/2026"), "message : {m}");
    }

    #[test]
    fn modifier_la_meme_affectation_n_est_pas_un_doublon() {
        assert!(verifier_pas_de_doublon(Some(&en_cours(7)), Some(7)).is_ok());
        assert!(verifier_pas_de_doublon(Some(&en_cours(7)), Some(8)).is_err());
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
