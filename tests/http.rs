//! Tests HTTP de bout en bout : le routeur réel est appelé en mémoire, sans ouvrir de port.
//! Ils prouvent l'authentification, le contrôle des rôles, les en-têtes de sécurité et l'API.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::etat::Etat;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::utilisateurs;
use http_body_util::BodyExt;
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, aujourd_hui, changer_conducteur, changer_statut_vehicule,
    creer_conducteur, creer_utilisateur, creer_vehicule, get, post_fichier, post_formulaire,
    post_multipart, requete_json, saisie_affectation, saisie_conducteur, se_connecter, texte,
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
    // La racine est l'accueil du profil : elle exige la connexion comme les autres pages.
    let reponse = get(&app, "/", None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/connexion");
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
async fn un_mot_de_passe_oublie_se_remplace(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let connexion = |mot_de_passe: &str| format!("login=gestion&mot_de_passe={mot_de_passe}");

    // Trop court, ou pour un compte qui n'existe pas : refusé, et l'ancien reste valable.
    let erreur = utilisateurs::changer_mot_de_passe(&pool, "gestion", "court")
        .await
        .unwrap_err();
    assert!(
        erreur.to_string().contains("au moins 12 caractères"),
        "{erreur}"
    );
    let erreur = utilisateurs::changer_mot_de_passe(&pool, "inconnu", "un-nouveau-mot-de-passe")
        .await
        .unwrap_err();
    assert!(erreur.to_string().contains("« inconnu »"), "{erreur}");
    se_connecter(&app, "gestion").await;

    utilisateurs::changer_mot_de_passe(&pool, " gestion ", "un-nouveau-mot-de-passe")
        .await
        .unwrap();

    // L'ancien mot de passe ne passe plus ; le nouveau ouvre la session.
    let ancien = connexion("mot-de-passe-de-test");
    let reponse = post_formulaire(&app, "/connexion", None, &ancien).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse)
        .await
        .contains("Identifiant ou mot de passe incorrect"));
    let nouveau = connexion("un-nouveau-mot-de-passe");
    let reponse = post_formulaire(&app, "/connexion", None, &nouveau).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
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

/// Un conducteur de test au service et au statut donnés (les autres champs, comme les autres).
async fn conducteur_au_service(pool: &PgPool, nom: &str, service: &str, statut: &str) -> i64 {
    gestion_parc_auto::metier::conducteurs::creer(
        pool,
        &gestion_parc_auto::metier::conducteurs::SaisieConducteur {
            service: service.to_string(),
            statut: statut.to_string(),
            ..saisie_conducteur(nom)
        },
    )
    .await
    .expect("création du conducteur")
    .id
}

