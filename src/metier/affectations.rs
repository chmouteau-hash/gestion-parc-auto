//! Affectations : qui peut conduire quoi, et quand.
//!
//! Un véhicule peut avoir plusieurs conducteurs en même temps, et un conducteur plusieurs
//! véhicules. Une affectation est « en cours » quand la date du jour tombe dans sa période : à
//! partir du jour de début, et avant le jour de fin, qui est celui où elle s'arrête. Avant elle
//! est « à venir », ensuite « terminée » (voir [`Etat`](crate::donnees::affectations::Etat)).
//!
//! Trois règles encadrent l'acte d'affecter :
//!
//! 1. un véhicule **réformé** ne reçoit plus d'affectation ;
//! 2. un conducteur **inactif**, ou dont le **permis est expiré**, ne reçoit pas de véhicule ;
//! 3. le même conducteur n'a pas, sur le même véhicule, deux affectations dont les **périodes se
//!    chevauchent** : il ne peut pas y être affecté deux fois le même jour.
//!
//! Chaque règle est une fonction pure, testée sans base, et vérifiée ici avant d'écrire. Le cas
//! le plus courant de la règle 3, deux affectations sans date de fin, est en plus tenu en base
//! par l'index unique partiel `affectations_pas_de_doublon_en_cours`, qui refuse le doublon même
//! si deux requêtes arrivent en même temps ([`traduire`] rend alors un message lisible).
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

/// Règle pure : la période de la nouvelle affectation ne chevauche aucune des affectations
/// existantes du même conducteur sur le même véhicule (`existantes`), sauf celle qu'on est en
/// train de modifier (`ignorer`).
pub fn verifier_pas_de_chevauchement(
    existantes: &[Affectation],
    nouvelle: &NouvelleAffectation,
    ignorer: Option<i64>,
) -> Resultat<()> {
    let genante = existantes
        .iter()
        .find(|a| Some(a.id) != ignorer && a.chevauche(nouvelle.date_debut, nouvelle.date_fin));
    match genante {
        Some(a) => Err(ErreurApp::metier(format!(
            "Le véhicule {} est déjà affecté à {} {} : les deux périodes se chevauchent.",
            a.immatriculation,
            a.conducteur_nom_complet(),
            a.periode_fr()
        ))),
        None => Ok(()),
    }
}

/// Règle pure : vrai si la modification fait durer l'affectation plus longtemps qu'avant, au
/// point qu'elle ne soit pas terminée aujourd'hui. C'est le cas quand on retire une date de fin,
/// ou qu'on la repousse dans le futur : cela revient à affecter de nouveau. Corriger une date de
/// fin déjà atteinte n'est pas concerné.
pub fn prolonge_jusqu_a_aujourd_hui(
    actuelle: &Affectation,
    nouvelle: &NouvelleAffectation,
    aujourd_hui: NaiveDate,
) -> bool {
    let fin_repoussee = match (actuelle.date_fin, nouvelle.date_fin) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(avant), Some(apres)) => apres > avant,
    };
    fin_repoussee && nouvelle.date_fin.is_none_or(|fin| fin > aujourd_hui)
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

/// Lit les affectations du couple véhicule/conducteur et applique
/// [`verifier_pas_de_chevauchement`].
async fn verifier_chevauchement(
    pool: &PgPool,
    nouvelle: &NouvelleAffectation,
    ignorer: Option<i64>,
) -> Resultat<()> {
    let existantes =
        affectations::lister_pour_couple(pool, nouvelle.vehicule_id, nouvelle.conducteur_id)
            .await?;
    verifier_pas_de_chevauchement(&existantes, nouvelle, ignorer)
}

/// Toutes les affectations : en cours d'abord, puis à venir, puis terminées. Dans chaque groupe,
/// la plus récente en premier.
pub async fn lister(pool: &PgPool, aujourd_hui: NaiveDate) -> Resultat<Vec<Affectation>> {
    let mut liste = affectations::lister(pool).await?;
    // Tri stable : dans chaque état, l'ordre rendu par la base est conservé.
    liste.sort_by_key(|a| a.etat(aujourd_hui));
    Ok(liste)
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
    verifier_chevauchement(pool, &nouvelle, None).await?;
    let id = affectations::inserer(pool, &nouvelle)
        .await
        .map_err(traduire)?;
    obtenir(pool, id).await
}

