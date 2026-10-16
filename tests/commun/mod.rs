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
use chrono::NaiveDate;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

use gestion_parc_auto::commun::types::Profil;
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
        profil: Profil::Entreprise,
    }
}

pub fn application(pool: PgPool) -> Router {
    application_avec_profil(pool, Profil::Entreprise)
}

/// La même application, réglée comme une installation du profil donné.
pub fn application_avec_profil(pool: PgPool, profil: Profil) -> Router {
    let config = Config {
        profil,
        ..config_de_test()
    };
    gestion_parc_auto::web::routeur(Etat::nouveau(pool, config).expect("état"))
}

pub fn saisie_vehicule(immatriculation: &str) -> SaisieVehicule {
    SaisieVehicule {
        immatriculation: immatriculation.to_string(),
        couleur: String::new(),
        marque: "Renault".to_string(),
        modele: "Kangoo".to_string(),
        date_mise_en_service: "2023-06-01".to_string(),
        carburant: "diesel".to_string(),
        kilometrage: "12000".to_string(),
        statut: "en_service".to_string(),
        vin: String::new(),
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

/// Passe un véhicule de test au statut donné (`reforme`, `en_maintenance`…).
pub async fn changer_statut_vehicule(pool: &PgPool, id: i64, immatriculation: &str, statut: &str) {
    let saisie = SaisieVehicule {
        statut: statut.to_string(),
        ..saisie_vehicule(immatriculation)
    };
    metier::vehicules::modifier(pool, id, &saisie, aujourd_hui())
        .await
        .expect("modification du véhicule");
}

/// Change le statut et la date d'expiration du permis d'un conducteur de test.
pub async fn changer_conducteur(pool: &PgPool, id: i64, nom: &str, statut: &str, permis: &str) {
    let saisie = SaisieConducteur {
        statut: statut.to_string(),
        date_expiration_permis: permis.to_string(),
        ..saisie_conducteur(nom)
    };
    metier::conducteurs::modifier(pool, id, &saisie, aujourd_hui())
        .await
        .expect("modification du conducteur");
}

/// « Aujourd'hui » pour les règles d'affectation : une date fixe, pour que les tests donnent le
/// même résultat quel que soit le jour où ils tournent. Le permis des conducteurs de test
/// (2031) est valide à cette date.
pub fn aujourd_hui() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
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

/// Envoie le formulaire de dépôt de document à l'adresse donnée (`/vehicules/{id}/documents` ou
/// `/conducteurs/{id}/documents`) : un PDF, déclaré comme facture.
pub async fn post_fichier(
    app: &Router,
    cookie: &str,
    chemin: &str,
    nom_fichier: &str,
    contenu: &[u8],
) -> Response<Body> {
    post_multipart(
        app,
        cookie,
        chemin,
        &[("type_document", "facture")],
        &[(nom_fichier, "application/pdf", contenu)],
    )
    .await
}

/// Envoie un formulaire `multipart/form-data` comme le fait un navigateur : les champs texte,
/// puis un champ `fichier` par fichier (nom, type MIME, contenu). Un champ de fichier laissé
/// vide par l'utilisateur s'écrit `("", "application/octet-stream", b"")`.
pub async fn post_multipart(
    app: &Router,
    cookie: &str,
    chemin: &str,
    champs: &[(&str, &str)],
    fichiers: &[(&str, &str, &[u8])],
) -> Response<Body> {
    post_multipart_nomme(app, cookie, chemin, "fichier", champs, fichiers).await
}

/// Comme [`post_multipart`], avec le nom du champ de fichier (`photo` pour le formulaire
/// « Nouveau véhicule »).
pub async fn post_multipart_nomme(
    app: &Router,
    cookie: &str,
    chemin: &str,
    champ_fichier: &str,
    champs: &[(&str, &str)],
    fichiers: &[(&str, &str, &[u8])],
) -> Response<Body> {
    const LIMITE: &str = "limite-de-test-7f3a";
    let mut corps = Vec::new();
    for (nom, valeur) in champs {
        corps.extend_from_slice(
            format!(
                "--{LIMITE}\r\nContent-Disposition: form-data; name=\"{nom}\"\r\n\r\n{valeur}\r\n"
            )
            .as_bytes(),
        );
    }
    for (nom_fichier, type_mime, contenu) in fichiers {
        corps.extend_from_slice(
            format!(
                "--{LIMITE}\r\nContent-Disposition: form-data; name=\"{champ_fichier}\"; \
                 filename=\"{nom_fichier}\"\r\nContent-Type: {type_mime}\r\n\r\n"
            )
            .as_bytes(),
        );
        corps.extend_from_slice(contenu);
        corps.extend_from_slice(b"\r\n");
    }
    corps.extend_from_slice(format!("--{LIMITE}--\r\n").as_bytes());

    let requete = Request::post(chemin)
        .header(header::COOKIE, cookie)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={LIMITE}"),
        )
        .body(Body::from(corps))
        .unwrap();
    app.clone().oneshot(requete).await.unwrap()
}

pub async fn texte(reponse: Response<Body>) -> String {
    let octets = reponse.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(octets.to_vec()).unwrap()
}
