//! Ce qui a été fait pendant une intervention (#94, bloc 1) : les lignes d'opérations, de la
//! saisie à la fiche, par le formulaire HTML et par l'API JSON.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::erreurs::ErreurApp;
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, requete_json,
    se_connecter, texte,
};

fn ligne(operation: &str, reference: &str, quantite: &str, prix: &str) -> SaisieLigne {
    SaisieLigne {
        operation: operation.to_string(),
        libelle: String::new(),
        reference: reference.to_string(),
        quantite: quantite.to_string(),
        prix: prix.to_string(),
    }
}

fn saisie(
    vehicule: i64,
    date: &str,
    kilometrage: &str,
    lignes: Vec<SaisieLigne>,
) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".to_string(),
        date_intervention: date.to_string(),
        kilometrage: kilometrage.to_string(),
        cout: "189,90".to_string(),
        prestataire: "Garage fictif".to_string(),
        notes: String::new(),
        lignes,
    }
}

async fn nombre_de_lignes(pool: &PgPool, intervention: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM lignes_intervention WHERE intervention_id = $1")
        .bind(intervention)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn les_lignes_se_lisent_dans_l_ordre_du_vocabulaire_se_remplacent_et_partent_avec_l_intervention(
    pool: PgPool,
) {
    let vehicule = creer_vehicule(&pool, "OP-100-OP").await;
    let mut autre = ligne("autre", "", "", "49,90");
    autre.libelle = "Joint de culasse".to_string();
    let creee = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![
                autre,
                ligne("filtre_huile", "W712", "2", "12,50"),
                ligne("vidange", "5W30", "", ""),
            ],
        ),
    )
    .await
    .unwrap();

    // Relue dans l'ordre du vocabulaire, « autre » en dernier, quel que soit l'ordre de saisie.
    let codes: Vec<&str> = creee.lignes.iter().map(|l| l.operation.as_str()).collect();
    assert_eq!(codes, ["vidange", "filtre_huile", "autre"]);
    assert_eq!(
        creee.operations_fr(),
        "Vidange (réf. 5W30) · Filtre à huile (réf. W712, × 2, 12,50 €) · Joint de culasse (49,90 €)"
    );
    // Même chose par les listes.
    let par_vehicule = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(par_vehicule[0].lignes.len(), 3);
    assert_eq!(
        interventions::lister(&pool).await.unwrap()[0].lignes.len(),
        3
    );

    // Modifier remplace l'ensemble des lignes.
    let modifiee = interventions::modifier(
        &pool,
        creee.id,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![ligne("pneus", "", "4", "412")],
        ),
    )
    .await
    .unwrap();
    assert_eq!(modifiee.operations_fr(), "Pneus (× 4, 412,00 €)");
    assert_eq!(nombre_de_lignes(&pool, creee.id).await, 1);

    // Et une intervention sans ligne n'a rien à dire.
    let vide = interventions::modifier(
        &pool,
        creee.id,
        &saisie(vehicule, "2026-05-05", "20000", vec![]),
    )
    .await
    .unwrap();
    assert_eq!(vide.operations_fr(), "");
    assert_eq!(nombre_de_lignes(&pool, creee.id).await, 0);

    // Supprimer l'intervention emporte ses lignes (cascade).
    let avec = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-06-06",
            "21000",
            vec![ligne("batterie", "", "", "")],
        ),
    )
    .await
    .unwrap();
    assert_eq!(nombre_de_lignes(&pool, avec.id).await, 1);
    interventions::supprimer(&pool, avec.id).await.unwrap();
    assert_eq!(nombre_de_lignes(&pool, avec.id).await, 0);
}

