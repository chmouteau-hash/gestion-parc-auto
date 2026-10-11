//! Tests HTTP de bout en bout : le routeur réel est appelé en mémoire, sans ouvrir de port.
//! Ils prouvent l'authentification, le contrôle des rôles, les en-têtes de sécurité et l'API.

mod commun;

use axum::http::{header, StatusCode};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, requete_json,
    se_connecter, texte,
};

#[sqlx::test]
async fn sans_session_les_pages_redirigent_vers_la_connexion(pool: PgPool) {
    let app = application(pool);
    for chemin in [
        "/alertes",
        "/vehicules",
        "/conducteurs/1",
        "/affectations/nouvelle",
    ] {
        let reponse = get(&app, chemin, None).await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{chemin}");
        assert_eq!(
            reponse.headers()[header::LOCATION],
            "/connexion",
            "{chemin}"
        );
    }
    // La racine renvoie vers la vue alertes, qui exige elle-même la connexion.
    let reponse = get(&app, "/", None).await;
    assert_eq!(reponse.headers()[header::LOCATION], "/alertes");
    // Et l'API répond 401 en JSON plutôt que de rediriger.
    let reponse = requete_json(&app, "GET", "/api/v1/vehicules", None, None).await;
    assert_eq!(reponse.status(), StatusCode::UNAUTHORIZED);
    assert!(texte(reponse).await.contains("Connexion requise"));
}

#[sqlx::test]
async fn connexion_refusee_puis_acceptee(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool);

    let reponse =
        post_formulaire(&app, "/connexion", None, "login=gestion&mot_de_passe=faux").await;
    assert_eq!(
        reponse.status(),
        StatusCode::OK,
        "le formulaire est ré-affiché"
    );
    assert!(texte(reponse)
        .await
        .contains("Identifiant ou mot de passe incorrect"));

    let cookie = se_connecter(&app, "gestion").await;
    assert!(cookie.starts_with("session="));

    let reponse = get(&app, "/vehicules", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("Aucun véhicule enregistré"));
    assert!(
        page.contains("Nouveau véhicule"),
        "un administrateur voit le bouton"
    );
}

#[sqlx::test]
async fn un_lecteur_ne_peut_pas_ecrire(pool: PgPool) {
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "lecture").await;

    let reponse = get(&app, "/vehicules", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(
        !texte(reponse).await.contains("Nouveau véhicule"),
        "le bouton est masqué"
    );

    // Le masquage ne suffit pas : la route elle-même refuse.
    let reponse = post_formulaire(
        &app,
        "/vehicules",
        Some(&cookie),
        "immatriculation=XX-1&marque=A&modele=B&date_mise_en_service=2024-01-01&carburant=essence&kilometrage=1&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);

    let reponse = get(&app, "/vehicules/nouveau", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);

    let reponse = requete_json(&app, "DELETE", "/api/v1/vehicules/1", Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn un_administrateur_cree_un_vehicule_par_formulaire(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/vehicules",
        Some(&cookie),
        "immatriculation=fo-123-rm&marque=Peugeot&modele=Partner&date_mise_en_service=2024-01-01&carburant=diesel&kilometrage=5000&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(fiche.starts_with("/vehicules/"));

    let reponse = get(&app, &fiche, Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("FO-123-RM"));
    assert!(page.contains("Peugeot Partner"));

    // Une saisie invalide ré-affiche le formulaire avec le message, sans créer de ligne.
    let reponse = post_formulaire(
        &app,
        "/vehicules",
        Some(&cookie),
        "immatriculation=FO-123-RM&marque=Peugeot&modele=Partner&date_mise_en_service=2024-01-01&carburant=diesel&kilometrage=5000&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse).await.contains("déjà utilisée"));
    assert_eq!(
        gestion_parc_auto::metier::vehicules::lister(&pool)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn api_json_cycle_complet(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/vehicules",
        Some(&cookie),
        Some(
            r#"{"immatriculation":"api-1","marque":"Citroën","modele":"Berlingo","date_mise_en_service":"2024-02-02","carburant":"electrique","kilometrage":"100","statut":"en_service"}"#,
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let corps: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(corps["immatriculation"], "API-1");
    let id = corps["id"].as_i64().unwrap();

    let reponse = requete_json(&app, "GET", "/api/v1/vehicules", Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let liste: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(liste.as_array().unwrap().len(), 1);

    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/vehicules/{id}"),
        Some(&cookie),
        Some(r#"{"immatriculation":"API-1","marque":"Citroën","modele":"Berlingo","date_mise_en_service":"2024-02-02","carburant":"electrique","kilometrage":"-3","statut":"en_service"}"#),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert!(texte(reponse).await.contains("kilométrage"));

    let reponse = requete_json(
        &app,
        "DELETE",
        &format!("/api/v1/vehicules/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);

    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/vehicules/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn en_tetes_de_securite_et_page_introuvable(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = get(&app, "/alertes", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let entetes = reponse.headers();
    assert_eq!(entetes[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(entetes[header::X_FRAME_OPTIONS], "DENY");
    assert!(entetes[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .contains("default-src 'self'"));

    let reponse = get(&app, "/vehicules/999999", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);

    let reponse = get(&app, "/nimporte-quoi", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn deconnexion_invalide_le_cookie(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_vehicule(&pool, "DC-1").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(&app, "/deconnexion", Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let effacement = reponse.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        effacement.contains("Max-Age=0") || effacement.contains("Expires="),
        "{effacement}"
    );

    // Le navigateur a oublié le cookie : la page redirige à nouveau.
    let reponse = get(&app, "/vehicules", None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
}
