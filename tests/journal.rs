//! Le journal des écritures (#60) : une ligne par écriture réussie, aucune pour une lecture ni
//! pour une écriture refusée, jamais de donnée nominative ; la suppression en cascade dit ce
//! qu'elle emporte.
//!
//! Le journal est capturé en mémoire par `commun::capturer_journal` (un seul abonné pour tout le
//! binaire de test) : chaque test y cherche ses propres lignes par un identifiant de connexion
//! qui n'est qu'à lui.

mod commun;

use axum::http::StatusCode;
use sqlx::PgPool;

use commun::{
    application, capturer_journal, creer_conducteur, creer_utilisateur, creer_vehicule, get,
    journal, post_formulaire, requete_json, se_connecter,
};

fn lignes_de(login: &str) -> Vec<String> {
    journal()
        .lines()
        .filter(|l| l.contains(&format!("login={login}")))
        .map(str::to_string)
        .collect()
}

#[sqlx::test]
async fn une_ecriture_reussie_laisse_une_ligne_sans_donnee_nominative(pool: PgPool) {
    capturer_journal();
    creer_utilisateur(&pool, "journal-creation", "administrateur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "journal-creation").await;

    let reponse = post_formulaire(
        &app,
        "/vehicules",
        Some(&cookie),
        "immatriculation=JL-100-NA&marque=Renault&modele=Kangoo&date_mise_en_service=2023-06-01&carburant=diesel&kilometrage=12000&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let lignes = lignes_de("journal-creation");
    let ecriture = lignes
        .iter()
        .find(|l| l.contains("écriture") && l.contains("chemin=/vehicules "))
        .unwrap_or_else(|| panic!("ligne d'écriture attendue dans {lignes:#?}"));
    assert!(ecriture.contains("methode=POST"), "{ecriture}");
    assert!(ecriture.contains("statut=303"), "{ecriture}");
    assert!(
        !journal().contains("JL-100-NA"),
        "l'immatriculation ne sort jamais"
    );
}

#[sqlx::test]
async fn une_lecture_ou_une_ecriture_refusee_ne_laisse_rien(pool: PgPool) {
    capturer_journal();
    creer_utilisateur(&pool, "journal-refus", "administrateur").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "journal-refus").await;

    let reponse = get(&app, "/vehicules", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    // Immatriculation vide : le formulaire revient avec son message, rien n'est écrit.
    let reponse = post_formulaire(
        &app,
        "/vehicules",
        Some(&cookie),
        "immatriculation=&marque=Renault&modele=Kangoo&date_mise_en_service=2023-06-01&carburant=diesel&kilometrage=12000&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);

    let ecritures: Vec<String> = lignes_de("journal-refus")
        .into_iter()
        .filter(|l| l.contains("écriture"))
        .collect();
    assert!(ecritures.is_empty(), "{ecritures:#?}");
    // La connexion, elle, a sa ligne (web/connexion.rs), comme avant.
    assert!(lignes_de("journal-refus")
        .iter()
        .any(|l| l.contains("connexion")));
}

#[sqlx::test]
async fn l_api_laisse_la_meme_ligne(pool: PgPool) {
    capturer_journal();
    creer_utilisateur(&pool, "journal-api", "administrateur").await;
    let id = creer_vehicule(&pool, "JL-200-NA").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "journal-api").await;

    let reponse = requete_json(
        &app,
        "DELETE",
        &format!("/api/v1/vehicules/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert!(reponse.status().is_success(), "{}", reponse.status());

    let lignes = lignes_de("journal-api");
    let ecriture = lignes
        .iter()
        .find(|l| l.contains("écriture"))
        .unwrap_or_else(|| panic!("ligne attendue dans {lignes:#?}"));
    assert!(ecriture.contains("methode=DELETE"), "{ecriture}");
    assert!(
        ecriture.contains(&format!("chemin=/api/v1/vehicules/{id}")),
        "{ecriture}"
    );
}

#[sqlx::test]
async fn la_suppression_en_cascade_dit_ce_qu_elle_emporte(pool: PgPool) {
    capturer_journal();
    creer_utilisateur(&pool, "journal-cascade", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "JL-300-NA").await;
    let conducteur = creer_conducteur(&pool, "Cascade").await;
    let app = application(pool);
    let cookie = se_connecter(&app, "journal-cascade").await;
    let reponse = post_formulaire(
        &app,
        "/affectations",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&conducteur_id={conducteur}&date_debut=2026-01-10&date_fin="
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{vehicule}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let cascade = journal()
        .lines()
        .find(|l| l.contains("cascade") && l.contains(&format!("vehicule_id={vehicule}")))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("ligne de cascade attendue dans\n{}", journal()));
    assert!(cascade.contains("affectations=1"), "{cascade}");
    assert!(cascade.contains("interventions=0"), "{cascade}");
    assert!(cascade.contains("documents=0"), "{cascade}");
    assert!(!cascade.contains("JL-300-NA"), "{cascade}");
}
