//! Règles d'affectation, prouvées sur une vraie base :
//!
//! - un véhicule peut avoir plusieurs conducteurs, et un conducteur plusieurs véhicules ;
//! - un véhicule réformé, un conducteur inactif ou au permis expiré ne reçoit pas d'affectation ;
//! - ces refus portent sur l'acte d'affecter : clôturer ou corriger une affectation existante
//!   reste possible ;
//! - le même conducteur n'est pas affecté deux fois en cours au même véhicule : l'application
//!   refuse avec un message lisible, et la base refuse même quand on la contourne (SQL direct).

mod commun;

use chrono::NaiveDate;
use gestion_parc_auto::erreurs::{a_le_code, contrainte_violee, ErreurApp, CODE_VIOLATION_UNICITE};
use gestion_parc_auto::metier::affectations::{self, SaisieAffectation};
use sqlx::PgPool;

use commun::{
    aujourd_hui, changer_conducteur, changer_statut_vehicule, creer_conducteur, creer_vehicule,
    saisie_affectation,
};

/// Le message d'un refus métier ; toute autre issue fait échouer le test.
async fn refus(pool: &PgPool, saisie: &SaisieAffectation) -> String {
    match affectations::creer(pool, saisie, aujourd_hui()).await {
        Err(ErreurApp::Metier(message)) => message,
        autre => panic!("un refus métier était attendu, reçu : {autre:?}"),
    }
}

#[sqlx::test]
async fn un_vehicule_peut_avoir_plusieurs_conducteurs(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AA-111-AA").await;
    let premier = creer_conducteur(&pool, "Durand").await;
    let second = creer_conducteur(&pool, "Lefebvre").await;

    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, premier, ""),
        aujourd_hui(),
    )
    .await
    .expect("première affectation acceptée");
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, second, ""),
        aujourd_hui(),
    )
    .await
    .expect("un second conducteur en même temps est accepté");

    let en_cours = affectations::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .filter(|a| a.est_active())
        .count();
    assert_eq!(en_cours, 2);
}

#[sqlx::test]
async fn un_conducteur_peut_avoir_plusieurs_vehicules(pool: PgPool) {
    let premier = creer_vehicule(&pool, "AB-111-AA").await;
    let second = creer_vehicule(&pool, "AB-222-AA").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;

    for vehicule in [premier, second] {
        affectations::creer(
            &pool,
            &saisie_affectation(vehicule, conducteur, ""),
            aujourd_hui(),
        )
        .await
        .expect("affectation acceptée");
    }

    let liste = affectations::lister_par_conducteur(&pool, conducteur)
        .await
        .unwrap();
    assert_eq!(liste.len(), 2);
    assert!(liste.iter().all(|a| a.est_active()));
}

#[sqlx::test]
async fn l_application_refuse_deux_fois_le_meme_conducteur_en_cours(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AC-111-AA").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let saisie = saisie_affectation(vehicule, conducteur, "");

    affectations::creer(&pool, &saisie, aujourd_hui())
        .await
        .expect("première affectation acceptée");

    let message = refus(&pool, &saisie).await;
    assert!(message.contains("AC-111-AA"), "message : {message}");
    assert!(message.contains("Camille Durand"), "message : {message}");

    // Une période déjà terminée ne gêne pas : l'historique peut contenir plusieurs lignes.
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, "2026-02-01"),
        aujourd_hui(),
    )
    .await
    .expect("affectation terminée acceptée");
}

