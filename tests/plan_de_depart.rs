//! Le plan de départ (#94, bloc 5) : sur la fiche d'un véhicule sans plan, « Proposer un plan »
//! ouvre les opérations courantes pour son carburant, cochées et pré-remplies ; les cases cochées
//! deviennent les lignes du plan, tout ou rien.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::metier::plan_entretien::{self, SaisiePlan};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, saisie_vehicule,
    se_connecter, texte,
};

/// Askama écrit `&` en `&#38;` ou `&amp;` selon l'endroit : les pages se comparent décodées.
fn decoder(page: &str) -> String {
    page.replace("&#38;", "&").replace("&amp;", "&")
}

async fn vehicule_au(pool: &PgPool, immatriculation: &str, carburant: &str) -> i64 {
    let mut saisie = saisie_vehicule(immatriculation);
    saisie.carburant = carburant.into();
    vehicules::creer(pool, &saisie).await.unwrap().id
}

/// Le plan du véhicule : (opération, tous les … km, tous les … mois), dans l'ordre de la fiche.
async fn plan(pool: &PgPool, vehicule: i64) -> Vec<(String, Option<i32>, Option<i32>)> {
    plan_entretien::lister_par_vehicule(pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .map(|l| (l.operation, l.periodicite_km, l.periodicite_mois))
        .collect()
}

async fn page(app: &axum::Router, vehicule: i64, cookie: &str) -> String {
    decoder(
        &texte(
            get(
                app,
                &format!("/plan-entretien/depart?vehicule_id={vehicule}"),
                Some(cookie),
            )
            .await,
        )
        .await,
    )
}

/// La case de l'opération, cochée ou non.
fn case(code: &str, cochee: bool) -> String {
    format!(
        r#"<input type="checkbox" name="operation" value="{code}"{}>"#,
        if cochee { " checked" } else { "" }
    )
}

#[sqlx::test]
async fn la_fiche_propose_un_plan_tant_que_le_vehicule_n_en_a_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "PD-100-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;
    let lien =
        format!(r#"href="/plan-entretien/depart?vehicule_id={vehicule}">Proposer un plan</a>"#);

    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(&lien), "{fiche}");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&lecteur)).await).await;
    assert!(!fiche.contains("Proposer un plan"));
    // Le lecteur ne l'ouvre pas, ni ne l'envoie.
    assert_eq!(
        get(
            &app,
            &format!("/plan-entretien/depart?vehicule_id={vehicule}"),
            Some(&lecteur)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_formulaire(
            &app,
            "/plan-entretien/depart",
            Some(&lecteur),
            &format!("vehicule_id={vehicule}&operation=niveaux&km_niveaux=&mois_niveaux=1")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    // Une opération suivie : le bouton s'efface, « Suivre une opération » reste.
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "niveaux".into(),
            periodicite_mois: "1".into(),
            ..Default::default()
        },
        commun::aujourd_hui(),
    )
    .await
    .unwrap();
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("Proposer un plan"));
    assert!(fiche.contains("Suivre une opération"));
}