#[sqlx::test]
async fn la_liste_des_conducteurs_dit_le_service_pour_distinguer_les_homonymes(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    // Deux Camille Martin : seul leur service les distingue.
    let logistique = conducteur_au_service(&pool, "Martin", "Logistique", "actif").await;
    let atelier = conducteur_au_service(&pool, "Martin", "Atelier", "actif").await;
    // Sans service : le nom seul, sans virgule qui pend.
    let sans_service = conducteur_au_service(&pool, "Durand", "", "actif").await;
    // L'empêchement vient après le service.
    let parti = conducteur_au_service(&pool, "Moreau", "Atelier", "inactif").await;

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/affectations/nouvelle", Some(&cookie)).await).await;
    for attendu in [
        format!(r#"<option value="{logistique}">Camille Martin, Logistique</option>"#),
        format!(r#"<option value="{atelier}">Camille Martin, Atelier</option>"#),
        format!(r#"<option value="{sans_service}">Camille Durand</option>"#),
        format!(r#"<option value="{parti}" disabled>Camille Moreau, Atelier (inactif)</option>"#),
    ] {
        assert!(page.contains(&attendu), "absent : {attendu}\n{page}");
    }

    // Le profil particulier n'a pas de service : le formulaire (masqué, mais joignable par son
    // adresse) n'en montre pas.
    let particulier = application_avec_profil(pool, Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    let page = texte(get(&particulier, "/affectations/nouvelle", Some(&cookie)).await).await;
    assert!(
        page.contains(&format!(
            r#"<option value="{logistique}">Camille Martin</option>"#
        )),
        "{page}"
    );
    assert!(!page.contains("Camille Martin, "), "{page}");
    assert!(!page.contains("Logistique"), "{page}");
}

#[sqlx::test]
async fn le_service_distingue_les_homonymes_dans_la_liste_des_affectations_et_sur_la_fiche(
    pool: PgPool,
) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "SV-111-SV").await;
    let logistique = conducteur_au_service(&pool, "Martin", "Logistique", "actif").await;
    let atelier = conducteur_au_service(&pool, "Martin", "Atelier", "actif").await;
    let sans_service = conducteur_au_service(&pool, "Durand", "", "actif").await;
    for conducteur in [logistique, atelier, sans_service] {
        gestion_parc_auto::metier::affectations::creer(
            &pool,
            &saisie_affectation(vehicule, conducteur, ""),
            aujourd_hui(),
        )
        .await
        .expect("affectation acceptée");
    }

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let liste = texte(get(&app, "/affectations", Some(&cookie)).await).await;
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;

    // (page, nombre de fois que chaque conducteur y figure) : la liste a une ligne par
    // affectation ; la fiche les nomme deux fois, parmi les conducteurs actuels puis dans
    // l'historique.
    for (nom, page, fois) in [("liste", &liste, 1), ("fiche", &fiche, 2)] {
        for (attendu, ecrit) in [
            (
                format!(r#"<a href="/conducteurs/{logistique}">Camille Martin</a>, Logistique"#),
                "Martin, Logistique",
            ),
            (
                format!(r#"<a href="/conducteurs/{atelier}">Camille Martin</a>, Atelier"#),
                "Martin, Atelier",
            ),
        ] {
            assert_eq!(
                page.matches(&attendu).count(),
                fois,
                "{nom} : « {ecrit} » attendu {fois} fois\n{page}"
            );
        }
        // Sans service : le nom seul, sans virgule qui pend.
        let seul = format!(r#"<a href="/conducteurs/{sans_service}">Camille Durand</a>"#);
        assert_eq!(page.matches(&seul).count(), fois, "{nom}\n{page}");
        assert!(!page.contains(&format!("{seul},")), "{nom}\n{page}");
    }

    // Le profil particulier n'a pas de service : la liste, masquée mais joignable par son
    // adresse, n'en montre pas.
    let particulier = application_avec_profil(pool, Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    let liste = texte(get(&particulier, "/affectations", Some(&cookie)).await).await;
    assert!(liste.contains("Camille Martin</a>"), "{liste}");
    assert!(!liste.contains("Logistique"), "{liste}");
    assert!(!liste.contains("Atelier"), "{liste}");
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
        format!(
            r#"<option value="{parti}" disabled>Camille Moreau, Logistique (inactif)</option>"#
        ),
        format!(
            r#"<option value="{sans_permis}" disabled>Camille Petit, Logistique (permis expiré)</option>"#
        ),
        format!(r#"<option value="{partage}">PA-111-PA (Renault Kangoo)</option>"#),
        format!(r#"<option value="{durand}">Camille Durand, Logistique</option>"#),
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
    // Le service vient à la suite du nom, avant la période.
    assert!(
        fiche.contains("Camille Bernard</a>, Logistique du 10/01/2026 au 31/12/2099"),
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

#[sqlx::test]
async fn un_identifiant_illisible_donne_la_page_introuvable(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    // Ni un mot, ni un nombre à virgule, ni un nombre trop grand ne sont des identifiants.
    for chemin in [
        "/vehicules/abc",
        "/conducteurs/1.5",
        "/affectations/x/modifier",
        "/interventions/x/modifier",
        "/echeances/x/modifier",
        "/documents/abc/telecharger",
        "/documents/abc/voir",
        "/vehicules/99999999999999999999",
    ] {
        let reponse = get(&app, chemin, Some(&cookie)).await;
        assert_eq!(reponse.status(), StatusCode::NOT_FOUND, "{chemin}");
        let page = texte(reponse).await;
        assert!(page.contains("<h1>Introuvable</h1>"), "{chemin}\n{page}");
    }

    // Une route qui écrit répond de même.
    let reponse = post_formulaire(&app, "/echeances/abc/traiter", Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);

    // L'API répond 404 en JSON.
    let reponse = requete_json(&app, "GET", "/api/v1/vehicules/abc", Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
    let corps: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(corps["erreur"], "Ressource introuvable");

    // Sans session, une adresse mal formée n'en dit pas plus qu'une autre : connexion d'abord.
    let reponse = get(&app, "/vehicules/abc", None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/connexion");
}

#[sqlx::test]
async fn un_depot_refuse_s_affiche_sur_la_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "DO-111-DO").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let config = commun::config_de_test();
    let dossier = config.documents_dir.clone();
    let app = gestion_parc_auto::web::routeur(Etat::nouveau(pool, config).expect("état"));
    let cookie = se_connecter(&app, "gestion").await;
    let depot_vehicule = format!("/vehicules/{vehicule}/documents");
    let depot_conducteur = format!("/conducteurs/{conducteur}/documents");

    const REFUS: &str = "Document non déposé. Le fichier dépasse la taille maximale de 10 Mo.";

    // 11 Mo : le serveur cesse de lire en cours d'envoi. 10 Mo et un octet : l'envoi passe, le
    // contrôle refuse. Dans les deux cas, la fiche du véhicule revient avec le même message.
    for taille in [11 * 1024 * 1024, 10 * 1024 * 1024 + 1] {
        let contenu = vec![b'x'; taille];
        let reponse = post_fichier(&app, &cookie, &depot_vehicule, "gros.pdf", &contenu).await;
        let statut = reponse.status();
        let page = texte(reponse).await;
        assert_eq!(statut, StatusCode::OK, "{taille}\n{page}");
        assert!(page.contains(REFUS), "{taille}\n{page}");
        assert!(
            page.contains(r#"<h1><span class="plaque">DO-111-DO</span>"#),
            "fiche du véhicule\n{page}"
        );
        assert!(page.contains("Se déconnecter"), "avec le menu\n{page}");
        assert!(
            page.contains("Aucun document."),
            "rien d'enregistré\n{page}"
        );
        assert!(!page.contains("Error parsing"), "{page}");
        assert!(!page.contains("trop volumineux ?"), "{page}");
    }

    // Un fichier vide, déposé depuis la fiche d'un conducteur : même principe.
    let reponse = post_fichier(&app, &cookie, &depot_conducteur, "vide.pdf", b"").await;
    let statut = reponse.status();
    let page = texte(reponse).await;
    assert_eq!(statut, StatusCode::OK, "{page}");
    assert!(
        page.contains("Document non déposé. Le fichier est vide."),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<h1><span class="initiales" aria-hidden="true">CD</span>Camille Durand</h1>"#
        ),
        "{page}"
    );

    // Un dépôt accepté redirige vers la fiche, à la hauteur de ses documents ; elle liste le
    // fichier sans message.
    for (depot, fiche) in [
        (&depot_vehicule, format!("/vehicules/{vehicule}")),
        (&depot_conducteur, format!("/conducteurs/{conducteur}")),
    ] {
        let reponse = post_fichier(&app, &cookie, depot, "piece.pdf", b"%PDF-1.4 essai").await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{depot}");
        assert_eq!(
            reponse.headers()[header::LOCATION],
            format!("{fiche}#documents"),
            "{depot}"
        );
        let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
        assert!(page.contains("piece.pdf"), "{page}");
        assert!(!page.contains("Document non déposé"), "{page}");
    }

    // Chaque fiche donne au navigateur de quoi refuser un fichier trop gros avant de l'envoyer.
    let attendu = concat!(
        r#"data-taille-max="10485760" "#,
        r#"data-trop-gros="Le fichier dépasse la taille maximale de 10 Mo.""#
    );
    for fiche in [
        format!("/vehicules/{vehicule}"),
        format!("/conducteurs/{conducteur}"),
    ] {
        let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
        assert!(page.contains(attendu), "{fiche}\n{page}");
    }

    // Un lecteur ne dépose rien, et un identifiant illisible reste « introuvable ».
    let lecteur = se_connecter(&app, "lecture").await;
    let reponse = post_fichier(&app, &lecteur, &depot_vehicule, "piece.pdf", b"x").await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let reponse = post_fichier(&app, &cookie, "/vehicules/abc/documents", "piece.pdf", b"x").await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(dossier);
}

/// Un fichier du formulaire de dépôt : nom, type MIME, contenu.
type Fichier<'a> = (&'a str, &'a str, &'a [u8]);

/// Le champ de fichier que l'utilisateur n'a pas touché : le navigateur l'envoie quand même.
const CHAMP_VIDE: Fichier = ("", "application/octet-stream", b"");

#[sqlx::test]
async fn un_document_porte_un_type(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "TY-111-PE").await;
    let conducteur = creer_conducteur(&pool, "Martin").await;
    let config = commun::config_de_test();
    let dossier = config.documents_dir.clone();
    let app = gestion_parc_auto::web::routeur(Etat::nouveau(pool, config).expect("état"));
    let cookie = se_connecter(&app, "gestion").await;
    let fiche = format!("/vehicules/{vehicule}");
    let depot = format!("{fiche}/documents");
    let pdf: Fichier = ("pv.pdf", "application/pdf", b"%PDF-1.4 essai");

    // Le type choisi au dépôt s'affiche dans la liste de la fiche.
    let champs = [("type_document", "controle_technique")];
    let reponse = post_multipart(&app, &cookie, &depot, &champs, &[pdf]).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(page.contains("<td>Contrôle technique</td>"), "{page}");

    // Sans type, ou avec un type hors du vocabulaire, la fiche revient avec le motif et rien
    // de plus n'est enregistré.
    let sans_type: &[(&str, &str)] = &[];
    let type_inconnu: &[(&str, &str)] = &[("type_document", "devis")];
    for (champs, refus) in [
        (
            sans_type,
            "Document non déposé. Choisissez le type du document.",
        ),
        (type_inconnu, "Document non déposé. La valeur « devis »"),
    ] {
        let reponse = post_multipart(&app, &cookie, &depot, champs, &[pdf]).await;
        let statut = reponse.status();
        let page = texte(reponse).await;
        assert_eq!(statut, StatusCode::OK, "{page}");
        assert!(page.contains(refus), "{refus}\n{page}");
        assert_eq!(page.matches("/telecharger").count(), 1, "{page}");
    }
    assert_eq!(
        std::fs::read_dir(&dossier).unwrap().count(),
        1,
        "aucun fichier orphelin sur le disque"
    );

    // Chaque fiche propose les types qui la concernent.
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(page.contains(r#"<option value="facture">"#), "{page}");
    assert!(!page.contains(r#"<option value="permis">"#), "{page}");
    let page = texte(get(&app, &format!("/conducteurs/{conducteur}"), Some(&cookie)).await).await;
    assert!(page.contains(r#"<option value="permis">"#), "{page}");
    assert!(!page.contains(r#"<option value="facture">"#), "{page}");

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn la_page_de_depot_range_le_document_sur_le_vehicule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let config = commun::config_de_test();
    let dossier = config.documents_dir.clone();
    let app = gestion_parc_auto::web::routeur(Etat::nouveau(pool.clone(), config).expect("état"));
    let cookie = se_connecter(&app, "gestion").await;

    // Sans véhicule, la page le dit au lieu de proposer un formulaire inutilisable.
    let page = texte(get(&app, "/documents/nouveau", Some(&cookie)).await).await;
    assert!(page.contains("Aucun véhicule enregistré"), "{page}");
    assert!(!page.contains(r#"name="fichier""#), "{page}");

    // Avec un véhicule : la liste, le bouton qui ouvre l'appareil photo, l'envoi automatique.
    let vehicule = creer_vehicule(&pool, "PH-111-OT").await;
    let page = texte(get(&app, "/documents/nouveau", Some(&cookie)).await).await;
    for attendu in [
        format!(r#"<option value="{vehicule}">PH-111-OT (Renault Kangoo)</option>"#),
        r#"<a href="/documents/nouveau" data-ico="depot" class="actif" aria-current="page">Déposer un document</a>"#
            .to_string(),
        r#"action="/documents""#.to_string(),
        r#"accept="image/*" capture="environment""#.to_string(),
        "data-envoi-auto".to_string(),
    ] {
        assert!(page.contains(&attendu), "{attendu}\n{page}");
    }

    // Une photo prise au téléphone : le second champ de fichier arrive vide.
    let id_vehicule = vehicule.to_string();
    let champs = [
        ("vehicule_id", id_vehicule.as_str()),
        ("type_document", "facture"),
    ];
    let photo: Fichier = ("image.jpg", "image/jpeg", b"fausse photo");
    let reponse = post_multipart(&app, &cookie, "/documents", &champs, &[photo, CHAMP_VIDE]).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let id: i64 = sqlx::query_scalar("SELECT id FROM documents WHERE nom_original = 'image.jpg'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let suite = format!("/documents/nouveau?depose={id}");
    assert_eq!(reponse.headers()[header::LOCATION], suite);

    // La page confirme, et garde le véhicule choisi pour le document suivant.
    let page = texte(get(&app, &suite, Some(&cookie)).await).await;
    assert!(page.contains("Enregistré : Facture pour"), "{page}");
    assert!(
        page.contains(&format!(r#"<option value="{vehicule}" selected>"#)),
        "{page}"
    );

    // Un fichier choisi par l'autre bouton : c'est le premier champ qui arrive vide.
    let champs_pv = [
        ("vehicule_id", id_vehicule.as_str()),
        ("type_document", "controle_technique"),
    ];
    let pdf: Fichier = ("pv.pdf", "application/pdf", b"%PDF-1.4 essai");
    let reponse = post_multipart(&app, &cookie, "/documents", &champs_pv, &[CHAMP_VIDE, pdf]).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    // Les deux documents sont sur la fiche du véhicule, avec leur type.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    for attendu in [
        "image.jpg",
        "pv.pdf",
        "<td>Facture</td>",
        "<td>Contrôle technique</td>",
    ] {
        assert!(fiche.contains(attendu), "{attendu}\n{fiche}");
    }

    // Refus : la page revient avec le motif, et ce qui était déjà choisi le reste.
    let sans_vehicule = [("vehicule_id", ""), ("type_document", "facture")];
    let vehicule_inconnu = [("vehicule_id", "999999"), ("type_document", "facture")];
    let sans_fichier: &[Fichier] = &[CHAMP_VIDE, CHAMP_VIDE];
    // Un octet de trop : l'envoi est reçu en entier, c'est le contrôle de taille qui refuse.
    let trop_gros = vec![b'x'; 10 * 1024 * 1024 + 1];
    let gros: &[Fichier] = &[("gros.jpg", "image/jpeg", trop_gros.as_slice())];
    for (champs, fichiers, refus, garde) in [
        (
            &sans_vehicule,
            &[photo][..],
            "Le champ « véhicule » doit être choisi dans la liste.",
            r#"<option value="facture" selected>"#.to_string(),
        ),
        (
            &vehicule_inconnu,
            &[photo][..],
            "Le véhicule ou le conducteur",
            r#"<option value="facture" selected>"#.to_string(),
        ),
        (
            &champs,
            sans_fichier,
            "Aucun fichier",
            format!(r#"<option value="{vehicule}" selected>"#),
        ),
        (
            &champs,
            gros,
            "Le fichier dépasse la taille maximale de 10 Mo.",
            format!(r#"<option value="{vehicule}" selected>"#),
        ),
    ] {
        let reponse = post_multipart(&app, &cookie, "/documents", champs, fichiers).await;
        let statut = reponse.status();
        let page = texte(reponse).await;
        assert_eq!(statut, StatusCode::OK, "{refus}\n{page}");
        assert!(page.contains("<h1>Déposer un document</h1>"), "{page}");
        assert!(
            page.contains(&format!("Document non déposé. {refus}")),
            "{refus}\n{page}"
        );
        assert!(page.contains(&garde), "{refus}\n{page}");
        assert!(!page.contains("Enregistré :"), "{page}");
    }
    assert_eq!(
        std::fs::read_dir(&dossier).unwrap().count(),
        2,
        "seuls les deux dépôts acceptés sont sur le disque"
    );

    // Une confirmation illisible, ou d'un document qui n'existe pas, n'affiche rien.
    for adresse in [
        "/documents/nouveau?depose=abc",
        "/documents/nouveau?depose=999999",
    ] {
        let reponse = get(&app, adresse, Some(&cookie)).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{adresse}");
        assert!(!texte(reponse).await.contains("Enregistré :"), "{adresse}");
    }

    // Un lecteur ne dépose rien : ni la page, ni l'envoi, ni le lien dans le menu.
    let lecteur = se_connecter(&app, "lecture").await;
    let reponse = get(&app, "/documents/nouveau", Some(&lecteur)).await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let reponse = post_multipart(&app, &lecteur, "/documents", &champs, &[photo]).await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let page = texte(get(&app, "/vehicules", Some(&lecteur)).await).await;
    assert!(!page.contains("/documents/nouveau"), "{page}");

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn une_image_s_affiche_un_autre_fichier_se_telecharge(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "VO-111-IR").await;
    let config = commun::config_de_test();
    let dossier = config.documents_dir.clone();
    let app = gestion_parc_auto::web::routeur(Etat::nouveau(pool.clone(), config).expect("état"));
    let cookie = se_connecter(&app, "gestion").await;
    let depot = format!("/vehicules/{vehicule}/documents");
    let champs = [("type_document", "autre")];

    let image: Fichier = ("image.jpg", "image/jpeg", b"fausse photo");
    let page_html: Fichier = ("page.html", "text/html", b"<script>alert(1)</script>");
    let mut ids = Vec::new();
    for fichier in [image, page_html] {
        let reponse = post_multipart(&app, &cookie, &depot, &champs, &[fichier]).await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{}", fichier.0);
        let id: i64 = sqlx::query_scalar("SELECT id FROM documents WHERE nom_original = $1")
            .bind(fichier.0)
            .fetch_one(&pool)
            .await
            .unwrap();
        ids.push(id);
    }
    let (id_image, id_html) = (ids[0], ids[1]);

    // La fiche ouvre la page du document par son nom, et propose aussi de le télécharger.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    for attendu in [
        format!(r#"<a href="/documents/{id_image}">image.jpg</a>"#),
        format!(r#"<a href="/documents/{id_image}/telecharger">Télécharger</a>"#),
    ] {
        assert!(fiche.contains(&attendu), "{attendu}\n{fiche}");
    }

    // Une image s'affiche dans le navigateur, y compris pour un lecteur.
    let lecteur = se_connecter(&app, "lecture").await;
    for session in [&cookie, &lecteur] {
        let reponse = get(&app, &format!("/documents/{id_image}/voir"), Some(session)).await;
        assert_eq!(reponse.status(), StatusCode::OK);
        assert_eq!(reponse.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(
            reponse.headers()[header::CONTENT_DISPOSITION],
            r#"inline; filename="image.jpg""#
        );
        assert_eq!(reponse.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(texte(reponse).await, "fausse photo");
    }

    // La même image, demandée en téléchargement, part en pièce jointe.
    let reponse = get(
        &app,
        &format!("/documents/{id_image}/telecharger"),
        Some(&cookie),
    )
    .await;
    assert_eq!(
        reponse.headers()[header::CONTENT_DISPOSITION],
        r#"attachment; filename="image.jpg""#
    );

    // Un fichier HTML ne s'affiche jamais : même par « voir », il part en pièce jointe.
    let reponse = get(&app, &format!("/documents/{id_html}/voir"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert_eq!(
        reponse.headers()[header::CONTENT_DISPOSITION],
        r#"attachment; filename="page.html""#
    );

    // Sans session, pas de document.
    let reponse = get(&app, &format!("/documents/{id_image}/voir"), None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/connexion");

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn marquer_traitee_revient_a_la_page_d_origine(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "EC-111-EC").await;
    let mut ids = Vec::new();
    for type_echeance in ["controle_technique", "assurance", "entretien_periodique"] {
        let saisie = SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: type_echeance.to_string(),
            date_limite: "2027-01-01".to_string(),
            statut: String::new(),
        };
        ids.push(echeances::creer(&pool, &saisie).await.unwrap().id);
    }
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Les deux pages qui portent le bouton disent d'où part le clic.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    let attendu = format!(r#"action="/echeances/{}/traiter?retour=vehicule""#, ids[0]);
    assert!(fiche.contains(&attendu), "absent : {attendu}\n{fiche}");
    let liste = texte(get(&app, "/echeances", Some(&cookie)).await).await;
    let attendu = format!(r#"action="/echeances/{}/traiter?retour=liste""#, ids[0]);
    assert!(liste.contains(&attendu), "absent : {attendu}\n{liste}");

    // Le clic ouvre le formulaire de la prochaine échéance, pré-rempli, avec le mot-clé de
    // l'origine pour « Annuler ». Une valeur forgée ne mène jamais hors de l'application.
    for (id, origine, type_echeance, date_suivante, retour) in [
        (
            ids[0],
            "?retour=vehicule",
            "controle_technique",
            "2029-01-01",
            "vehicule",
        ),
        (ids[1], "?retour=liste", "assurance", "2028-01-01", "liste"),
        (
            ids[2],
            "?retour=https://exemple.test",
            "entretien_periodique",
            "2028-01-01",
            "liste",
        ),
    ] {
        let chemin = format!("/echeances/{id}/traiter{origine}");
        let reponse = post_formulaire(&app, &chemin, Some(&cookie), "").await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{origine}");
        let attendu = format!(
            "/echeances/nouvelle?vehicule_id={vehicule}&type_echeance={type_echeance}&date_limite={date_suivante}&retour={retour}"
        );
        assert_eq!(reponse.headers()[header::LOCATION], attendu, "{origine}");
    }

    let toutes = echeances::lister(&pool).await.unwrap();
    assert!(toutes.iter().all(|e| !e.est_ouverte()), "toutes traitées");
}

#[sqlx::test]
async fn marquer_traitee_depuis_les_alertes(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "AL-777-AL").await;
    // Une échéance dépassée d'hier : elle figure aux alertes, avec son bouton.
    let hier = chrono::Local::now().date_naive().pred_opt().unwrap();
    let saisie = SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: "assurance".to_string(),
        date_limite: hier.to_string(),
        statut: String::new(),
    };
    let id = echeances::creer(&pool, &saisie).await.unwrap().id;
    // Le permis expiré d'un conducteur actif figure aux alertes aussi : sans bouton.
    let conducteur = creer_conducteur(&pool, "Expire").await;
    changer_conducteur(&pool, conducteur, "Expire", "actif", "2020-01-01").await;

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let action = format!(r#"action="/echeances/{id}/traiter?retour=alertes""#);
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert_eq!(page.matches("Marquer traitée").count(), 1, "{page}");
    assert!(page.contains(&action), "absent : {action}\n{page}");
    assert!(page.contains("Permis de conduire"), "{page}");

    // Un lecteur ne traite rien : il ne voit pas le bouton.
    let lecteur = se_connecter(&app, "lecture").await;
    let page = texte(get(&app, "/alertes", Some(&lecteur)).await).await;
    assert!(!page.contains("Marquer traitée"), "{page}");

    // Le clic propose la prochaine échéance, dont « Annuler » ramène aux alertes ; l'échéance a
    // disparu des alertes, le permis, lui, y reste.
    let chemin = format!("/echeances/{id}/traiter?retour=alertes");
    let reponse = post_formulaire(&app, &chemin, Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let suite = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        suite.starts_with("/echeances/nouvelle?") && suite.ends_with("&retour=alertes"),
        "{suite}"
    );
    let page = texte(get(&app, &suite, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<a href="/alertes" class="bouton-discret">Annuler</a>"#),
        "{page}"
    );
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(!page.contains("Marquer traitée"), "{page}");
    assert!(page.contains("Permis de conduire"), "{page}");
}

#[sqlx::test]
async fn le_profil_particulier_masque_les_affectations_et_le_service(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PR-111-OF").await;
    let conducteur = creer_conducteur(&pool, "Durand").await;
    let fiche_vehicule = format!("/vehicules/{vehicule}");
    let modifier_vehicule = format!("{fiche_vehicule}/modifier");
    let fiche_conducteur = format!("/conducteurs/{conducteur}");
    let modifier_conducteur = format!("{fiche_conducteur}/modifier");

    // Ce que seule une entreprise voit : la page, puis le morceau de HTML.
    let reserve_a_l_entreprise = [
        ("/vehicules", r#"href="/affectations""#),
        (fiche_vehicule.as_str(), "Conducteur actuel"),
        (fiche_vehicule.as_str(), "Historique des affectations"),
        (modifier_vehicule.as_str(), "met fin à ses affectations"),
        ("/conducteurs", "<th>Service</th>"),
        ("/conducteurs", "<td>Logistique</td>"),
        (fiche_conducteur.as_str(), "<dt>Service</dt>"),
        (
            fiche_conducteur.as_str(),
            r#"<h2 data-ico="echange">Affectations</h2>"#,
        ),
        (modifier_conducteur.as_str(), "<label>Service"),
        (modifier_conducteur.as_str(), "met fin à ses affectations"),
        (
            modifier_conducteur.as_str(),
            "son historique d'affectations",
        ),
    ];

    // Sans réglage, l'installation est celle d'une entreprise : tout est là, comme avant.
    let entreprise = application(pool.clone());
    let cookie = se_connecter(&entreprise, "gestion").await;
    for (chemin, attendu) in reserve_a_l_entreprise {
        let page = texte(get(&entreprise, chemin, Some(&cookie)).await).await;
        assert!(
            page.contains(attendu),
            "{chemin} : absent : {attendu}\n{page}"
        );
    }

    // Profil particulier : plus rien de cela, et les pages s'affichent toujours.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    for (chemin, masque) in reserve_a_l_entreprise {
        let reponse = get(&particulier, chemin, Some(&cookie)).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{chemin}");
        let page = texte(reponse).await;
        assert!(
            !page.contains(masque),
            "{chemin} : présent : {masque}\n{page}"
        );
    }

    // Le reste de la fiche d'un véhicule et de la liste des conducteurs n'a pas bougé.
    let page = texte(get(&particulier, &fiche_vehicule, Some(&cookie)).await).await;
    for attendu in [
        r#"<h2 data-ico="calendrier">Échéances</h2>"#,
        r#"<h2 data-ico="cle">Entretien</h2>"#,
        r#"<h2 data-ico="fichier">Documents</h2>"#,
    ] {
        assert!(page.contains(attendu), "absent : {attendu}\n{page}");
    }
    let page = texte(get(&particulier, "/conducteurs", Some(&cookie)).await).await;
    assert!(page.contains("<th>Expiration du permis</th>"), "{page}");
    assert!(page.contains(">Durand</a>"), "{page}");

    // Le service déjà saisi voyage dans un champ caché : enregistrer la fiche ne l'efface pas.
    let page = texte(get(&particulier, &modifier_conducteur, Some(&cookie)).await).await;
    let champ_cache = r#"<input type="hidden" name="service" value="Logistique">"#;
    assert!(page.contains(champ_cache), "{page}");
    let reponse = post_formulaire(
        &particulier,
        &fiche_conducteur,
        Some(&cookie),
        "nom=Durand&prenom=Camille&service=Logistique&date_expiration_permis=2032-02-02&statut=actif",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let enregistre = gestion_parc_auto::metier::conducteurs::obtenir(&pool, conducteur)
        .await
        .unwrap();
    assert_eq!(enregistre.service, "Logistique");
    assert_eq!(enregistre.date_expiration_permis.to_string(), "2032-02-02");

    // Le profil masque, il n'interdit pas : l'adresse des affectations répond toujours.
    let reponse = get(&particulier, "/affectations", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
}

#[sqlx::test]
async fn le_formulaire_conducteur_ne_promet_pas_de_suppression_automatique(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;

    // Au-delà de la durée de conservation, c'est la commande `purger-conducteurs` qui supprime,
    // jamais le serveur : le texte d'aide le dit dans les deux profils.
    for profil in [Profil::Entreprise, Profil::Particulier] {
        let app = application_avec_profil(pool.clone(), profil);
        let cookie = se_connecter(&app, "gestion").await;
        let page = texte(get(&app, "/conducteurs/nouveau", Some(&cookie)).await).await;
        assert!(page.contains("commande d'administration"), "{page}");
        assert!(page.contains("ne la lance pas d'elle-même"), "{page}");
        assert!(!page.contains("automatiquement"), "{page}");
    }
}

#[sqlx::test]
async fn les_pages_disent_ou_l_on_est_et_annoncent_les_refus(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AC-111-ES").await;
    let saisie = SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: "assurance".to_string(),
        date_limite: "2027-01-01".to_string(),
        statut: String::new(),
    };
    echeances::creer(&pool, &saisie).await.unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le menu marque la page en cours pour un lecteur d'écran, une seule fois par page, et
    // chaque page à menu commence par le lien qui mène droit au contenu.
    let evitement = r##"<a class="bouton lien-evitement" href="#contenu">Aller au contenu</a>"##;
    for (chemin, intitule) in [
        ("/", "Tableau de bord"),
        ("/alertes", "Alertes"),
        ("/vehicules", "Véhicules"),
        ("/conducteurs", "Conducteurs"),
        ("/affectations", "Affectations"),
        ("/interventions", "Entretien"),
        ("/echeances", "Échéances"),
    ] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        let actif = format!(r#" class="actif" aria-current="page">{intitule}</a>"#);
        assert!(page.contains(&actif), "{chemin}\n{page}");
        assert_eq!(page.matches("aria-current").count(), 1, "{chemin}\n{page}");
        let (avant_le_menu, _) = page.split_once("<header").expect("en-tête");
        assert!(avant_le_menu.contains(evitement), "{chemin}\n{page}");
        let contenu = r#"<main class="contenu" id="contenu">"#;
        assert!(page.contains(contenu), "{chemin}\n{page}");
    }

    // La colonne des actions a un en-tête pour les lecteurs d'écran, sans texte affiché : sur
    // écran étroit, `app.js` ne recopie sur les cellules que le texte des en-têtes.
    let page = texte(get(&app, "/echeances", Some(&cookie)).await).await;
    assert!(page.contains(r#"<th aria-label="Actions"></th>"#), "{page}");
    assert!(!page.contains("<th></th>"), "{page}");

    // Une saisie refusée s'annonce deux fois : par le titre de la page, lu en premier, et par le
    // rôle du message. Sans refus, le titre ne change pas.
    let message = r#"<p class="message-erreur" role="alert">"#;
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), "immatriculation=").await;
    let page = texte(reponse).await;
    let titre = "<title>Erreur : Nouveau véhicule · Autoclair Pro</title>";
    assert!(page.contains(titre), "{page}");
    assert!(page.contains(message), "{page}");
    let page = texte(get(&app, "/vehicules/nouveau", Some(&cookie)).await).await;
    let titre = "<title>Nouveau véhicule · Autoclair Pro</title>";
    assert!(page.contains(titre), "{page}");
    assert!(!page.contains(message), "{page}");

    // Une adresse inconnue et une connexion refusée portent le même rôle.
    let page = texte(get(&app, "/adresse-inconnue", Some(&cookie)).await).await;
    assert!(page.contains(message), "{page}");
    let corps = "login=gestion&mot_de_passe=faux";
    let page = texte(post_formulaire(&app, "/connexion", None, corps).await).await;
    let titre = "<title>Erreur : Connexion · Autoclair Pro</title>";
    assert!(page.contains(titre), "{page}");
    assert!(page.contains(message), "{page}");
}

/// Les deux marques de la charte : le particulier est « Autoclair », l'entreprise « Autoclair
/// Pro ». Chacune a son logo et son icône, tirés des fichiers de la charte.
const MARQUES: [(Profil, &str, &str, &str); 2] = [
    (
        Profil::Entreprise,
        "Autoclair Pro",
        "/static/logos/autoclair-pro.svg",
        "/static/logos/icone-autoclair-pro.svg",
    ),
    (
        Profil::Particulier,
        "Autoclair",
        "/static/logos/autoclair.svg",
        "/static/logos/icone-autoclair.svg",
    ),
];

#[sqlx::test]
async fn l_en_tete_et_le_titre_disent_le_profil(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    for (profil, marque, logo, icone) in MARQUES {
        let app = application_avec_profil(pool.clone(), profil);
        let cookie = se_connecter(&app, "gestion").await;
        let page = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
        // Sur la barre latérale marine d'Autoclair Pro, le logo de l'en-tête est le négatif.
        let logo_en_tete = if profil == Profil::Entreprise {
            "/static/logos/autoclair-pro-negatif.svg"
        } else {
            logo
        };
        let lien = format!(
            r#"<a class="marque" href="/"><img class="logo" src="{logo_en_tete}" alt="{marque}"></a>"#
        );
        let titre = format!("<title>Véhicules · {marque}</title>");
        let onglet = format!(r#"<link rel="icon" type="image/svg+xml" href="{icone}">"#);
        assert!(page.contains(&lien), "{marque}\n{page}");
        assert!(page.contains(&titre), "{marque}\n{page}");
        assert!(page.contains(&onglet), "{marque}\n{page}");
        // Ce que le téléphone lit pour installer la marque sur son écran d'accueil : le
        // manifest, l'icône 180 px d'iPhone et la couleur de la barre d'état.
        let (icone_180, theme) = installation(profil);
        let manifest = r#"<link rel="manifest" href="/manifest.webmanifest">"#;
        let apple = format!(r#"<link rel="apple-touch-icon" href="{icone_180}">"#);
        let theme = format!(r#"<meta name="theme-color" content="{theme}">"#);
        assert!(page.contains(manifest), "{marque}\n{page}");
        assert!(page.contains(&apple), "{marque}\n{page}");
        assert!(page.contains(&theme), "{marque}\n{page}");
        // Les jetons de la charte se chargent avant la feuille de style qui les emploie.
        let jetons = page.find("/static/tokens.css").expect("jetons liés");
        let style = page.find("/static/style.css").expect("style lié");
        assert!(jetons < style, "{marque}\n{page}");
    }
}

#[sqlx::test]
async fn la_page_de_connexion_dit_le_profil(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    for (profil, marque, logo, _) in MARQUES {
        let app = application_avec_profil(pool.clone(), profil);
        let logo = format!(r#"<img class="logo logo-grand" src="{logo}" alt="{marque}">"#);

        // Avant toute connexion, la page dit déjà dans quelle installation l'on est, et le
        // téléphone peut déjà l'installer : c'est souvent depuis cette page qu'on le fait.
        let page = texte(get(&app, "/connexion", None).await).await;
        let titre = format!("<title>Connexion · {marque}</title>");
        assert!(page.contains(&logo), "{marque}\n{page}");
        assert!(page.contains(&titre), "{marque}\n{page}");
        let (icone_180, theme) = installation(profil);
        let manifest = r#"<link rel="manifest" href="/manifest.webmanifest">"#;
        let apple = format!(r#"<link rel="apple-touch-icon" href="{icone_180}">"#);
        let theme = format!(r#"<meta name="theme-color" content="{theme}">"#);
        assert!(page.contains(manifest), "{marque}\n{page}");
        assert!(page.contains(&apple), "{marque}\n{page}");
        assert!(page.contains(&theme), "{marque}\n{page}");

        // Un refus réaffiche la page avec le même nom, sous le titre d'erreur.
        let corps = "login=gestion&mot_de_passe=faux";
        let page = texte(post_formulaire(&app, "/connexion", None, corps).await).await;
        let titre = format!("<title>Erreur : Connexion · {marque}</title>");
        assert!(page.contains(&logo), "{marque}\n{page}");
        assert!(page.contains(&titre), "{marque}\n{page}");
    }
}

/// Ce que l'installation sur l'écran d'accueil demande à chaque marque : son icône 180 px
/// (iPhone) et la couleur de thème. Le marine de la barre latérale pour Autoclair Pro, le blanc
/// de l'en-tête pour Autoclair : les valeurs des jetons `--marine-700` et `--blanc` de la charte.
fn installation(profil: Profil) -> (&'static str, &'static str) {
    match profil {
        Profil::Entreprise => ("/static/logos/autoclair-pro-180.png", "#062966"),
        Profil::Particulier => ("/static/logos/autoclair-180.png", "#ffffff"),
    }
}

/// Le manifest d'application web : ce que le téléphone lit pour poser l'icône de la marque sur
/// son écran d'accueil et l'ouvrir plein écran. Anonyme, comme les logos. Chaque icône qu'il
/// cite se sert vraiment, en PNG.
#[sqlx::test]
async fn le_manifest_installe_la_marque_sur_le_telephone(pool: PgPool) {
    for (profil, marque, _, _) in MARQUES {
        let app = application_avec_profil(pool.clone(), profil);
        let (icone_180, theme) = installation(profil);

        let reponse = get(&app, "/manifest.webmanifest", None).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{marque}");
        let type_recu = reponse.headers()[header::CONTENT_TYPE].to_str().unwrap();
        assert!(
            type_recu.starts_with("application/manifest+json"),
            "{marque} : {type_recu}"
        );
        assert_eq!(
            reponse.headers()[header::CACHE_CONTROL],
            "public, max-age=86400"
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&texte(reponse).await).expect("manifest JSON");
        assert_eq!(manifest["name"], marque);
        assert_eq!(manifest["short_name"], marque);
        assert_eq!(manifest["lang"], "fr");
        assert_eq!(manifest["start_url"], "/");
        assert_eq!(manifest["display"], "standalone");
        assert_eq!(manifest["theme_color"], theme, "{marque}");
        // Le fond de l'écran d'attente est celui des pages (`--fond`).
        assert_eq!(manifest["background_color"], "#f5f7fb", "{marque}");
        let icones = manifest["icons"].as_array().expect("icônes");
        let tailles: Vec<&str> = icones
            .iter()
            .map(|i| i["sizes"].as_str().unwrap())
            .collect();
        assert_eq!(tailles, ["192x192", "512x512"], "{marque}");

        // Les icônes du manifest et celle d'iPhone se servent en PNG, et se gardent un jour.
        let mut chemins: Vec<&str> = icones.iter().map(|i| i["src"].as_str().unwrap()).collect();
        chemins.push(icone_180);
        for chemin in chemins {
            let reponse = get(&app, chemin, None).await;
            assert_eq!(reponse.status(), StatusCode::OK, "{marque} : {chemin}");
            assert_eq!(reponse.headers()[header::CONTENT_TYPE], "image/png");
            assert_eq!(
                reponse.headers()[header::CACHE_CONTROL],
                "public, max-age=86400"
            );
            let octets = reponse.into_body().collect().await.unwrap().to_bytes();
            assert!(
                octets.starts_with(b"\x89PNG\r\n\x1a\n"),
                "{marque} : {chemin}"
            );
        }
    }
}

/// Au-dessus de 700 px, un tableau plus large que la page défile dans son cadre, sans que la
/// page bouge (#68, téléphone en paysage) : chaque tableau servi est dans un `.tableau-cadre`.
#[sqlx::test]
async fn chaque_tableau_a_son_cadre_defilant(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AB-123-CD").await;
    let conducteur = creer_conducteur(&pool, "Martin").await;
    // Une échéance dépassée : « Alertes » et le tableau de bord ont alors une ligne.
    let saisie = SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: "assurance".to_string(),
        date_limite: "2020-01-01".to_string(),
        statut: String::new(),
    };
    echeances::creer(&pool, &saisie).await.unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let pages = [
        "/".to_string(),
        "/vehicules".to_string(),
        "/conducteurs".to_string(),
        "/echeances".to_string(),
        "/alertes".to_string(),
        "/interventions".to_string(),
        "/affectations".to_string(),
        format!("/vehicules/{vehicule}"),
        format!("/conducteurs/{conducteur}"),
    ];
    let mut tableaux_vus = 0;
    for chemin in &pages {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        let tableaux = page.matches(r#"<table class="tableau">"#).count();
        let cadres = page.matches(r#"<div class="tableau-cadre">"#).count();
        assert_eq!(tableaux, cadres, "{chemin}\n{page}");
        tableaux_vus += tableaux;
    }
    // Les listes remplies ont bien des tableaux : le test ne passe pas sur des pages vides.
    assert!(tableaux_vus >= 6, "{tableaux_vus} tableaux vus");
}

/// La carte d'un véhicule, sur l'accueil Autoclair, se replie dans la largeur de l'écran à toute
/// taille de texte (#71). Trois règles y tiennent, et la mesure à l'écran les a toutes trois
/// trouvées nécessaires : la colonne de la grille n'est jamais plus large que l'écran (20 rem
/// grandit avec le texte), la ligne d'actions peut rétrécir sous son contenu, et la saisie du
/// kilométrage peut rétrécir à son tour.
#[test]
fn la_carte_du_vehicule_se_replie_dans_la_largeur_de_l_ecran() {
    let style = include_str!("../static/style.css");
    let regle = |debut: &str| -> String {
        let i = style
            .find(debut)
            .unwrap_or_else(|| panic!("règle introuvable : {debut}"));
        let reste = &style[i..];
        reste[..reste.find('}').expect("la règle se ferme")].to_string()
    };

    let cartes = regle(".cartes {");
    assert!(
        cartes.contains("minmax(min(20rem, 100%), 1fr)"),
        "la colonne doit pouvoir descendre sous 20 rem :\n{cartes}"
    );
    let actions = regle(".carte-vehicule > .carte-actions {");
    assert!(
        actions.contains("min-width: 0"),
        "la ligne d'actions doit pouvoir rétrécir :\n{actions}"
    );
    let releve = regle(".carte-actions .releve {");
    assert!(
        releve.contains("flex: 0 1 auto") && releve.contains("min-width: 0"),
        "la saisie du kilométrage doit pouvoir rétrécir :\n{releve}"
    );
    let champ = regle(".releve input {");
    assert!(
        champ.contains("min-width: 0"),
        "le champ doit pouvoir rétrécir :\n{champ}"
    );
}

/// Les actions d'une ligne restent sous le doigt quand un tableau défile dans son cadre (#68) :
/// au-dessus de 700 px, la colonne des actions est collée au bord droit. Et le tableau ne coupe
/// rien lui-même : un `overflow` sur `.tableau` en ferait le conteneur de référence du
/// `position: sticky`, qui n'aurait plus aucun effet sans que rien ne le dise.
#[test]
fn la_colonne_des_actions_reste_collee_au_bord_du_cadre() {
    let style = include_str!("../static/style.css");

    let debut = style
        .find("@media (width > 700px)")
        .expect("la règle des écrans larges existe");
    let media = &style[debut..];
    let media = &media[..media.find("\n}\n").expect("le bloc se ferme")];
    for attendu in [
        "td:last-child:not([data-label])",
        "th:last-child:not([data-label])",
        "position: sticky",
        "right: 0",
    ] {
        assert!(media.contains(attendu), "manque « {attendu} » :\n{media}");
    }

    let debut = style.find("\n.tableau {\n").expect("la règle du tableau");
    let regle = &style[debut..];
    let regle = &regle[..regle.find('}').expect("la règle se ferme")];
    assert!(
        !regle.contains("overflow"),
        "`.tableau` ne doit pas porter d'overflow :\n{regle}"
    );
}

/// Sur un écran tactile, la barre de défilement d'un cadre de tableau ne sert à rien : on ne
/// peut pas la saisir et elle ne dit rien que l'ombre de la colonne des actions ne dise déjà
/// (#68, retour de Christophe). Elle est masquée dans `(pointer: coarse)` seulement : à la souris
/// elle reste, c'est elle qu'on attrape.
#[test]
fn le_cadre_d_un_tableau_n_a_pas_de_barre_de_defilement_au_doigt() {
    let style = include_str!("../static/style.css");
    let debut = style
        .find("@media (pointer: coarse)")
        .expect("la règle des écrans tactiles existe");
    let media = &style[debut..];
    let media = &media[..media.find("\n}\n").expect("le bloc se ferme")];
    for attendu in [
        ".tableau-cadre { scrollbar-width: none; }",
        ".tableau-cadre::-webkit-scrollbar { display: none; }",
    ] {
        assert!(media.contains(attendu), "manque « {attendu} » :\n{media}");
    }
    // À la souris, rien n'est caché : la règle hors requête ne parle pas de barre.
    let regle = style
        .find("\n.tableau-cadre {\n")
        .expect("la règle du cadre");
    let regle = &style[regle..];
    let regle = &regle[..regle.find('}').expect("la règle se ferme")];
    assert!(!regle.contains("scrollbar"), "{regle}");
}

#[sqlx::test]
async fn les_fichiers_de_la_charte_se_servent_sans_connexion(pool: PgPool) {
    let app = application(pool);
    // Chemin, type attendu, un extrait que le fichier doit contenir.
    for (chemin, type_mime, extrait) in [
        ("/static/tokens.css", "text/css", "--marine-700: #062966"),
        ("/static/logos/autoclair.svg", "image/svg+xml", "<svg"),
        ("/static/logos/autoclair-pro.svg", "image/svg+xml", "<svg"),
        (
            "/static/logos/autoclair-negatif.svg",
            "image/svg+xml",
            "<svg",
        ),
        (
            "/static/logos/autoclair-pro-negatif.svg",
            "image/svg+xml",
            "<svg",
        ),
        ("/static/logos/icone-autoclair.svg", "image/svg+xml", "<svg"),
        (
            "/static/logos/icone-autoclair-pro.svg",
            "image/svg+xml",
            "<svg",
        ),
    ] {
        let reponse = get(&app, chemin, None).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{chemin}");
        let type_recu = reponse.headers()[header::CONTENT_TYPE].to_str().unwrap();
        assert!(type_recu.starts_with(type_mime), "{chemin} : {type_recu}");
        let page = texte(reponse).await;
        assert!(page.contains(extrait), "{chemin}");
    }

    // Les polices se servent en WOFF2, et se gardent un jour : sans cela, chaque page les
    // redemanderait.
    for chemin in [
        "/static/polices/montserrat.woff2",
        "/static/polices/instrument-sans.woff2",
    ] {
        let reponse = get(&app, chemin, None).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{chemin}");
        assert_eq!(reponse.headers()[header::CONTENT_TYPE], "font/woff2");
        assert_eq!(
            reponse.headers()[header::CACHE_CONTROL],
            "public, max-age=86400"
        );
    }

    // Seuls les chemins de la liste existent : aucun nom de fichier n'est lu dans l'adresse.
    let reponse = get(&app, "/static/polices/inconnue.woff2", None).await;
    assert_ne!(reponse.status(), StatusCode::OK);
}

/// La feuille de style n'écrit aucune couleur à elle : elle les prend dans les jetons de la
/// charte. Une teinte posée à la main ferait diverger les deux marques sans que rien ne le dise.
#[test]
fn la_feuille_de_style_n_ecrit_aucune_couleur() {
    let style = include_str!("../static/style.css");
    let jetons = include_str!("../design-system/autoclair/code/tokens.css");
    // Une valeur hexadécimale n'est tolérée que dans les icônes (« stroke='black' » n'en est
    // pas une) : cherchons « #xxxxxx » hors des lignes qui définissent une icône.
    for (numero, ligne) in style.lines().enumerate() {
        if ligne.contains("data:image/svg+xml") {
            continue;
        }
        let ligne = ligne.split("/*").next().unwrap_or("");
        let hexa = ligne.match_indices('#').any(|(i, _)| {
            let suite: String = ligne[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            matches!(suite.len(), 3 | 4 | 6 | 8)
        });
        assert!(
            !hexa,
            "style.css:{} écrit une couleur : {ligne}",
            numero + 1
        );
    }
    // Et les jetons qu'elle emploie existent tous.
    for nom in [
        "--marine-700",
        "--cyan-400",
        "--succes",
        "--danger",
        "--font-titre",
    ] {
        assert!(
            jetons.contains(&format!("{nom}:")),
            "{nom} absent de tokens.css"
        );
    }
}

/// Les affectations terminées ne forment pas la liste de travail : elles sont repliées dessous
/// (#75). Sans rien d'en cours, la page le dit et garde « Nouvelle affectation » à portée.
#[sqlx::test]
async fn les_affectations_terminees_sont_repliees_sous_la_liste(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-111-AF").await;
    let martin = creer_conducteur(&pool, "Martin").await;
    gestion_parc_auto::metier::affectations::creer(
        &pool,
        &saisie_affectation(vehicule, martin, "2026-02-01"),
        aujourd_hui(),
    )
    .await
    .expect("affectation terminée");

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Rien d'en cours : le message le dit, il n'y a pas de bouton « Terminer aujourd'hui », la
    // terminée est repliée et « Nouvelle affectation » reste dans la barre de titre.
    let page = texte(get(&app, "/affectations", Some(&cookie)).await).await;
    assert!(
        page.contains("Aucune affectation en cours ni à venir."),
        "{page}"
    );
    assert!(page.contains(r#"<details class="repliee">"#), "{page}");
    assert!(page.contains("Terminées (1)"), "{page}");
    assert!(page.contains("Nouvelle affectation"), "{page}");
    assert!(!page.contains("Terminer aujourd'hui"), "{page}");

    // Avec une affectation en cours : elle est dans la liste, avant la section repliée.
    let durand = creer_conducteur(&pool, "Durand").await;
    gestion_parc_auto::metier::affectations::creer(
        &pool,
        &saisie_affectation(vehicule, durand, ""),
        aujourd_hui(),
    )
    .await
    .expect("affectation en cours");
    let page = texte(get(&app, "/affectations", Some(&cookie)).await).await;
    let replie = page
        .find(r#"<details class="repliee">"#)
        .expect("la section repliée existe");
    let en_cours = page
        .find("Terminer aujourd'hui")
        .expect("l'affectation en cours a son bouton");
    assert!(en_cours < replie, "{page}");
    assert!(page.contains("Terminées (1)"), "{page}");
    assert!(
        !page.contains("Aucune affectation en cours ni à venir."),
        "{page}"
    );
}

/// Un véhicule dit qui l'a : ceux en cours, à défaut le dernier avec sa date de fin, dans la
/// liste et en tête de fiche (#76). Le profil particulier n'a pas d'affectations : pas de colonne.
#[sqlx::test]
async fn le_vehicule_dit_qui_l_a_ou_qui_l_a_eu_en_dernier(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let occupe = creer_vehicule(&pool, "QA-111-QA").await;
    let rendu = creer_vehicule(&pool, "QB-222-QB").await;
    let jamais = creer_vehicule(&pool, "QC-333-QC").await;
    let martin = creer_conducteur(&pool, "Martin").await;
    let durand = creer_conducteur(&pool, "Durand").await;
    for (vehicule, conducteur, fin) in [(occupe, martin, ""), (rendu, durand, "2026-02-01")] {
        gestion_parc_auto::metier::affectations::creer(
            &pool,
            &saisie_affectation(vehicule, conducteur, fin),
            aujourd_hui(),
        )
        .await
        .expect("affectation acceptée");
    }

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(liste.contains("<th>Conducteur</th>"), "{liste}");
    let en_cours = format!(r#"<a href="/conducteurs/{martin}">Camille Martin</a>"#);
    assert!(liste.contains(&en_cours), "{liste}");
    let dernier = format!(r#"dernier : <a href="/conducteurs/{durand}">Camille Durand</a>"#);
    assert!(liste.contains(&dernier), "{liste}");
    assert!(liste.contains("jusqu'au 01/02/2026"), "{liste}");
    // Le véhicule rendu et celui qui n'a jamais été affecté disent « aucun ».
    assert_eq!(liste.matches("<em>aucun</em>").count(), 2, "{liste}");

    // Fiche du véhicule rendu : le dernier conducteur est nommé en tête, avec sa date de fin.
    let fiche = texte(get(&app, &format!("/vehicules/{rendu}"), Some(&cookie)).await).await;
    assert!(fiche.contains("dernier conducteur : <a href"), "{fiche}");
    assert!(fiche.contains("jusqu'au 01/02/2026"), "{fiche}");
    // Fiche du véhicule occupé : le conducteur actuel, sans « dernier conducteur ».
    let fiche = texte(get(&app, &format!("/vehicules/{occupe}"), Some(&cookie)).await).await;
    assert!(fiche.contains("Camille Martin</a>"), "{fiche}");
    assert!(!fiche.contains("dernier conducteur"), "{fiche}");
    // Fiche du véhicule jamais affecté : « aucun », sans personne à nommer.
    let fiche = texte(get(&app, &format!("/vehicules/{jamais}"), Some(&cookie)).await).await;
    assert!(fiche.contains("<em>aucun</em>"), "{fiche}");
    assert!(!fiche.contains("dernier conducteur"), "{fiche}");

    let particulier = application_avec_profil(pool, Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    let liste = texte(get(&particulier, "/vehicules", Some(&cookie)).await).await;
    assert!(!liste.contains("<th>Conducteur</th>"), "{liste}");
    assert!(!liste.contains("dernier :"), "{liste}");
}

/// « Exporter (CSV) » est masqué sous 700 px, sur les cinq listes du profil entreprise (#77) :
/// la classe est posée sur chaque bouton, et la règle est dans le bloc des écrans étroits.
#[test]
fn l_export_csv_se_masque_sur_telephone() {
    let style = include_str!("../static/style.css");
    let regle = "a.bouton-discret.export-csv { display: none; }";
    let position = style.find(regle).expect("la règle de masquage existe");
    let avant = &style[..position];
    let media = avant
        .rfind("@media")
        .expect("la règle est dans un bloc @media");
    assert!(
        avant[media..].starts_with("@media (max-width: 700px)"),
        "{}",
        &avant[media..]
    );
    assert!(
        !avant[media..].contains("\n}\n"),
        "le bloc @media est déjà fermé avant la règle"
    );

    for (nom, gabarit) in [
        (
            "vehicules",
            include_str!("../templates/vehicules/liste.html"),
        ),
        (
            "conducteurs",
            include_str!("../templates/conducteurs/liste.html"),
        ),
        (
            "affectations",
            include_str!("../templates/affectations/liste.html"),
        ),
        (
            "echeances",
            include_str!("../templates/echeances/liste.html"),
        ),
        (
            "interventions",
            include_str!("../templates/interventions/liste.html"),
        ),
    ] {
        let ligne = gabarit
            .lines()
            .find(|l| l.contains(".csv"))
            .unwrap_or_else(|| panic!("{nom} : aucun lien d'export"));
        assert!(ligne.contains("export-csv"), "{nom} : {ligne}");
    }
}

/// Au plus serré (entre 700 et 1 280 px), les boutons de la colonne des actions passent à la ligne
/// et les cellules perdent de leur marge : sans cela, la colonne collée recouvre la pastille
/// d'état de la colonne d'avant (#78).
#[test]
fn le_tableau_serre_ne_laisse_pas_la_colonne_des_actions_recouvrir_la_pastille() {
    let style = include_str!("../static/style.css");
    let debut = style
        .find("@media (700px < width <= 1280px)")
        .expect("la règle du tableau serré existe");
    let media = &style[debut..];
    let media = &media[..media.find("\n}\n").expect("le bloc se ferme")];
    for attendu in [
        ".tableau-cadre .tableau th, .tableau-cadre .tableau td { padding-left: var(--space-2); padding-right: var(--space-2); }",
        ".tableau td:last-child:not([data-label]):not([data-vide]) .bouton-discret { white-space: normal; }",
    ] {
        assert!(media.contains(attendu), "manque « {attendu} » :\n{media}");
    }
}

/// Une immatriculation s'affiche en plaque dessinée partout où elle sert de clé de ligne : les
/// cinq listes et la fiche d'un véhicule.
#[sqlx::test]
async fn l_immatriculation_s_affiche_en_plaque_dans_les_listes_et_sur_la_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-111-AQ").await;
    let conducteur = creer_conducteur(&pool, "Martin").await;
    gestion_parc_auto::metier::affectations::creer(
        &pool,
        &saisie_affectation(vehicule, conducteur, ""),
        aujourd_hui(),
    )
    .await
    .expect("affectation acceptée");
    echeances::creer(
        &pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: "controle_technique".to_string(),
            date_limite: "2027-01-01".to_string(),
            statut: String::new(),
        },
    )
    .await
    .expect("échéance acceptée");

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let plaque = r#"<span class="plaque">PL-111-AQ</span>"#;
    for chemin in [
        "/vehicules".to_string(),
        "/affectations".to_string(),
        "/echeances".to_string(),
        format!("/vehicules/{vehicule}"),
        format!("/conducteurs/{conducteur}"),
    ] {
        let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
        assert!(page.contains(plaque), "{chemin} : pas de plaque\n{page}");
    }
    // Dans une liste, la plaque est le lien vers la fiche.
    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(
        liste.contains(&format!(r#"<a href="/vehicules/{vehicule}">{plaque}</a>"#)),
        "{liste}"
    );
}

/// La plaque est dessinée avec les jetons de la charte : fond blanc, bandes `marine-500`, cercle
/// d'étoiles en masque. Le « F » porte un texte de remplacement vide pour les lecteurs d'écran.
#[test]
fn la_plaque_est_dessinee_a_la_francaise() {
    let style = include_str!("../static/style.css");
    let debut = style.find("\n.plaque {\n").expect("la règle de la plaque");
    let regle = &style[debut..];
    let fin = regle
        .find("h1 .plaque")
        .expect("la fin du bloc de la plaque");
    let regle = &regle[..fin];
    for attendu in [
        "var(--marine-500)",
        "var(--blanc)",
        "border: 1.5px solid var(--encre)",
        "var(--ico-etoiles)",
        r#"content: "F" / "";"#,
    ] {
        assert!(regle.contains(attendu), "manque « {attendu} » :\n{regle}");
    }
    assert!(
        style.contains("--ico-etoiles: url("),
        "le masque des étoiles"
    );
}

/// La boîte « Supprimer » d'un véhicule dit où se règle le statut « Réformé » et y mène (#67) :
/// le message nomme le chemin, et un lien « Modifier le statut » ouvre le formulaire. Un véhicule
/// déjà réformé n'a plus rien à se faire proposer.
#[sqlx::test]
async fn la_suppression_d_un_vehicule_propose_le_statut_reforme(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let actif = creer_vehicule(&pool, "RF-111-RF").await;
    let reforme = creer_vehicule(&pool, "RF-222-RF").await;
    changer_statut_vehicule(&pool, reforme, "RF-222-RF", "reforme").await;

    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;

    let page = texte(get(&app, &format!("/vehicules/{actif}"), Some(&cookie)).await).await;
    for attendu in [
        "passez-le plutôt au statut « Réformé » (Modifier, champ Statut)",
        &format!(r#"data-confirmer-alternative="/vehicules/{actif}/modifier""#),
        r#"data-confirmer-alternative-libelle="Modifier le statut""#,
        "data-confirmation-alternative",
    ] {
        assert!(page.contains(attendu), "absent : {attendu}\n{page}");
    }
    assert!(!page.contains("Préférez le statut"), "{page}");

    // Réformé : la question reste, sans suggestion ni lien.
    let page = texte(get(&app, &format!("/vehicules/{reforme}"), Some(&cookie)).await).await;
    assert!(
        page.contains("et tout son historique ?\""),
        "la question seule\n{page}"
    );
    assert!(!page.contains("passez-le plutôt"), "{page}");
    assert!(
        !page.contains(&format!(
            r#"data-confirmer-alternative="/vehicules/{reforme}"#
        )),
        "{page}"
    );

    // Le script prend en charge l'autre choix.
    let script = include_str!("../static/app.js");
    assert!(script.contains("confirmerAlternative"), "app.js");
}

/// Les véhicules réformés ne sont pas dans la liste de travail : ils sont repliés dessous, avec
/// leur suppression définitive pour l'administrateur. Un lecteur ne la voit pas.
#[sqlx::test]
async fn les_vehicules_reformes_sont_replies_sous_la_liste(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "consultation", "lecteur").await;
    let actif = creer_vehicule(&pool, "RP-111-RP").await;
    let reforme = creer_vehicule(&pool, "RP-222-RP").await;
    changer_statut_vehicule(&pool, reforme, "RP-222-RP", "reforme").await;

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    let replie = page
        .find(r#"<details class="repliee" id="reformes">"#)
        .expect("la section repliée existe");
    let en_service = page
        .find(&format!(r#"<a href="/vehicules/{actif}">"#))
        .expect("le véhicule en service est dans la liste");
    let reforme_pos = page
        .find(&format!(r#"<a href="/vehicules/{reforme}">"#))
        .expect("le réformé est dans la page");
    assert!(
        en_service < replie,
        "le véhicule actif est avant la section\n{page}"
    );
    assert!(
        reforme_pos > replie,
        "le réformé est dans la section\n{page}"
    );
    assert!(page.contains("Réformés (1)"), "{page}");
    let suppression = format!(r#"action="/vehicules/{reforme}/supprimer""#);
    assert!(
        page.contains(&suppression),
        "suppression du réformé\n{page}"
    );
    assert!(
        !page.contains(&format!(r#"action="/vehicules/{actif}/supprimer""#)),
        "pas de suppression sur un véhicule en service\n{page}"
    );
    assert!(
        page.contains("Supprimer définitivement le véhicule RP-222-RP"),
        "{page}"
    );

    // Un lecteur voit les réformés, sans pouvoir les supprimer.
    let lecteur = se_connecter(&app, "consultation").await;
    let page = texte(get(&app, "/vehicules", Some(&lecteur)).await).await;
    assert!(page.contains("Réformés (1)"), "{page}");
    assert!(!page.contains(&suppression), "{page}");
}

/// Sans véhicule en service, la page le dit, et « Nouveau véhicule » reste dans la barre de titre.
#[sqlx::test]
async fn sans_vehicule_en_service_la_liste_garde_les_reformes_et_le_bouton(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let reforme = creer_vehicule(&pool, "RQ-111-RQ").await;
    changer_statut_vehicule(&pool, reforme, "RQ-111-RQ", "reforme").await;

    let app = application(pool);
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(
        page.contains("Aucun véhicule en service ni en maintenance."),
        "{page}"
    );
    assert!(page.contains("Réformés (1)"), "{page}");
    assert!(page.contains("Nouveau véhicule"), "{page}");
    assert!(!page.contains("Aucun véhicule enregistré."), "{page}");

    // Le script ouvre la section visée par l'adresse.
    let script = include_str!("../static/app.js");
    assert!(script.contains("location.hash"), "app.js");
}

/// Les corps de règles (ce qui est entre accolades) d'un extrait de feuille de style, espaces
/// normalisés, dans l'ordre.
fn corps_des_regles(css: &str) -> Vec<String> {
    css.split('{')
        .skip(1)
        .filter_map(|reste| reste.split('}').next())
        .map(|corps| corps.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// Un tableau plus large que son cadre se présente en fiches (#83) : la feuille de style a un bloc
/// `.en-fiches` dont les règles « fiche » sont celles du bloc sous 700 px, et `app.js` pose la
/// classe d'après la mesure. Les deux copies ne doivent pas diverger.
#[test]
fn un_tableau_trop_large_se_presente_en_fiches_comme_sous_700_px() {
    let style = include_str!("../static/style.css");

    // De la première règle « fiche » à la fin de la dernière.
    let extrait = |debut: &str, derniere: &str| -> String {
        let a = style
            .find(debut)
            .unwrap_or_else(|| panic!("absent : {debut}"));
        let b = a + style[a..]
            .find(derniere)
            .unwrap_or_else(|| panic!("absent : {derniere}"));
        let c = b + style[b..].find('}').expect("la règle se ferme") + 1;
        style[a..c].to_string()
    };
    let mobile = corps_des_regles(&extrait(
        "  .tableau, .tableau tbody, .tableau tr, .tableau td { display: block; }",
        "  .tableau td:last-child:not([data-label]) { margin-top: 0.3rem;",
    ));
    let en_fiches = corps_des_regles(&extrait(
        "  .tableau-cadre.en-fiches .tableau, .tableau-cadre.en-fiches .tableau tbody",
        "  .tableau-cadre.en-fiches .tableau td:last-child:not([data-label]) { margin-top: 0.3rem;",
    ));
    assert_eq!(mobile.len(), 12, "règles du bloc sous 700 px : {mobile:#?}");
    assert_eq!(
        mobile, en_fiches,
        "les règles « fiche » de `.en-fiches` doivent rester celles du bloc sous 700 px"
    );

    // Écran seulement : le papier garde ses tableaux.
    let bloc = style
        .find(
            "@media screen {
  .tableau-cadre.en-fiches",
        )
        .expect("le bloc `.en-fiches` est dans @media screen");
    assert!(
        bloc > style.find("@media (max-width: 700px) {").unwrap(),
        "le bloc `.en-fiches` suit le bloc sous 700 px"
    );

    // `app.js` mesure, pose la classe et la refait quand un cadre change de taille.
    let script = include_str!("../static/app.js");
    for attendu in [
        "cadre.scrollWidth > cadre.clientWidth",
        "classList.add(\"en-fiches\")",
        "classList.remove(\"en-fiches\")",
        "ResizeObserver",
    ] {
        assert!(script.contains(attendu), "app.js : manque « {attendu} »");
    }
}