#[sqlx::test]
async fn une_ligne_refusee_n_enregistre_rien_et_la_base_tient_ses_regles(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "OP-200-OP").await;

    // « Autre » sans libellé : refus lisible, et ni l'intervention ni le compteur ne bougent.
    let refus = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "99000",
            vec![ligne("autre", "", "", "")],
        ),
    )
    .await;
    assert!(matches!(refus, Err(ErreurApp::Metier(m)) if m.contains("Autre")));
    assert!(interventions::lister(&pool).await.unwrap().is_empty());
    assert_eq!(
        vehicules::obtenir(&pool, vehicule)
            .await
            .unwrap()
            .kilometrage,
        12000
    );

    // La base elle-même refuse un doublon et un « autre » sans libellé, même en la contournant.
    let creee = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![ligne("vidange", "", "", "")],
        ),
    )
    .await
    .unwrap();
    let doublon = sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation) VALUES ($1, 'vidange')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(gestion_parc_auto::erreurs::a_le_code(
        &doublon,
        gestion_parc_auto::erreurs::CODE_VIOLATION_UNICITE
    ));
    let sans_libelle = sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation) VALUES ($1, 'autre')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(gestion_parc_auto::erreurs::a_le_code(
        &sans_libelle,
        gestion_parc_auto::erreurs::CODE_VIOLATION_CHECK
    ));
    // Deux « autre » distincts, en revanche, passent.
    sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation, libelle)
         VALUES ($1, 'autre', 'Joint'), ($1, 'autre', 'Durite')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn le_formulaire_enregistre_les_cases_cochees_et_les_remontre(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "OP-300-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire vierge propose toutes les opérations, aucune cochée.
    let page = texte(get(&app, "/interventions/nouvelle", Some(&cookie)).await).await;
    assert!(page.contains("Ce qui a été fait"), "{page}");
    for libelle in [
        "Pièces et consommables",
        "Vidange",
        "Filtre à huile",
        "Courroie de distribution",
        "Contrôles et réglages",
        "Pression des pneus",
        "Niveaux",
        "Autre",
        "Entretien courant",
    ] {
        assert!(page.contains(libelle), "{libelle} absent\n{page}");
    }
    assert!(!page.contains(" checked"), "{page}");

    // Deux cases cochées ; les champs d'une case non cochée (filtre à air) voyagent aussi et
    // sont ignorés.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&cout=149%2C90&prestataire=Garage&notes=\
         &operation=vidange&reference_vidange=5W30&quantite_vidange=&prix_vidange=45\
         &reference_filtre_air=oubli%C3%A9&quantite_filtre_air=3\
         &operation=autre&libelle_autre=Joint&reference_autre=&quantite_autre=&prix_autre="
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}")
    );

    let attendu = "Vidange (réf. 5W30, 45,00 €) · Joint";
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(attendu), "{fiche}");
    assert!(!fiche.contains("oublié"), "{fiche}");
    let liste = texte(get(&app, "/interventions", Some(&cookie)).await).await;
    assert!(liste.contains(attendu), "{liste}");

    // « Modifier » remontre les cases cochées avec leurs valeurs, les autres vides.
    let id = interventions::lister(&pool).await.unwrap()[0].id;
    let formulaire = texte(
        get(
            &app,
            &format!("/interventions/{id}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        formulaire.contains(r#"value="vidange" checked"#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"name="reference_vidange" value="5W30""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"name="prix_vidange" value="45.00""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"value="autre" checked"#)
            && formulaire.contains(r#"name="libelle_autre" value="Joint""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"value="filtre_air"> Filtre à air"#),
        "{formulaire}"
    );

    // Tout décocher vide les lignes.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&cout=149%2C90&prestataire=Garage&notes=&reference_vidange=5W30"
    );
    let reponse =
        post_formulaire(&app, &format!("/interventions/{id}"), Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("5W30"), "{fiche}");

    // Un refus réaffiche le formulaire avec le message et la saisie.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&operation=autre&libelle_autre=&operation=pneus&quantite_pneus=4"
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"role="alert""#) && page.contains("Autre"),
        "{page}"
    );
    assert!(
        page.contains(r#"value="pneus" checked"#)
            && page.contains(r#"name="quantite_pneus" value="4""#),
        "{page}"
    );
    assert_eq!(interventions::lister(&pool).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn l_api_json_porte_les_lignes(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "OP-400-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"revision","date_intervention":"2026-05-10",
            "kilometrage":"13000","cout":"149,90",
            "lignes":[{{"operation":"vidange","prix":"45"}},
                      {{"operation":"filtre_huile","reference":"W712","quantite":"1","prix":"12,50"}}]}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/interventions",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let cree: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let id = cree["id"].as_i64().unwrap();
    assert_eq!(cree["lignes"].as_array().unwrap().len(), 2);
    assert_eq!(cree["lignes"][0]["operation"], "vidange");
    assert_eq!(cree["lignes"][0]["prix_centimes"], 4500);
    assert_eq!(cree["lignes"][1]["reference_piece"], "W712");

    // Un lecteur les lit aussi.
    let lecture = se_connecter(&app, "lecture").await;
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/interventions/{id}"),
        Some(&lecture),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let lue: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(lue["lignes"].as_array().unwrap().len(), 2);

    // Une ligne fautive est refusée comme toute saisie.
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"revision","date_intervention":"2026-05-10",
            "kilometrage":"13000","lignes":[{{"operation":"vidange_moteur"}}]}}"#
    );
    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/interventions/{id}"),
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        interventions::obtenir(&pool, id)
            .await
            .unwrap()
            .lignes
            .len(),
        2,
        "le refus n'a rien changé"
    );
}