#[sqlx::test]
async fn la_page_propose_les_operations_du_carburant(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let diesel = creer_vehicule(&pool, "PD-200-AN").await;
    let essence = vehicule_au(&pool, "PD-201-AN", "essence").await;
    let electrique = vehicule_au(&pool, "PD-202-AN", "electrique").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let p = page(&app, diesel, &cookie).await;
    assert!(p.contains("Plan de départ · PD-200-AN"), "{p}");
    assert!(p.contains(&case("vidange", true)), "{p}");
    assert!(p.contains(r#"name="km_vidange" value="15000""#), "{p}");
    assert!(p.contains(r#"name="mois_vidange" value="12""#), "{p}");
    assert!(p.contains(&case("filtre_carburant", true)), "{p}");
    assert!(!p.contains(r#"value="bougies""#), "{p}");
    // La courroie dépend du moteur : proposée, pas cochée, avec la raison.
    assert!(p.contains(&case("courroie_distribution", false)), "{p}");
    assert!(p.contains("certains ont une chaîne"), "{p}");
    assert!(p.contains("Le carnet d'entretien du véhicule prime"), "{p}");

    let p = page(&app, essence, &cookie).await;
    assert!(p.contains(&case("bougies", true)), "{p}");
    assert!(!p.contains(r#"value="filtre_carburant""#), "{p}");

    let p = page(&app, electrique, &cookie).await;
    assert!(!p.contains(r#"value="vidange""#), "{p}");
    assert!(!p.contains(r#"value="bougies""#), "{p}");
    assert!(p.contains(&case("liquide_frein", true)), "{p}");
}

#[sqlx::test]
async fn les_cases_cochees_deviennent_le_plan(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PD-300-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La vidange ajustée à 20 000 km, la pression des pneus telle quelle ; la courroie, décochée,
    // envoie ses champs, que le serveur ne lit pas.
    let reponse = post_formulaire(
        &app,
        "/plan-entretien/depart",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&operation=vidange&km_vidange=20+000&mois_vidange=12&km_courroie_distribution=120000&mois_courroie_distribution=72&operation=pression_pneus&km_pression_pneus=&mois_pression_pneus=1"
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}").as_str()
    );
    let fait = reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .find(|c| c.starts_with("fait="))
        .unwrap_or_default();
    assert!(fait.starts_with("fait=plan-de-depart;"), "{fait}");
    assert_eq!(
        plan(&pool, vehicule).await,
        vec![
            ("vidange".into(), Some(20_000), Some(12)),
            ("pression_pneus".into(), None, Some(1)),
        ]
    );

    // Les opérations déjà suivies ne sont plus proposées.
    let p = page(&app, vehicule, &cookie).await;
    assert!(!p.contains(r#"value="vidange""#), "{p}");
    assert!(p.contains(&case("filtre_huile", true)), "{p}");
}

#[sqlx::test]
async fn un_refus_nomme_l_operation_garde_la_saisie_et_n_enregistre_rien(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PD-400-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Rien de coché.
    let reponse = post_formulaire(
        &app,
        "/plan-entretien/depart",
        Some(&cookie),
        &format!("vehicule_id={vehicule}&km_vidange=15000&mois_vidange=12"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let p = texte(reponse).await;
    assert!(p.contains("Cocher au moins une opération à suivre."), "{p}");
    assert!(p.contains(&case("vidange", false)), "{p}");

    // Une périodicité fausse : le refus nomme l'opération, la saisie revient telle quelle, et
    // l'opération valable envoyée avec elle n'est pas enregistrée.
    let reponse = post_formulaire(
        &app,
        "/plan-entretien/depart",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&operation=niveaux&km_niveaux=&mois_niveaux=1&operation=vidange&km_vidange=abc&mois_vidange=12"
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let p = decoder(&texte(reponse).await);
    assert!(p.contains("Vidange : Le champ"), "{p}");
    assert!(p.contains(r#"name="km_vidange" value="abc""#), "{p}");
    assert!(p.contains(&case("vidange", true)), "{p}");
    assert!(p.contains(&case("filtre_huile", false)), "{p}");
    assert!(plan(&pool, vehicule).await.is_empty());

    // Une opération déjà suivie entre-temps : refusée, et rien d'autre n'est enregistré.
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "15000".into(),
            ..Default::default()
        },
        commun::aujourd_hui(),
    )
    .await
    .unwrap();
    let reponse = post_formulaire(
        &app,
        "/plan-entretien/depart",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&operation=niveaux&km_niveaux=&mois_niveaux=1&operation=vidange&km_vidange=15000&mois_vidange=12"
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let p = texte(reponse).await;
    assert!(p.contains("est déjà suivie pour ce véhicule"), "{p}");
    assert_eq!(
        plan(&pool, vehicule).await,
        vec![("vidange".into(), Some(15_000), None)]
    );
}
