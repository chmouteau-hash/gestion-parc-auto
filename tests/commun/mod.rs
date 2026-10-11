//! Outils partagés par les tests d'intégration.
//!
//! Chaque test marqué `#[sqlx::test]` reçoit un `PgPool` sur une base **jetable** : SQLx la crée
//! à partir de `DATABASE_URL`, y applique les migrations de `migrations/`, puis la supprime à la
//! fin du test. Les tests ne se voient donc jamais entre eux et n'altèrent jamais la base de
//! développement.

#![allow(dead_code)]

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

use gestion_parc_auto::config::Config;
use gestion_parc_auto::etat::Etat;
use gestion_parc_auto::metier;
use gestion_parc_auto::metier::affectations::SaisieAffectation;
use gestion_parc_auto::metier::conducteurs::SaisieConducteur;
use gestion_parc_auto::metier::vehicules::SaisieVehicule;

/// Dossier de documents propre à chaque exécution de test, dans le dossier temporaire du système.
pub fn dossier_documents_de_test() -> PathBuf {
    std::env::temp_dir().join(format!("gestion-parc-auto-test-{}", uuid::Uuid::new_v4()))
}

pub fn config_de_test() -> Config {
    Config {
        database_url: String::new(),
        session_secret: "x".repeat(64),
        documents_dir: dossier_documents_de_test(),
        port: 0,
        cookies_securises: false,
        alertes_horizon_jours: 30,
        conservation_conducteurs_inactifs_jours: 365,
    }
}

pub fn application(pool: PgPool) -> Router {
    gestion_parc_auto::web::routeur(Etat::nouveau(pool, config_de_test()).expect("état"))
}

pub fn saisie_vehicule(immatriculation: &str) -> SaisieVehicule {
    SaisieVehicule {
        immatriculation: immatriculation.to_string(),
        marque: "Renault".to_string(),
        modele: "Kangoo".to_string(),
        date_mise_en_service: "2023-06-01".to_string(),
        carburant: "diesel".to_string(),
        kilometrage: "12000".to_string(),
        statut: "en_service".to_string(),
    }
}

pub fn saisie_conducteur(nom: &str) -> SaisieConducteur {
    SaisieConducteur {
        nom: nom.to_string(),
        prenom: "Camille".to_string(),
        service: "Logistique".to_string(),
        date_expiration_permis: "2031-01-01".to_string(),
        statut: "actif".to_string(),
    }
}

pub async fn creer_vehicule(pool: &PgPool, immatriculation: &str) -> i64 {
    metier::vehicules::creer(pool, &saisie_vehicule(immatriculation))
        .await
        .expect("création du véhicule")
        .id
}

pub async fn creer_conducteur(pool: &PgPool, nom: &str) -> i64 {
    metier::conducteurs::creer(pool, &saisie_conducteur(nom))
        .await
        .expect("création du conducteur")
        .id
}

pub fn saisie_affectation(
    vehicule_id: i64,
    conducteur_id: i64,
    date_fin: &str,
) -> SaisieAffectation {
    SaisieAffectation {
        vehicule_id: vehicule_id.to_string(),
        conducteur_id: conducteur_id.to_string(),
        date_debut: "2026-01-10".to_string(),
        date_fin: date_fin.to_string(),
    }
}

pub async fn creer_utilisateur(pool: &PgPool, login: &str, role: &str) {
    metier::utilisateurs::creer(pool, login, "mot-de-passe-de-test", role)
        .await
        .expect("création de l'utilisateur");
}

/// Se connecte via `POST /connexion` et renvoie la valeur de l'en-tête `Cookie` à renvoyer.
pub async fn se_connecter(app: &Router, login: &str) -> String {
    let reponse = app
        .clone()
        .oneshot(
            Request::post("/connexion")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "login={login}&mot_de_passe=mot-de-passe-de-test"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "connexion refusée");
    let set_cookie = reponse
        .headers()
        .get(header::SET_COOKIE)
        .expect("cookie de session")
        .to_str()
        .unwrap();
    // On ne garde que `nom=valeur`, sans les attributs (Path, HttpOnly…).
    set_cookie.split(';').next().unwrap().to_string()
}

pub async fn get(app: &Router, chemin: &str, cookie: Option<&str>) -> Response<Body> {
    let mut requete = Request::get(chemin);
    if let Some(cookie) = cookie {
        requete = requete.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(requete.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

pub async fn post_formulaire(
    app: &Router,
    chemin: &str,
    cookie: Option<&str>,
    corps: &str,
) -> Response<Body> {
    let mut requete =
        Request::post(chemin).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        requete = requete.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(requete.body(Body::from(corps.to_string())).unwrap())
        .await
        .unwrap()
}

pub async fn requete_json(
    app: &Router,
    methode: &str,
    chemin: &str,
    cookie: Option<&str>,
    corps: Option<&str>,
) -> Response<Body> {
    let mut requete = Request::builder()
        .method(methode)
        .uri(chemin)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        requete = requete.header(header::COOKIE, cookie);
    }
    let corps = corps.map(|c| Body::from(c.to_string())).unwrap_or_default();
    app.clone()
        .oneshot(requete.body(corps).unwrap())
        .await
        .unwrap()
}

pub async fn texte(reponse: Response<Body>) -> String {
    let octets = reponse.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(octets.to_vec()).unwrap()
}