/// Modifie une affectation.
///
/// Les règles sur le véhicule et le conducteur ne rejouent que si la modification revient à
/// affecter : le véhicule change, le conducteur change, ou l'affectation est prolongée au-delà
/// d'aujourd'hui (date de fin retirée, ou repoussée dans le futur). Corriger une date de début,
/// avancer une date de fin ou retoucher l'historique passe toujours.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieAffectation,
    aujourd_hui: NaiveDate,
) -> Resultat<Affectation> {
    let nouvelle = valider(saisie)?;
    let actuelle = obtenir(pool, id).await?;
    let prolongee = prolonge_jusqu_a_aujourd_hui(&actuelle, &nouvelle, aujourd_hui);
    if prolongee || nouvelle.vehicule_id != actuelle.vehicule_id {
        verifier_vehicule(pool, nouvelle.vehicule_id).await?;
    }
    if prolongee || nouvelle.conducteur_id != actuelle.conducteur_id {
        verifier_conducteur(pool, nouvelle.conducteur_id, aujourd_hui).await?;
    }
    verifier_chevauchement(pool, &nouvelle, Some(id)).await?;
    if !affectations::mettre_a_jour(pool, id, &nouvelle)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

/// Met fin à une affectation à la date donnée (en général aujourd'hui). Elle peut être sans date
/// de fin, ou avoir une fin prévue plus tard, qui est alors avancée.
pub async fn cloturer(pool: &PgPool, id: i64, date_fin: NaiveDate) -> Resultat<Affectation> {
    let actuelle = obtenir(pool, id).await?;
    if date_fin < actuelle.date_debut {
        return Err(ErreurApp::metier(
            "La date de fin ne peut pas précéder la date de début.",
        ));
    }
    if actuelle.date_fin.is_some_and(|fin| fin <= date_fin) {
        return Err(ErreurApp::metier(
            "Cette affectation est déjà terminée à cette date.",
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
    use crate::donnees::affectations::Etat;
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

    /// Affectation de Léa Martin sur AB-123-CD, sur la période donnée.
    fn affectation(id: i64, debut: NaiveDate, fin: Option<NaiveDate>) -> Affectation {
        Affectation {
            id,
            vehicule_id: 1,
            conducteur_id: 2,
            date_debut: debut,
            date_fin: fin,
            cree_le: Utc::now(),
            immatriculation: "AB-123-CD".into(),
            conducteur_nom: "Martin".into(),
            conducteur_prenom: "Léa".into(),
        }
    }

    fn periode(debut: NaiveDate, fin: Option<NaiveDate>) -> NouvelleAffectation {
        NouvelleAffectation {
            vehicule_id: 1,
            conducteur_id: 2,
            date_debut: debut,
            date_fin: fin,
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
    fn etat_d_une_affectation_datee() {
        // Du 11 octobre au 9 décembre 2026.
        let a = affectation(1, jour(2026, 10, 11), Some(jour(2026, 12, 9)));
        assert_eq!(a.etat(jour(2026, 10, 10)), Etat::AVenir);
        assert_eq!(a.etat(jour(2026, 10, 11)), Etat::EnCours, "jour de début");
        assert_eq!(a.etat(jour(2026, 11, 15)), Etat::EnCours);
        assert_eq!(a.etat(jour(2026, 12, 8)), Etat::EnCours, "veille de la fin");
        assert_eq!(a.etat(jour(2026, 12, 9)), Etat::Terminee, "jour de fin");
        assert_eq!(a.etat(jour(2026, 12, 10)), Etat::Terminee);
    }

    #[test]
    fn etat_d_une_affectation_sans_date_de_fin() {
        let a = affectation(1, jour(2026, 10, 11), None);
        assert_eq!(a.etat(jour(2026, 10, 10)), Etat::AVenir);
        assert_eq!(a.etat(jour(2026, 10, 11)), Etat::EnCours);
        assert_eq!(a.etat(jour(2035, 1, 1)), Etat::EnCours);
    }

    #[test]
    fn terminer_aujourd_hui_prend_effet_tout_de_suite() {
        // « Terminer aujourd'hui », une réforme ou une désactivation posent la date du jour comme
        // date de fin : l'affectation n'est alors plus en cours, sans attendre le lendemain.
        let aujourd_hui = jour(2026, 10, 11);
        let terminee_aujourd_hui = affectation(1, jour(2026, 10, 1), Some(aujourd_hui));
        assert_eq!(terminee_aujourd_hui.etat(aujourd_hui), Etat::Terminee);
        assert!(!terminee_aujourd_hui.est_en_cours(aujourd_hui));

        // Même pour une affectation commencée le jour même.
        let commencee_et_terminee = affectation(2, aujourd_hui, Some(aujourd_hui));
        assert_eq!(commencee_et_terminee.etat(aujourd_hui), Etat::Terminee);
    }

    #[test]
    fn les_etats_se_classent_en_cours_puis_a_venir_puis_terminee() {
        let mut etats = [Etat::Terminee, Etat::AVenir, Etat::EnCours];
        etats.sort();
        assert_eq!(etats, [Etat::EnCours, Etat::AVenir, Etat::Terminee]);
    }

    #[test]
    fn pas_de_chevauchement_quand_rien_n_existe() {
        let nouvelle = periode(jour(2026, 1, 10), None);
        assert!(verifier_pas_de_chevauchement(&[], &nouvelle, None).is_ok());
    }

    #[test]
    fn deux_affectations_sans_fin_se_chevauchent() {
        let existantes = [affectation(7, jour(2026, 1, 10), None)];
        let nouvelle = periode(jour(2026, 6, 1), None);
        let m = message(verifier_pas_de_chevauchement(&existantes, &nouvelle, None));
        assert!(m.contains("AB-123-CD"), "message : {m}");
        assert!(m.contains("Léa Martin"), "message : {m}");
        assert!(m.contains("depuis le 10/01/2026"), "message : {m}");
    }

    #[test]
    fn une_periode_datee_chevauche_si_elle_partage_un_jour() {
        // Existante : du 11 octobre au 9 décembre.
        let existantes = [affectation(7, jour(2026, 10, 11), Some(jour(2026, 12, 9)))];
        let chevauche = |debut, fin| {
            verifier_pas_de_chevauchement(&existantes, &periode(debut, fin), None).is_err()
        };
        assert!(chevauche(jour(2026, 11, 1), None), "commence au milieu");
        assert!(
            chevauche(jour(2026, 12, 8), Some(jour(2026, 12, 31))),
            "commence la veille de la fin"
        );
        assert!(
            chevauche(jour(2026, 9, 1), Some(jour(2026, 10, 12))),
            "finit le lendemain du début"
        );
        // Deux périodes qui se suivent ne se chevauchent pas : l'une s'arrête le jour où
        // l'autre commence.
        assert!(
            !chevauche(jour(2026, 12, 9), None),
            "commence le jour de fin"
        );
        assert!(
            !chevauche(jour(2026, 9, 1), Some(jour(2026, 10, 11))),
            "finit le jour de début"
        );

        let m = message(verifier_pas_de_chevauchement(
            &existantes,
            &periode(jour(2026, 11, 1), None),
            None,
        ));
        assert!(m.contains("du 11/10/2026 au 09/12/2026"), "message : {m}");
    }

    #[test]
    fn modifier_la_meme_affectation_n_est_pas_un_chevauchement() {
        let existantes = [affectation(7, jour(2026, 1, 10), None)];
        let nouvelle = periode(jour(2026, 1, 15), None);
        assert!(verifier_pas_de_chevauchement(&existantes, &nouvelle, Some(7)).is_ok());
        assert!(verifier_pas_de_chevauchement(&existantes, &nouvelle, Some(8)).is_err());
    }

    #[test]
    fn prolonger_jusqu_a_aujourd_hui_revient_a_affecter() {
        let aujourd_hui = jour(2026, 10, 11);
        let terminee = affectation(1, jour(2026, 1, 1), Some(jour(2026, 3, 1)));
        let sans_fin = affectation(2, jour(2026, 1, 1), None);
        let prolonge = |actuelle: &Affectation, fin| {
            prolonge_jusqu_a_aujourd_hui(actuelle, &periode(jour(2026, 1, 1), fin), aujourd_hui)
        };

        assert!(prolonge(&terminee, None), "date de fin retirée");
        assert!(
            prolonge(&terminee, Some(jour(2026, 12, 31))),
            "fin repoussée dans le futur"
        );
        assert!(
            !prolonge(&terminee, Some(aujourd_hui)),
            "fin repoussée à aujourd'hui : l'affectation reste terminée"
        );
        assert!(
            !prolonge(&terminee, Some(jour(2026, 4, 1))),
            "fin repoussée, mais toujours dans le passé"
        );
        assert!(!prolonge(&terminee, Some(jour(2026, 2, 1))), "fin avancée");
        assert!(!prolonge(&sans_fin, None), "rien ne change");
        assert!(
            !prolonge(&sans_fin, Some(jour(2026, 12, 31))),
            "poser une date de fin raccourcit"
        );
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
