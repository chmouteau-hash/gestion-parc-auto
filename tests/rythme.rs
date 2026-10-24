//! Le rythme et les compteurs à relever (#94, bloc 4) : le rythme des trois derniers mois de
//! l'historique donne une date aux bornes en kilomètres du plan d'entretien ; un compteur relevé
//! il y a plus d'un mois se signale sur la page Alertes et sur la carte d'Autoclair.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local, NaiveDate};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::plan_entretien::{self, SaisiePlan};
use gestion_parc_auto::metier::releves::{self, SaisieReleve};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, changer_statut_vehicule, creer_utilisateur,
    creer_vehicule, get, post_formulaire, requete_json, se_connecter, texte,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn il_y_a(jours: i64) -> NaiveDate {
    aujourd_hui() - Duration::days(jours)
}

/// Le relevé de la création du véhicule, redaté et corrigé : le point de départ de l'historique.
async fn redater_la_creation(pool: &PgPool, vehicule: i64, date: NaiveDate, kilometrage: i32) {
    let id: i64 = sqlx::query_scalar("SELECT id FROM releves_kilometrage WHERE vehicule_id = $1")
        .bind(vehicule)
        .fetch_one(pool)
        .await
        .unwrap();
    releves::modifier(
        pool,
        id,
        &SaisieReleve {
            date_releve: date.to_string(),
            kilometrage: kilometrage.to_string(),
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn le_rythme_donne_une_date_aux_bornes_en_kilometres(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "RY-100-AN").await;
    // 9 000 km il y a 60 jours, 12 000 aujourd'hui : 50 km par jour.
    redater_la_creation(&pool, vehicule, il_y_a(60), 9_000).await;
    releves::relever(&pool, vehicule, "12000", aujourd_hui())
        .await
        .unwrap();
    // La vidange tous les 15 000 km, faite il y a 30 jours à 10 500 : la prochaine à 25 500, dans
    // 13 500 km, soit 270 jours au rythme actuel.
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "15000".into(),
            derniere_date: il_y_a(30).to_string(),
            derniere_kilometrage: "10500".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let historique = releves::historique(&pool, vehicule).await.unwrap();
    let rythme = releves::rythme(&historique, aujourd_hui()).expect("un rythme");
    assert!((rythme.km_par_jour - 50.0).abs() < 1e-9);
    let date = rythme.date_dans(13_500, aujourd_hui()).unwrap();
    assert_eq!(date, aujourd_hui() + Duration::days(270));
    let estimation = releves::estimation_fr(date, aujourd_hui());

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La fiche : la borne en kilomètres dit quand elle sera atteinte.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(
            "25\u{202F}500 km · dans 13\u{202F}500 km, {estimation}"
        )),
        "{fiche}"
    );

    // L'historique du compteur dit le rythme.
    let page = texte(
        get(
            &app,
            &format!("/vehicules/{vehicule}/compteur"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains("Rythme : environ 1\u{202F}520 km par mois, sur les trois derniers mois."),
        "{page}"
    );

    // L'API donne la date estimée.
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/plan-entretien"),
        Some(&cookie),
        None,
    )
    .await;
    let plan: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(plan[0]["date_estimee"], date.to_string());
}

#[sqlx::test]
async fn sans_rythme_pas_de_date_estimee(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    // Un seul point dans les trois derniers mois : le relevé de la création.
    let vehicule = creer_vehicule(&pool, "RY-200-AN").await;
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "15000".into(),
            derniere_date: il_y_a(200).to_string(),
            derniere_kilometrage: "10000".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains("25\u{202F}000 km · dans 13\u{202F}000 km<"),
        "{fiche}"
    );
    assert!(!fiche.contains("au rythme actuel"), "{fiche}");
    let page = texte(
        get(
            &app,
            &format!("/vehicules/{vehicule}/compteur"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(!page.contains("Rythme :"), "{page}");
}

#[sqlx::test]
async fn un_compteur_releve_il_y_a_plus_d_un_mois_est_a_relever(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let ancien = creer_vehicule(&pool, "RY-300-AN").await;
    redater_la_creation(&pool, ancien, il_y_a(45), 12_000).await;
    let recent = creer_vehicule(&pool, "RY-301-AN").await;
    let reforme = creer_vehicule(&pool, "RY-302-AN").await;
    redater_la_creation(&pool, reforme, il_y_a(90), 12_000).await;
    changer_statut_vehicule(&pool, reforme, "RY-302-AN", "reforme").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;

    // La page Alertes : une section à part, qui ne compte ni dans les dépassées ni dans les à venir.
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Aucune alerte"), "{page}");
    assert!(page.contains("<h2>Compteurs à relever</h2>"), "{page}");
    assert!(
        page.contains(&format!(
            r#"<a href="/vehicules/{ancien}">Véhicule RY-300-AN</a>"#
        )),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            "le {} · il y a 45 jours",
            il_y_a(45).format("%d/%m/%Y")
        )),
        "{page}"
    );
    assert!(
        !page.contains("RY-301-AN") && !page.contains("RY-302-AN"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"action="/vehicules/{ancien}/kilometrage?retour=alertes""#
        )),
        "{page}"
    );
    // Le lecteur voit la section, sans le bouton.
    let page = texte(get(&app, "/alertes", Some(&lecteur)).await).await;
    assert!(page.contains("Véhicule RY-300-AN"), "{page}");
    assert!(!page.contains("retour=alertes"), "{page}");

    // La carte d'Autoclair le dit aussi.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_p = se_connecter(&particulier, "gestion").await;
    let accueil = texte(get(&particulier, "/", Some(&cookie_p)).await).await;
    assert!(
        accueil.contains("<strong>Relever le compteur</strong> : dernier relevé il y a 45 jours."),
        "{accueil}"
    );
    assert_eq!(
        accueil.matches("Relever le compteur").count(),
        1,
        "{accueil}"
    );

    // Relevé depuis la page Alertes : retour à la page, le véhicule n'y est plus.
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{ancien}/kilometrage?retour=alertes"),
        Some(&cookie),
        "kilometrage=13000",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/alertes");
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(!page.contains("Compteurs à relever"), "{page}");
    let _ = recent;
}