#[sqlx::test]
async fn la_base_refuse_le_doublon_meme_en_contournant_l_application(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "BB-222-BB").await;
    let premier = creer_conducteur(&pool, "Durand").await;
    let second = creer_conducteur(&pool, "Lefebvre").await;

    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, premier, ""),
        aujourd_hui(),
    )
    .await
    .unwrap();

    // Insertions directes, sans passer par la vérification applicative.
    let inserer = |conducteur: i64| {
        sqlx::query(
            "INSERT INTO affectations (vehicule_id, conducteur_id, date_debut) VALUES ($1, $2, $3)",
        )
        .bind(vehicule)
        .bind(conducteur)
        .bind(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap())
        .execute(&pool)
    };

    // Un autre conducteur sur le même véhicule : la base l'accepte.
    inserer(second)
        .await
        .expect("la base accepte plusieurs conducteurs par véhicule");

    // Le même conducteur une seconde fois en cours : l'index unique partiel refuse.
    let erreur = inserer(premier)
        .await
        .expect_err("l'index unique partiel doit refuser le doublon");
    assert!(a_le_code(&erreur, CODE_VIOLATION_UNICITE), "{erreur}");
    assert_eq!(
        contrainte_violee(&erreur).as_deref(),
        Some("affectations_pas_de_doublon_en_cours")
    );
}

#[sqlx::test]
async fn un_vehicule_reforme_ne_recoit_pas_d_affectation(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "RF-111-RF").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    changer_statut_vehicule(&pool, vehicule, "RF-111-RF", "reforme").await;

    let message = refus(&pool, &saisie_affectation(vehicule, conducteur, "")).await;
    assert!(message.contains("RF-111-RF"), "message : {message}");
    assert!(message.contains("réformé"), "message : {message}");

    // La règle vaut aussi pour une période saisie après coup, avec une date de fin.
    refus(
        &pool,
        &saisie_affectation(vehicule, conducteur, "2026-02-01"),
    )
    .await;

    // Un véhicule en maintenance, lui, reste affectable.
    changer_statut_vehicule(&pool, vehicule, "RF-111-RF", "en_maintenance").await;
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .expect("un véhicule en maintenance reste affectable");
}

#[sqlx::test]
async fn un_conducteur_inactif_ne_recoit_pas_de_vehicule(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "IN-111-IN").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    changer_conducteur(&pool, conducteur, "Durand", "inactif", "2031-01-01").await;

    let message = refus(&pool, &saisie_affectation(vehicule, conducteur, "")).await;
    assert!(message.contains("Camille Durand"), "message : {message}");
    assert!(message.contains("Inactif"), "message : {message}");
    assert!(affectations::lister(&pool).await.unwrap().is_empty());
}

#[sqlx::test]
async fn un_permis_expire_empeche_l_affectation(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "PE-111-PE").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;

    // Permis expiré la veille du jour de test (1er juin 2026).
    changer_conducteur(&pool, conducteur, "Durand", "actif", "2026-05-31").await;
    let message = refus(&pool, &saisie_affectation(vehicule, conducteur, "")).await;
    assert!(message.contains("Camille Durand"), "message : {message}");
    assert!(message.contains("31/05/2026"), "message : {message}");

    // Un permis qui expire le jour même est encore valable ce jour-là.
    changer_conducteur(&pool, conducteur, "Durand", "actif", "2026-06-01").await;
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .expect("permis encore valable le jour de son expiration");
}

#[sqlx::test]
async fn cloturer_et_corriger_restent_possibles_apres_coup(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "CL-111-CL").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let affectation = affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .unwrap();

    // Depuis, le véhicule a été réformé et le conducteur a quitté l'entreprise.
    changer_statut_vehicule(&pool, vehicule, "CL-111-CL", "reforme").await;
    changer_conducteur(&pool, conducteur, "Durand", "inactif", "2020-01-01").await;

    // Corriger la date de début d'une affectation existante passe toujours.
    let mut saisie = saisie_affectation(vehicule, conducteur, "");
    saisie.date_debut = "2026-01-15".to_string();
    let corrigee = affectations::modifier(&pool, affectation.id, &saisie, aujourd_hui())
        .await
        .expect("corriger une date n'est pas affecter");
    assert_eq!(corrigee.date_debut_fr(), "15/01/2026");

    // La clôturer aussi : c'est justement ce qu'il faut faire.
    let cloturee = affectations::cloturer(&pool, affectation.id, aujourd_hui())
        .await
        .expect("clôture acceptée");
    assert!(!cloturee.est_active());

    // La rouvrir, en revanche, revient à affecter : refus.
    let erreur = affectations::modifier(&pool, affectation.id, &saisie, aujourd_hui())
        .await
        .expect_err("rouvrir une affectation rejoue les règles");
    assert!(matches!(erreur, ErreurApp::Metier(_)), "{erreur:?}");
}

