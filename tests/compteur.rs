//! L'historique du compteur (#94, bloc 4 ; retour #4) : le compteur d'un véhicule est le plus
//! haut kilométrage de ses relevés, de ses interventions et de ses pleins. Il se recalcule à
//! chaque écriture : corriger ou supprimer la saisie fautive le fait redescendre.

mod commun;

use axum::http::{header, StatusCode};
use chrono::Duration;
use gestion_parc_auto::metier::depenses::{self, SaisieDepense};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, requete_json,
    se_connecter, texte,
};

/// Le serveur date les relevés du vrai jour : les tests aussi (`commun::aujourd_hui` est une date
/// fixe).
fn aujourd_hui() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

async fn compteur(pool: &PgPool, vehicule: i64) -> i32 {
    vehicules::obtenir(pool, vehicule)
        .await
        .unwrap()
        .kilometrage
}

fn intervention(vehicule: i64, kilometrage: &str) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".into(),
        date_intervention: aujourd_hui().to_string(),
        kilometrage: kilometrage.into(),
        ..Default::default()
    }
}

fn plein(vehicule: i64, kilometrage: &str) -> SaisieDepense {
    SaisieDepense {
        vehicule_id: vehicule.to_string(),
        type_depense: "carburant".into(),
        date_depense: aujourd_hui().to_string(),
        montant: "60".into(),
        carburant: "gazole".into(),
        quantite: "35".into(),
        kilometrage: kilometrage.into(),
        plein_complet: "oui".into(),
        libelle: String::new(),
        notes: String::new(),
    }
}

/// Le code du message posé par la redirection (cookie `fait`).
fn fait(reponse: &axum::response::Response) -> String {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .find(|c| c.starts_with("fait="))
        .unwrap_or_default()
}

