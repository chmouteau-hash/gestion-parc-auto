//! Tests HTTP de bout en bout : le routeur réel est appelé en mémoire, sans ouvrir de port.
//! Ils prouvent l'authentification, le contrôle des rôles, les en-têtes de sécurité et l'API.

mod commun;

use axum::http::{header, StatusCode};
use sqlx::PgPool;

use commun::{
    application, changer_conducteur, changer_statut_vehicule, creer_conducteur, creer_utilisateur,
    creer_vehicule, get, post_formulaire, requete_json, se_connecter, texte,
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

#[sqlx::test]
async fn le_formulaire_d_affectation_applique_les_regles(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let partage = creer_vehicule(&pool, "PA-111-PA").await;
    let reforme = creer_vehicule(&pool, "RE-222-RE").await;
    changer_statut_vehicule(&pool, reforme, "RE-222-RE", "reforme").await;
    let durand = creer_conducteur(&pool, "Durand").await;
    let lefebvre = creer_conducteur(&pool, "Lefebvre").await;
    let parti = creer_conducteur(&pool, "Moreau").await;
    changer_conducteur(&pool, parti, "Moreau", "inactif", "2031-01-01").await;
    // Un permis expiré depuis des années : le test ne dépend pas du jour où il tourne.
    let bernard = creer_conducteur(&pool, "Bernard").await;
    let sans_permis = creer_conducteur(&pool, "Petit").await;
    changer_conducteur(&pool, sans_permis, "Petit", "actif", "2020-01-01").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire grise ce qui ne peut pas être choisi, et dit pourquoi.
    let page = texte(get(&app, "/affectations/nouvelle", Some(&cookie)).await).await;
    for attendu in [
        format!(
            r#"<option value="{reforme}" disabled>RE-222-RE (Renault Kangoo) (réformé)</option>"#
        ),
        format!(r#"<option value="{parti}" disabled>Camille Moreau (inactif)</option>"#),
        format!(r#"<option value="{sans_permis}" disabled>Camille Petit (permis expiré)</option>"#),
        format!(r#"<option value="{partage}">PA-111-PA (Renault Kangoo)</option>"#),
        format!(r#"<option value="{durand}">Camille Durand</option>"#),
    ] {
        assert!(page.contains(&attendu), "absent : {attendu}\n{page}");
    }

    // Griser ne suffit pas : la route elle-même refuse et ré-affiche le formulaire.
    let corps = |vehicule: i64, conducteur: i64| {
        format!("vehicule_id={vehicule}&conducteur_id={conducteur}&date_debut=2026-01-10&date_fin=")
    };
    for (vehicule, conducteur, message) in [
        (reforme, durand, "est réformé"),
        (partage, parti, "a le statut « Inactif »"),
        (
            partage,
            sans_permis,
            "a un permis expiré depuis le 01/01/2020",
        ),
    ] {
        let reponse = post_formulaire(
            &app,
            "/affectations",
            Some(&cookie),
            &corps(vehicule, conducteur),
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::OK, "{message}");
        let page = texte(reponse).await;
        assert!(page.contains(message), "absent : {message}\n{page}");
    }

    // Deux conducteurs sur le même véhicule : accepté, et la fiche les montre tous les deux.
    for conducteur in [durand, lefebvre] {
        let reponse = post_formulaire(
            &app,
            "/affectations",
            Some(&cookie),
            &corps(partage, conducteur),
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    }
    let fiche = texte(get(&app, &format!("/vehicules/{partage}"), Some(&cookie)).await).await;
    assert!(fiche.contains("Conducteurs actuels"), "{fiche}");
    assert!(fiche.contains("Camille Durand"));
    assert!(fiche.contains("Camille Lefebvre"));
    assert!(fiche.contains("Affecter un autre conducteur"));

    // Une affectation avec une date de fin lointaine est en cours elle aussi : son conducteur
    // figure parmi les conducteurs actuels, avec sa période.
    let reponse = post_formulaire(
        &app,
        "/affectations",
        Some(&cookie),
        &format!(
            "vehicule_id={partage}&conducteur_id={bernard}&date_debut=2026-01-10&date_fin=2099-12-31"
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = texte(get(&app, &format!("/vehicules/{partage}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains("Camille Bernard</a> du 10/01/2026 au 31/12/2099"),
        "{fiche}"
    );
    // La liste la présente « En cours » et propose de la terminer.
    let liste = texte(get(&app, "/affectations", Some(&cookie)).await).await;
    assert_eq!(liste.matches("etat-en_cours").count(), 3, "{liste}");
    assert_eq!(liste.matches("Terminer aujourd").count(), 3, "{liste}");

    // Les fiches ne proposent pas une affectation qui serait refusée.
    let fiche = texte(get(&app, &format!("/vehicules/{reforme}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("/affectations/nouvelle"), "{fiche}");
    let fiche = texte(get(&app, &format!("/conducteurs/{parti}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("/affectations/nouvelle"), "{fiche}");
    let fiche = texte(get(&app, &format!("/conducteurs/{durand}"), Some(&cookie)).await).await;
    assert!(fiche.contains("/affectations/nouvelle"), "{fiche}");

    // L'API applique les mêmes règles.
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/affectations",
        Some(&cookie),
        Some(&format!(
            r#"{{"vehicule_id":"{reforme}","conducteur_id":"{durand}","date_debut":"2026-01-10"}}"#
        )),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert!(texte(reponse).await.contains("réformé"));

    // Réformer le véhicule partagé par le formulaire met fin à ses trois affectations, y compris
    // celle qui était prévue jusqu'en 2099. L'effet est visible tout de suite.
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{partage}"),
        Some(&cookie),
        "immatriculation=PA-111-PA&marque=Renault&modele=Kangoo&date_mise_en_service=2023-06-01&carburant=diesel&kilometrage=12000&statut=reforme",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = texte(get(&app, &format!("/vehicules/{partage}"), Some(&cookie)).await).await;
    assert!(fiche.contains("<em>aucun</em>"), "{fiche}");
    assert!(!fiche.contains("etat-en_cours"), "{fiche}");
    assert_eq!(fiche.matches("etat-terminee").count(), 3, "{fiche}");
    assert!(!fiche.contains("/affectations/nouvelle"), "{fiche}");
}
