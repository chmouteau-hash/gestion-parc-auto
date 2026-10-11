//! Règle « une seule affectation active par véhicule », prouvée aux deux niveaux :
//! 1. l'application refuse avec un message lisible ;
//! 2. la base refuse même quand on contourne l'application (SQL direct).

mod commun;

use chrono::NaiveDate;
use gestion_parc_auto::erreurs::{a_le_code, contrainte_violee, ErreurApp, CODE_VIOLATION_UNICITE};
use gestion_parc_auto::metier::affectations;
use sqlx::PgPool;

use commun::{creer_conducteur, creer_vehicule, saisie_affectation};

#[sqlx::test]
async fn l_application_refuse_une_seconde_affectation_active(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AA-111-AA").await;
    let premier = creer_conducteur(&pool, "Durand").await;
    let second = creer_conducteur(&pool, "Lefebvre").await;

    affectations::creer(&pool, &saisie_affectation(vehicule, premier, ""))
        .await
        .expect("première affectation acceptée");

    let erreur = affectations::creer(&pool, &saisie_affectation(vehicule, second, ""))
        .await
        .expect_err("seconde affectation active refusée");
    match erreur {
        ErreurApp::Metier(message) => {
            assert!(message.contains("AA-111-AA"), "message : {message}");
            assert!(message.contains("Camille Durand"), "message : {message}");
        }
        autre => panic!("erreur inattendue : {autre:?}"),
    }

    // Une affectation déjà terminée ne gêne pas : l'historique peut contenir plusieurs lignes.
    affectations::creer(&pool, &saisie_affectation(vehicule, second, "2026-02-01"))
        .await
        .expect("affectation terminée acceptée");
}

#[sqlx::test]
async fn la_base_refuse_meme_en_contournant_l_application(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "BB-222-BB").await;
    let premier = creer_conducteur(&pool, "Durand").await;
    let second = creer_conducteur(&pool, "Lefebvre").await;

    affectations::creer(&pool, &saisie_affectation(vehicule, premier, ""))
        .await
        .unwrap();

    // Insertion directe, sans passer par la vérification applicative.
    let erreur = sqlx::query(
        "INSERT INTO affectations (vehicule_id, conducteur_id, date_debut) VALUES ($1, $2, $3)",
    )
    .bind(vehicule)
    .bind(second)
    .bind(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap())
    .execute(&pool)
    .await
    .expect_err("l'index unique partiel doit refuser");

    assert!(a_le_code(&erreur, CODE_VIOLATION_UNICITE), "{erreur}");
    assert_eq!(
        contrainte_violee(&erreur).as_deref(),
        Some("affectations_une_seule_active_par_vehicule")
    );
}

#[sqlx::test]
async fn cloturer_puis_reaffecter(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "CC-333-CC").await;
    let premier = creer_conducteur(&pool, "Durand").await;
    let second = creer_conducteur(&pool, "Lefebvre").await;

    let active = affectations::creer(&pool, &saisie_affectation(vehicule, premier, ""))
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

    let nouvelle = affectations::creer(&pool, &saisie_affectation(vehicule, second, ""))
        .await
        .expect("le véhicule est libre");
    assert_eq!(nouvelle.conducteur_nom, "Lefebvre");

    let liste = affectations::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(liste.len(), 2);
}

#[sqlx::test]
async fn modifier_sans_changer_l_affectation_active_est_permis(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "DD-444-DD").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let active = affectations::creer(&pool, &saisie_affectation(vehicule, conducteur, ""))
        .await
        .unwrap();

    let mut saisie = saisie_affectation(vehicule, conducteur, "");
    saisie.date_debut = "2026-01-15".to_string();
    let modifiee = affectations::modifier(&pool, active.id, &saisie)
        .await
        .expect("modifier sa propre affectation active ne viole pas la règle");
    assert_eq!(modifiee.date_debut_fr(), "15/01/2026");
}

#[sqlx::test]
async fn supprimer_un_vehicule_emporte_ses_affectations(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EE-555-EE").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    affectations::creer(&pool, &saisie_affectation(vehicule, conducteur, ""))
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
