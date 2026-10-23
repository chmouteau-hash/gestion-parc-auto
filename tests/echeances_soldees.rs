//! Les gestes qui soldent (#94, bloc 3) : enregistrer un contrôle technique favorable solde
//! l'échéance « Contrôle technique » ouverte, une révision l'« Entretien périodique », et le
//! formulaire propose aussitôt la suivante, datée de la visite. La saisie du passé ne solde rien.

mod commun;

use axum::http::{header, StatusCode};
use axum::response::Response;
use chrono::{Duration, Local, Months, NaiveDate};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::plan_entretien::{self, SaisiePlan};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, post_formulaire, requete_json, se_connecter,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn dans(jours: i64) -> NaiveDate {
    aujourd_hui() + Duration::days(jours)
}

async fn echeance(pool: &PgPool, vehicule: i64, type_echeance: &str, date: NaiveDate) -> i64 {
    echeances::creer(
        pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: type_echeance.into(),
            date_limite: date.to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

async fn statut(pool: &PgPool, id: i64) -> String {
    echeances::obtenir(pool, id).await.unwrap().statut
}

/// Le formulaire « Nouvelle intervention » tel que le navigateur l'envoie (le véhicule de test
/// est à 12 000 km).
fn corps(vehicule: i64, type_intervention: &str, resultat: &str, date: NaiveDate) -> String {
    format!(
        "vehicule_id={vehicule}&type_intervention={type_intervention}&resultat_controle={resultat}\
         &date_intervention={date}&kilometrage=12000&cout=&prestataire=&notes="
    )
}

fn vers(reponse: &Response) -> String {
    reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string()
}

/// Le code du message posé par la redirection (cookie `fait`).
fn fait(reponse: &Response) -> String {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .find(|c| c.starts_with("fait="))
        .unwrap_or_default()
}

fn plus_mois(date: NaiveDate, mois: u32) -> NaiveDate {
    date.checked_add_months(Months::new(mois)).unwrap()
}

#[sqlx::test]
async fn un_controle_favorable_solde_l_echeance_et_propose_la_suivante(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-100-LD").await;
    let ct = echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let visite = dans(-2);
    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "controle_technique", "favorable", visite),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        vers(&reponse),
        format!(
            "/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=controle_technique&date_limite={}&retour=vehicule",
            plus_mois(visite, 24)
        )
    );
    assert!(
        fait(&reponse).starts_with("fait=intervention-solde-ct;"),
        "{}",
        fait(&reponse)
    );
    assert_eq!(statut(&pool, ct).await, "traitee");
}

#[sqlx::test]
async fn un_controle_defavorable_laisse_l_echeance_ouverte(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-110-LD").await;
    let ct = echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    for resultat in ["defavorable", "critique"] {
        let reponse = post_formulaire(
            &app,
            "/interventions",
            Some(&cookie),
            &corps(vehicule, "controle_technique", resultat, dans(-1)),
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
        assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
        assert!(fait(&reponse).starts_with("fait=intervention-creee;"));
        assert_eq!(statut(&pool, ct).await, "ouverte", "{resultat}");
    }
}

#[sqlx::test]
async fn une_revision_solde_l_entretien_periodique_l_entretien_courant_non(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-120-LD").await;
    let ep = echeance(&pool, vehicule, "entretien_periodique", dans(10)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "entretien_courant", "", dans(-3)),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert_eq!(statut(&pool, ep).await, "ouverte");

    let revision = dans(-1);
    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "revision", "", revision),
    )
    .await;
    assert_eq!(
        vers(&reponse),
        format!(
            "/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=entretien_periodique&date_limite={}&retour=vehicule",
            plus_mois(revision, 12)
        )
    );
    assert!(fait(&reponse).starts_with("fait=intervention-solde-ep;"));
    assert_eq!(statut(&pool, ep).await, "traitee");
}

#[sqlx::test]
async fn la_saisie_du_passe_et_une_date_future_ne_soldent_rien(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-130-LD").await;
    let ct = echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Un contrôle d'il y a trois ans, saisi pour mémoire, et celui d'où part l'échéance.
    for date in [dans(-3 * 365), dans(20) - Months::new(24)] {
        let reponse = post_formulaire(
            &app,
            "/interventions",
            Some(&cookie),
            &corps(vehicule, "controle_technique", "favorable", date),
        )
        .await;
        assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"), "{date}");
        assert_eq!(statut(&pool, ct).await, "ouverte", "{date}");
    }
    // Un contrôle daté dans le futur n'est pas fait.
    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "controle_technique", "favorable", dans(5)),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert_eq!(statut(&pool, ct).await, "ouverte");
}

#[sqlx::test]
async fn un_controle_en_retard_solde_et_la_suivante_part_de_la_visite(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-140-LD").await;
    let ct = echeance(&pool, vehicule, "controle_technique", dans(-40)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "controle_technique", "favorable", aujourd_hui()),
    )
    .await;
    assert!(
        vers(&reponse).contains(&format!("&date_limite={}&", plus_mois(aujourd_hui(), 24))),
        "{}",
        vers(&reponse)
    );
    assert_eq!(statut(&pool, ct).await, "traitee");
}

/// Décidé le 23/10/2026 (#94, bloc 2b) : le plan remplace l'« Entretien périodique », masqué par
/// le calcul et jamais modifié en base. Une révision ne le solde donc pas.
#[sqlx::test]
async fn sur_un_vehicule_qui_a_un_plan_la_revision_laisse_l_entretien_periodique(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-150-LD").await;
    let ep = echeance(&pool, vehicule, "entretien_periodique", dans(10)).await;
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "15000".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &(corps(vehicule, "revision", "", dans(-1)) + "&operation=vidange"),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert_eq!(statut(&pool, ep).await, "ouverte");
}

#[sqlx::test]
async fn l_api_solde_aussi_et_modifier_ne_solde_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SO-160-LD").await;
    let ct = echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Défavorable d'abord : rien n'est soldé. Le corriger en « favorable » ne solde pas non plus :
    // seul l'enregistrement d'une intervention solde.
    let reponse = post_formulaire(
        &app,
        "/interventions",
        Some(&cookie),
        &corps(vehicule, "controle_technique", "defavorable", dans(-2)),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let reponse = requete_json(&app, "GET", "/api/v1/interventions", Some(&cookie), None).await;
    let liste: serde_json::Value = serde_json::from_str(&commun::texte(reponse).await).unwrap();
    let id = liste[0]["id"].as_i64().unwrap();
    let reponse = post_formulaire(
        &app,
        &format!("/interventions/{id}"),
        Some(&cookie),
        &corps(vehicule, "controle_technique", "favorable", dans(-2)),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(statut(&pool, ct).await, "ouverte");

    // Par l'API, un contrôle favorable solde l'échéance ; la suivante, c'est au client de la créer.
    let json = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"controle_technique","resultat_controle":"favorable","date_intervention":"{}","kilometrage":"12000"}}"#,
        dans(-1)
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/interventions",
        Some(&cookie),
        Some(&json),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    assert_eq!(statut(&pool, ct).await, "traitee");
}
