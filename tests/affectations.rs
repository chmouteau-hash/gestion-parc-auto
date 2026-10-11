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

/// Date de fin de chaque affectation d'un véhicule, de la plus ancienne à la plus récente.
async fn fins_par_vehicule(pool: &PgPool, vehicule: i64) -> Vec<String> {
    let mut liste = affectations::lister_par_vehicule(pool, vehicule)
        .await
        .unwrap();
    liste.sort_by_key(|a| a.id);
    liste.iter().map(|a| a.date_fin_fr()).collect()
}

#[sqlx::test]
async fn reformer_un_vehicule_cloture_ses_affectations(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "RC-111-RC").await;
    let autre = creer_vehicule(&pool, "RC-222-RC").await;
    let durand = creer_conducteur(&pool, "Durand").await;
    let lefebvre = creer_conducteur(&pool, "Lefebvre").await;
    let moreau = creer_conducteur(&pool, "Moreau").await;

    // Sur le véhicule : une période passée, deux affectations en cours, une période qui devait
    // durer jusqu'à la fin de l'année. Sur l'autre véhicule : une affectation en cours.
    for (v, c, fin) in [
        (vehicule, durand, "2026-02-01"),
        (vehicule, durand, ""),
        (vehicule, lefebvre, ""),
        (vehicule, moreau, "2026-12-31"),
        (autre, durand, ""),
    ] {
        affectations::creer(&pool, &saisie_affectation(v, c, fin), aujourd_hui())
            .await
            .unwrap();
    }

    changer_statut_vehicule(&pool, vehicule, "RC-111-RC", "reforme").await;

    // La période passée ne bouge pas ; tout ce qui courait encore s'arrête le jour de la réforme
    // (1er juin 2026, le jour de test), y compris la période prévue jusqu'en décembre.
    assert_eq!(
        fins_par_vehicule(&pool, vehicule).await,
        ["01/02/2026", "01/06/2026", "01/06/2026", "01/06/2026"]
    );
    // L'historique est conservé, et l'autre véhicule n'est pas touché.
    assert_eq!(fins_par_vehicule(&pool, autre).await, ["en cours"]);
}

#[sqlx::test]
async fn desactiver_un_conducteur_cloture_ses_affectations(pool: PgPool) {
    let premier = creer_vehicule(&pool, "DC-111-DC").await;
    let second = creer_vehicule(&pool, "DC-222-DC").await;
    let partant = creer_conducteur(&pool, "Durand").await;
    let restant = creer_conducteur(&pool, "Lefebvre").await;

    for (v, c) in [(premier, partant), (second, partant), (premier, restant)] {
        affectations::creer(&pool, &saisie_affectation(v, c, ""), aujourd_hui())
            .await
            .unwrap();
    }

    changer_conducteur(&pool, partant, "Durand", "inactif", "2031-01-01").await;

    // Ses deux affectations sont terminées ; celle de son collègue sur le même véhicule court
    // toujours.
    assert_eq!(
        fins_par_vehicule(&pool, premier).await,
        ["01/06/2026", "en cours"]
    );
    assert_eq!(fins_par_vehicule(&pool, second).await, ["01/06/2026"]);

    // Le réactiver ne rouvre rien : il faudra l'affecter de nouveau.
    changer_conducteur(&pool, partant, "Durand", "actif", "2031-01-01").await;
    assert_eq!(fins_par_vehicule(&pool, second).await, ["01/06/2026"]);
}

#[sqlx::test]
async fn une_affectation_pas_encore_commencee_se_termine_le_jour_de_son_debut(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "FU-111-FU").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;

    // Affectation prévue pour juillet, alors que le véhicule est réformé le 1er juin.
    let mut saisie = saisie_affectation(vehicule, conducteur, "");
    saisie.date_debut = "2026-07-01".to_string();
    affectations::creer(&pool, &saisie, aujourd_hui())
        .await
        .unwrap();

    changer_statut_vehicule(&pool, vehicule, "FU-111-FU", "reforme").await;

    // La base interdit une fin antérieure au début : la clôture se cale sur le début.
    assert_eq!(fins_par_vehicule(&pool, vehicule).await, ["01/07/2026"]);
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

    // Depuis, le permis du conducteur a expiré. Rien ne clôture l'affectation à sa place :
    // un permis expire avec le temps, sans que personne ne modifie la fiche.
    changer_conducteur(&pool, conducteur, "Durand", "actif", "2026-03-01").await;

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

    // Corriger une affectation terminée, en gardant sa date de fin, passe aussi.
    saisie.date_debut = "2026-01-20".to_string();
    saisie.date_fin = "2026-06-01".to_string();
    affectations::modifier(&pool, affectation.id, &saisie, aujourd_hui())
        .await
        .expect("corriger l'historique reste possible");

    // La rouvrir, en revanche, revient à affecter : refus.
    saisie.date_fin = String::new();
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
