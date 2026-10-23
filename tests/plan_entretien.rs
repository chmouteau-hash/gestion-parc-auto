//! Le plan d'entretien d'un véhicule (#94, bloc 2) : les opérations suivies, leur dernière fois
//! calculée à partir de l'entretien, et leur prochaine borne, de la saisie à la fiche, par les
//! formulaires et par l'API JSON.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local, Months, NaiveDate};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::alertes::Horizon;
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::plan_entretien::{self, EtatPlan, SaisiePlan};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    requete_json, se_connecter, texte,
};

const HORIZON: Horizon = Horizon {
    jours: 30,
    km: 1000,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn il_y_a(jours: i64) -> NaiveDate {
    aujourd_hui() - Duration::days(jours)
}

/// Une révision avec une ligne « vidange », datée et kilométrée.
fn vidange(vehicule: i64, date: NaiveDate, kilometrage: i32) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".into(),
        date_intervention: date.to_string(),
        kilometrage: kilometrage.to_string(),
        lignes: vec![SaisieLigne {
            operation: "vidange".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Le véhicule de test (`creer_vehicule`) est mis en service le 01/06/2023, à 12 000 km au
/// compteur.
#[sqlx::test]
async fn suivre_une_operation_avec_sa_derniere_fois(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-200-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire propose les opérations en deux groupes, sans « Autre ».
    let page = texte(
        get(
            &app,
            &format!("/plan-entretien/nouveau?vehicule_id={vehicule}"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"<optgroup label="Pièces et consommables">"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<optgroup label="Contrôles et réglages">"#),
        "{page}"
    );
    assert!(page.contains(r#"value="vidange""#), "{page}");
    assert!(!page.contains(r#"value="autre""#), "{page}");
    assert!(page.contains(r#"name="derniere_date""#), "{page}");

    // Vidange tous les 15 000 km ou 12 mois, faite il y a 100 jours à 20 000 km.
    let derniere = il_y_a(100);
    let corps = format!(
        "vehicule_id={vehicule}&operation=vidange&periodicite_km=15+000&periodicite_mois=12\
         &derniere_date={derniere}&derniere_kilometrage=20000"
    );
    let reponse = post_formulaire(&app, "/plan-entretien", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}")
    );

    // La dernière fois est une vraie intervention, de type Révision, avec sa ligne ; le compteur
    // a suivi.
    let liste = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0].type_intervention, "revision");
    assert_eq!(liste[0].date_intervention, derniere);
    assert_eq!(liste[0].operations_fr(), "Vidange");
    assert_eq!(liste[0].notes, plan_entretien::NOTE_DERNIERE_FOIS);
    assert_eq!(
        vehicules::obtenir(&pool, vehicule)
            .await
            .unwrap()
            .kilometrage,
        20000
    );

    let prochaine = derniere.checked_add_months(Months::new(12)).unwrap();
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    for attendu in [
        "Plan d'entretien".to_string(),
        "Suivre une opération".to_string(),
        "tous les 15\u{202F}000 km ou tous les 12 mois".to_string(),
        format!("{} à 20\u{202F}000 km", derniere.format("%d/%m/%Y")),
        "35\u{202F}000 km · dans 15\u{202F}000 km".to_string(),
        format!(
            "{} · dans {} jours",
            prochaine.format("%d/%m/%Y"),
            (prochaine - aujourd_hui()).num_days()
        ),
        r#"<span class="pastille plan-a_jour">À jour</span>"#.to_string(),
        r#"class="trajet plan-a_jour trajet-"#.to_string(),
        "Ne plus suivre".to_string(),
    ] {
        assert!(fiche.contains(&attendu), "{attendu} absent\n{fiche}");
    }

    // La même opération une seconde fois : refusée, le formulaire revient avec le motif.
    let reponse = post_formulaire(
        &app,
        "/plan-entretien",
        Some(&cookie),
        &format!("vehicule_id={vehicule}&operation=vidange&periodicite_mois=6"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("est déjà suivie pour ce véhicule"), "{page}");
    // Et elle n'est plus proposée.
    assert!(!page.contains(r#"value="vidange""#), "{page}");

    // Autoclair (particulier) a le même plan sur la fiche.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    let fiche = texte(
        get(
            &particulier,
            &format!("/vehicules/{vehicule}"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        fiche.contains("Plan d'entretien") && fiche.contains("35\u{202F}000 km"),
        "{fiche}"
    );
}

/// La dernière fois se calcule à partir de l'entretien : jamais notée, l'opération n'est pas « en
/// retard » ; l'ordre de saisie des interventions anciennes est indifférent ; une intervention
/// datée dans le futur ne compte pas.
#[sqlx::test]
async fn la_derniere_fois_se_calcule_a_partir_de_l_entretien(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-210-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Tous les 10 000 km, jamais notée : la borne comptée depuis la mise en service (10 000 km)
    // est passée (12 000 km), mais on ne sait pas quand la vidange a été faite.
    let (ligne, intervention) = plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "10000".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    assert_eq!(intervention, None);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(r#"<span class="pastille plan-inconnue">Dernière fois inconnue</span>"#),
        "{fiche}"
    );
    assert!(!fiche.contains("plan-depassee"), "{fiche}");
    assert!(
        fiche.contains("Notez-la en ajoutant l'intervention"),
        "{fiche}"
    );

    // Trois vidanges anciennes, saisies dans le désordre : la plus récente par date fait foi.
    for (jours, km) in [(500, 4000), (200, 9000), (800, 2000)] {
        interventions::creer(&pool, &vidange(vehicule, il_y_a(jours), km))
            .await
            .unwrap();
    }
    // Une vidange prévue dans un mois n'est pas encore faite.
    interventions::creer(&pool, &vidange(vehicule, il_y_a(-30), 11000))
        .await
        .unwrap();
    let plan = plan_entretien::plan_du_vehicule(&pool, vehicule, aujourd_hui(), HORIZON)
        .await
        .unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].ligne.id, ligne.id);
    let derniere = plan[0].derniere_fois.unwrap();
    assert_eq!((derniere.date, derniere.kilometrage), (il_y_a(200), 9000));
    assert_eq!(plan[0].prochaine_km, Some(19000));
    assert_eq!(plan[0].reste_km, Some(7000));
    assert_eq!(plan[0].etat, EtatPlan::AJour);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains("19\u{202F}000 km · dans 7\u{202F}000 km"),
        "{fiche}"
    );

    // Les mêmes interventions saisies dans l'ordre chronologique, sur un autre véhicule, donnent
    // la même prochaine.
    let autre = creer_vehicule(&pool, "PL-211-AN").await;
    for (jours, km) in [(800, 2000), (500, 4000), (200, 9000)] {
        interventions::creer(&pool, &vidange(autre, il_y_a(jours), km))
            .await
            .unwrap();
    }
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: autre.to_string(),
            operation: "vidange".into(),
            periodicite_km: "10000".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let autre_plan = plan_entretien::plan_du_vehicule(&pool, autre, aujourd_hui(), HORIZON)
        .await
        .unwrap();
    assert_eq!(autre_plan[0].prochaine_km, plan[0].prochaine_km);
    assert_eq!(
        autre_plan[0].derniere_fois.map(|r| r.date),
        Some(il_y_a(200))
    );

    // Corriger l'intervention la plus récente corrige la prochaine, sans autre geste.
    let recente = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .find(|i| i.date_intervention == il_y_a(200))
        .unwrap();
    interventions::modifier(&pool, recente.id, &vidange(vehicule, il_y_a(200), 9500))
        .await
        .unwrap();
    let plan = plan_entretien::plan_du_vehicule(&pool, vehicule, aujourd_hui(), HORIZON)
        .await
        .unwrap();
    assert_eq!(plan[0].prochaine_km, Some(19500));
}

#[sqlx::test]
async fn modifier_puis_ne_plus_suivre(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "PL-220-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;
    let (ligne, _) = plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "filtre_air".into(),
            periodicite_km: "30000".into(),
            periodicite_mois: "24".into(),
            derniere_date: il_y_a(30).to_string(),
            derniere_kilometrage: "11000".into(),
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let id = ligne.id;

    // La modification garde l'opération proposée et ne redemande pas la dernière fois.
    let page = texte(
        get(
            &app,
            &format!("/plan-entretien/{id}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"<option value="filtre_air" selected>"#),
        "{page}"
    );
    assert!(!page.contains(r#"name="derniere_date""#), "{page}");
    assert!(page.contains("La dernière fois se change dans l"), "{page}");

    // Sans périodicité : refusé, le formulaire revient.
    let reponse = post_formulaire(
        &app,
        &format!("/plan-entretien/{id}"),
        Some(&cookie),
        "operation=filtre_air&periodicite_km=&periodicite_mois=",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse).await.contains("tous les combien la refaire"));

    let reponse = post_formulaire(
        &app,
        &format!("/plan-entretien/{id}"),
        Some(&cookie),
        "operation=filtre_air&periodicite_km=20000&periodicite_mois=",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let modifiee = plan_entretien::obtenir(&pool, id).await.unwrap();
    assert_eq!(
        (modifiee.periodicite_km, modifiee.periodicite_mois),
        (Some(20000), None)
    );
    assert_eq!(modifiee.vehicule_id, vehicule);

    // Le lecteur voit le plan, sans rien pour le changer.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&lecteur)).await).await;
    assert!(
        fiche.contains("Filtre à air") && fiche.contains("tous les 20\u{202F}000 km"),
        "{fiche}"
    );
    assert!(
        !fiche.contains("Suivre une opération") && !fiche.contains("Ne plus suivre"),
        "{fiche}"
    );
    assert_eq!(
        get(
            &app,
            &format!("/plan-entretien/nouveau?vehicule_id={vehicule}"),
            Some(&lecteur)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_formulaire(
            &app,
            &format!("/plan-entretien/{id}/supprimer"),
            Some(&lecteur),
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    // Ne plus suivre : la ligne part, l'intervention de la dernière fois reste.
    let reponse = post_formulaire(
        &app,
        &format!("/plan-entretien/{id}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(plan_entretien::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        interventions::lister_par_vehicule(&pool, vehicule)
            .await
            .unwrap()
            .len(),
        1
    );
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains("Aucune opération suivie."), "{fiche}");
}

/// L'API lit le plan calculé et l'écrit ; la dernière fois d'un contrôle est un « Entretien
/// courant ».
#[sqlx::test]
async fn l_api_lit_et_ecrit_le_plan(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "PL-230-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;

    let derniere = il_y_a(10);
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","operation":"pression_pneus","periodicite_mois":"1",
            "derniere_date":"{derniere}","derniere_kilometrage":"12100"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/plan-entretien",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let cree: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let id = cree["id"].as_i64().unwrap();
    assert_eq!(cree["operation"], "pression_pneus");
    assert_eq!(cree["periodicite_mois"], 1);
    assert!(cree["periodicite_km"].is_null());
    let liste = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(liste[0].type_intervention, "entretien_courant");
    assert_eq!(liste[0].operations_fr(), "Pression des pneus");

    // Le lecteur lit le plan calculé.
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/plan-entretien"),
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let plan: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let prochaine = derniere.checked_add_months(Months::new(1)).unwrap();
    assert_eq!(plan[0]["id"], id);
    // Dans vingt jours environ : dans l'horizon des alertes (30 jours), donc « à venir ».
    assert_eq!(plan[0]["etat"], "a_venir");
    assert_eq!(plan[0]["borne"], "date");
    assert_eq!(plan[0]["derniere_fois"]["kilometrage"], 12100);
    assert_eq!(plan[0]["prochaine_date"], prochaine.to_string());
    assert_eq!(
        plan[0]["reste_jours"],
        (prochaine - aujourd_hui()).num_days()
    );
    assert!(plan[0]["prochaine_km"].is_null());

    // Le lecteur n'écrit pas.
    for (methode, chemin, corps) in [
        ("POST", "/api/v1/plan-entretien".to_string(), Some("{}")),
        ("PUT", format!("/api/v1/plan-entretien/{id}"), Some("{}")),
        ("DELETE", format!("/api/v1/plan-entretien/{id}"), None),
    ] {
        let reponse = requete_json(&app, methode, &chemin, Some(&lecteur), corps).await;
        assert_eq!(
            reponse.status(),
            StatusCode::FORBIDDEN,
            "{methode} {chemin}"
        );
    }

    // « Autre » ne se suit pas.
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/plan-entretien",
        Some(&cookie),
        Some(&format!(
            r#"{{"vehicule_id":"{vehicule}","operation":"autre","periodicite_mois":"6"}}"#
        )),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);

    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/plan-entretien/{id}"),
        Some(&cookie),
        Some(r#"{"operation":"pression_pneus","periodicite_mois":"2"}"#),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let modifiee: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(modifiee["periodicite_mois"], 2);
    assert_eq!(modifiee["vehicule_id"], vehicule);

    let reponse = requete_json(
        &app,
        "DELETE",
        &format!("/api/v1/plan-entretien/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        requete_json(
            &app,
            "GET",
            &format!("/api/v1/plan-entretien/{id}"),
            Some(&cookie),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

/// Règle (b) du bloc 2 : le formulaire d'intervention avertit d'une date dans le futur, que le
/// plan ne compte pas comme faite (le script `app.js` pose la question avant l'envoi).
#[sqlx::test]
async fn le_formulaire_d_intervention_avertit_d_une_date_future(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/interventions/nouvelle", Some(&cookie)).await).await;
    assert!(
        page.contains(r#"name="date_intervention""#)
            && page.contains("required data-avertir-futur>"),
        "{page}"
    );
    let script = include_str!("../static/app.js");
    assert!(
        script.contains("input[type=date][data-avertir-futur]")
            && script.contains("est dans le futur")
            && script.contains("ne comptera l'intervention comme faite"),
        "app.js"
    );
}