/// Les relevés faits à la main du véhicule : (date, kilométrage), dans l'ordre de leur saisie.
async fn releves(pool: &PgPool, vehicule: i64) -> Vec<(chrono::NaiveDate, i32)> {
    sqlx::query_as::<_, (chrono::NaiveDate, i32)>(
        "SELECT date_releve, kilometrage FROM releves_kilometrage WHERE vehicule_id = $1 ORDER BY id",
    )
    .bind(vehicule)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn id_du_releve(pool: &PgPool, vehicule: i64, kilometrage: i32) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT id FROM releves_kilometrage WHERE vehicule_id = $1 AND kilometrage = $2",
    )
    .bind(vehicule)
    .bind(kilometrage)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn corriger_ou_supprimer_une_intervention_fautive_fait_redescendre_le_compteur(pool: PgPool) {
    // Le retour #4 : 99 000 tapé pour 13 000.
    let vehicule = creer_vehicule(&pool, "CP-100-AN").await;
    // Le kilométrage de la création est le premier relevé.
    assert_eq!(
        releves(&pool, vehicule).await,
        vec![(aujourd_hui(), 12_000)]
    );
    let faute = interventions::creer(&pool, &intervention(vehicule, "99000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 99_000);

    interventions::modifier(&pool, faute.id, &intervention(vehicule, "13000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 13_000);

    interventions::supprimer(&pool, faute.id).await.unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 12_000);
}

#[sqlx::test]
async fn un_plein_compte_dans_l_historique_jusqu_a_sa_correction(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "CP-200-AN").await;
    let faute = depenses::creer(&pool, &plein(vehicule, "50000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 50_000);

    depenses::modifier(&pool, faute.id, &plein(vehicule, "14000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 14_000);
    // Sans kilométrage, le plein ne compte plus.
    depenses::modifier(&pool, faute.id, &plein(vehicule, ""))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 12_000);

    depenses::modifier(&pool, faute.id, &plein(vehicule, "20000"))
        .await
        .unwrap();
    depenses::supprimer(&pool, faute.id).await.unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 12_000);
}

#[sqlx::test]
async fn deplacer_une_intervention_recalcule_les_deux_compteurs(pool: PgPool) {
    let premier = creer_vehicule(&pool, "CP-300-AN").await;
    let second = creer_vehicule(&pool, "CP-301-AN").await;
    let erreur = interventions::creer(&pool, &intervention(premier, "30000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, premier).await, 30_000);

    // Saisie sur le mauvais véhicule, puis corrigée.
    interventions::modifier(&pool, erreur.id, &intervention(second, "30000"))
        .await
        .unwrap();
    assert_eq!(compteur(&pool, premier).await, 12_000);
    assert_eq!(compteur(&pool, second).await, 30_000);
}

#[sqlx::test]
async fn relever_puis_corriger_et_supprimer_un_releve(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CP-400-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // « Relever » sur la fiche : un relevé daté du jour.
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{vehicule}/kilometrage?retour=fiche"),
        Some(&cookie),
        "kilometrage=15000",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(compteur(&pool, vehicule).await, 15_000);
    assert_eq!(
        releves(&pool, vehicule).await,
        vec![(aujourd_hui(), 12_000), (aujourd_hui(), 15_000)]
    );

    // Plus bas que le compteur : refusé, avec la voie de la correction.
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{vehicule}/kilometrage?retour=fiche"),
        Some(&cookie),
        "kilometrage=14000",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("ne peut pas baisser") && page.contains("historique du compteur"),
        "{page}"
    );

    // Le relevé fautif se corrige : le compteur redescend.
    let id = id_du_releve(&pool, vehicule, 15_000).await;
    let formulaire =
        texte(get(&app, &format!("/releves/{id}/modifier"), Some(&cookie)).await).await;
    assert!(
        formulaire.contains(r#"name="kilometrage" value="15000""#),
        "{formulaire}"
    );
    let reponse = post_formulaire(
        &app,
        &format!("/releves/{id}"),
        Some(&cookie),
        &format!("date_releve={}&kilometrage=14000", aujourd_hui()),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}/compteur").as_str()
    );
    assert!(fait(&reponse).starts_with("fait=releve-modifie;"));
    assert_eq!(compteur(&pool, vehicule).await, 14_000);

    // Un relevé dans le futur est refusé, la saisie revient.
    let demain = aujourd_hui() + Duration::days(1);
    let reponse = post_formulaire(
        &app,
        &format!("/releves/{id}"),
        Some(&cookie),
        &format!("date_releve={demain}&kilometrage=14500"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("ne peut pas être dans le futur"), "{page}");
    assert!(page.contains(r#"value="14500""#), "{page}");
    assert_eq!(compteur(&pool, vehicule).await, 14_000);

    // Supprimé : le compteur revient au relevé de la création.
    let reponse =
        post_formulaire(&app, &format!("/releves/{id}/supprimer"), Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(fait(&reponse).starts_with("fait=releve-supprime;"));
    assert_eq!(compteur(&pool, vehicule).await, 12_000);
}

#[sqlx::test]
async fn l_historique_du_compteur_montre_les_trois_sources(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "CP-500-AN").await;
    let mut ancienne = intervention(vehicule, "9000");
    ancienne.date_intervention = (aujourd_hui() - Duration::days(400)).to_string();
    interventions::creer(&pool, &ancienne).await.unwrap();
    depenses::creer(&pool, &plein(vehicule, "12500"))
        .await
        .unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;

    // La fiche y mène, pour tous.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&lecteur)).await).await;
    assert!(
        fiche.contains(&format!(
            r#"href="/vehicules/{vehicule}/compteur">historique</a>"#
        )),
        "{fiche}"
    );

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
        page.contains("Historique du compteur · CP-500-AN"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            "Compteur : <strong>12\u{202F}500 km</strong>, d'après le plein du {}.",
            aujourd_hui().format("%d/%m/%Y")
        )),
        "{page}"
    );
    // Le plus récent en premier : le plein, le relevé de la création, l'intervention ancienne.
    let plein_a = page.find("<td>Plein").expect("le plein");
    let releve_a = page.find("<td>Relevé").expect("le relevé");
    let intervention_a = page
        .find("<td>Intervention · Révision")
        .expect("l'intervention");
    assert!(plein_a < releve_a && releve_a < intervention_a, "{page}");
    assert!(page.contains("Le compteur vient d'ici"), "{page}");
    assert!(
        page.contains(">Corriger</a>") && page.contains(r#"action="/releves/"#),
        "{page}"
    );

    // Le lecteur voit l'historique, sans rien pour le corriger.
    let page = texte(
        get(
            &app,
            &format!("/vehicules/{vehicule}/compteur"),
            Some(&lecteur),
        )
        .await,
    )
    .await;
    assert!(page.contains("<td>Relevé"), "{page}");
    assert!(
        !page.contains(">Corriger</a>") && !page.contains(r#"action="/releves/"#),
        "{page}"
    );
    let id = id_du_releve(&pool, vehicule, 12_000).await;
    for (chemin, corps) in [
        (
            format!("/releves/{id}"),
            "date_releve=2026-01-01&kilometrage=1",
        ),
        (format!("/releves/{id}/supprimer"), ""),
    ] {
        assert_eq!(
            post_formulaire(&app, &chemin, Some(&lecteur), corps)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        get(&app, &format!("/releves/{id}/modifier"), Some(&lecteur))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test]
async fn le_kilometrage_ne_se_change_plus_en_modifiant_le_vehicule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CP-600-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La création le demande ; la modification renvoie à l'historique.
    let creation = texte(get(&app, "/vehicules/nouveau", Some(&cookie)).await).await;
    assert!(creation.contains(r#"name="kilometrage""#));
    let modification = texte(
        get(
            &app,
            &format!("/vehicules/{vehicule}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        !modification.contains(r#"name="kilometrage""#),
        "{modification}"
    );
    assert!(
        modification.contains(&format!(
            r#"href="/vehicules/{vehicule}/compteur">historique du compteur</a>"#
        )),
        "{modification}"
    );

    // Par l'API : le même kilométrage passe, un autre est refusé.
    let corps = |kilometrage: &str| {
        format!(
            r#"{{"immatriculation":"CP-600-AN","marque":"Renault","modele":"Kangoo","date_mise_en_service":"2023-06-01","carburant":"diesel","kilometrage":"{kilometrage}","statut":"en_service"}}"#
        )
    };
    let chemin = format!("/api/v1/vehicules/{vehicule}");
    let reponse = requete_json(&app, "PUT", &chemin, Some(&cookie), Some(&corps("12000"))).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let reponse = requete_json(&app, "PUT", &chemin, Some(&cookie), Some(&corps("15000"))).await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert!(texte(reponse).await.contains("relevez le compteur"));
    assert_eq!(compteur(&pool, vehicule).await, 12_000);
}

#[sqlx::test]
async fn l_api_lit_et_corrige_les_releves(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CP-700-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = requete_json(
        &app,
        "POST",
        &format!("/api/v1/vehicules/{vehicule}/releves"),
        Some(&cookie),
        Some(r#"{"kilometrage":"20000"}"#),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let cree: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let id = cree["id"].as_i64().unwrap();
    assert_eq!(compteur(&pool, vehicule).await, 20_000);

    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/releves"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let historique: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(historique.as_array().unwrap().len(), 2);
    assert_eq!(historique[0]["origine"], "releve");
    assert_eq!(historique[0]["kilometrage"], 20000);

    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/releves/{id}"),
        Some(&cookie),
        Some(&format!(
            r#"{{"date_releve":"{}","kilometrage":"18000"}}"#,
            aujourd_hui()
        )),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert_eq!(compteur(&pool, vehicule).await, 18_000);

    let reponse = requete_json(
        &app,
        "DELETE",
        &format!("/api/v1/releves/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);
    assert_eq!(compteur(&pool, vehicule).await, 12_000);

    let reponse = requete_json(
        &app,
        "GET",
        "/api/v1/vehicules/999999/releves",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}

/// La reprise de la migration `20261024090000` : un compteur plus haut que les interventions et
/// les pleins devient un relevé ; les autres non. La requête est rejouée telle qu'elle est écrite
/// dans la migration, sur des véhicules mis dans l'état d'avant.
#[sqlx::test]
async fn la_migration_reprend_les_compteurs_saisis_a_la_main(pool: PgPool) {
    let saisi_a_la_main = creer_vehicule(&pool, "CP-800-AN").await;
    let deduit = creer_vehicule(&pool, "CP-801-AN").await;
    let sans_historique = creer_vehicule(&pool, "CP-802-AN").await;
    let a_zero = creer_vehicule(&pool, "CP-803-AN").await;
    interventions::creer(&pool, &intervention(saisi_a_la_main, "40000"))
        .await
        .unwrap();
    depenses::creer(&pool, &plein(deduit, "40000"))
        .await
        .unwrap();
    // L'état d'avant : pas de relevé, des compteurs saisis à la main.
    sqlx::query("DELETE FROM releves_kilometrage")
        .execute(&pool)
        .await
        .unwrap();
    for (vehicule, kilometrage) in [
        (saisi_a_la_main, 50_000),
        (deduit, 40_000),
        (sans_historique, 30_000),
        (a_zero, 0),
    ] {
        sqlx::query("UPDATE vehicules SET kilometrage = $2 WHERE id = $1")
            .bind(vehicule)
            .bind(kilometrage)
            .execute(&pool)
            .await
            .unwrap();
    }

    let migration = include_str!("../migrations/20261024090000_releves_kilometrage.sql");
    let reprise = &migration[migration.find("INSERT INTO").unwrap()..];
    sqlx::raw_sql(reprise).execute(&pool).await.unwrap();

    let reprises: Vec<(i64, i32)> = sqlx::query_as(
        "SELECT vehicule_id, kilometrage FROM releves_kilometrage ORDER BY vehicule_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        reprises,
        vec![(saisi_a_la_main, 50_000), (sans_historique, 30_000)]
    );
    // Rien d'autre n'a bougé.
    assert_eq!(compteur(&pool, deduit).await, 40_000);
    assert_eq!(compteur(&pool, a_zero).await, 0);
}