#[sqlx::test]
async fn changer_de_vehicule_ou_de_conducteur_rejoue_les_regles(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "CH-111-CH").await;
    let reforme = creer_vehicule(&pool, "CH-222-CH").await;
    let libre = creer_vehicule(&pool, "CH-333-CH").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let inactif = creer_conducteur(&pool, "Lefebvre").await;
    changer_statut_vehicule(&pool, reforme, "CH-222-CH", "reforme").await;
    changer_conducteur(&pool, inactif, "Lefebvre", "inactif", "2031-01-01").await;

    let affectation = affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .unwrap();

    let vers_reforme = saisie_affectation(reforme, conducteur, "");
    let erreur = affectations::modifier(&pool, affectation.id, &vers_reforme, aujourd_hui())
        .await
        .expect_err("déplacer vers un véhicule réformé est refusé");
    assert!(matches!(erreur, ErreurApp::Metier(_)), "{erreur:?}");

    let vers_inactif = saisie_affectation(vehicule, inactif, "");
    let erreur = affectations::modifier(&pool, affectation.id, &vers_inactif, aujourd_hui())
        .await
        .expect_err("passer à un conducteur inactif est refusé");
    assert!(matches!(erreur, ErreurApp::Metier(_)), "{erreur:?}");

    let vers_libre = saisie_affectation(libre, conducteur, "");
    let deplacee = affectations::modifier(&pool, affectation.id, &vers_libre, aujourd_hui())
        .await
        .expect("déplacer vers un véhicule en service est accepté");
    assert_eq!(deplacee.immatriculation, "CH-333-CH");
}

#[sqlx::test]
async fn cloturer_puis_reaffecter(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "CC-333-CC").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let saisie = saisie_affectation(vehicule, conducteur, "");

    let active = affectations::creer(&pool, &saisie, aujourd_hui())
        .await
        .unwrap();
    assert!(active.est_active());

    let cloturee = affectations::cloturer(
        &pool,
        active.id,
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
    )
    .await
    .unwrap();
    assert!(!cloturee.est_active());
    assert_eq!(cloturee.date_fin_fr(), "01/04/2026");

    // Clôturer deux fois est une erreur métier, pas une erreur technique.
    let erreur = affectations::cloturer(
        &pool,
        active.id,
        NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
    )
    .await
    .unwrap_err();
    assert!(matches!(erreur, ErreurApp::Metier(_)));

    // Une fois l'affectation clôturée, le même conducteur peut reprendre le même véhicule.
    affectations::creer(&pool, &saisie, aujourd_hui())
        .await
        .expect("réaffectation acceptée après clôture");

    let liste = affectations::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(liste.len(), 2);
}

#[sqlx::test]
async fn modifier_sa_propre_affectation_en_cours_n_est_pas_un_doublon(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "DD-444-DD").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let active = affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .unwrap();

    let mut saisie = saisie_affectation(vehicule, conducteur, "");
    saisie.date_debut = "2026-01-15".to_string();
    let modifiee = affectations::modifier(&pool, active.id, &saisie, aujourd_hui())
        .await
        .expect("modifier sa propre affectation en cours ne viole pas la règle");
    assert_eq!(modifiee.date_debut_fr(), "15/01/2026");
}

#[sqlx::test]
async fn supprimer_un_vehicule_emporte_ses_affectations(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EE-555-EE").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .unwrap();

    gestion_parc_auto::metier::vehicules::supprimer(
        &pool,
        &commun::dossier_documents_de_test(),
        vehicule,
    )
    .await
    .unwrap();

    assert!(affectations::lister(&pool).await.unwrap().is_empty());
}
