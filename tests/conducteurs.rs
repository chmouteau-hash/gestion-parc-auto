//! L'avertissement d'homonyme à la création d'un conducteur, prouvé sur une vraie base :
//!
//! - un conducteur de même nom et de même prénom (la casse et les accents ne comptent pas, le
//!   service non plus) est signalé, pas refusé : le formulaire revient avec l'avertissement et un
//!   bouton « Créer quand même » ;
//! - les conducteurs inactifs comptent, avec l'invitation à réactiver la fiche existante ;
//! - seule la création avertit : la modification et l'API JSON ne changent pas ;
//! - une saisie invalide montre son erreur, pas l'avertissement.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::conducteurs::{self, SaisieConducteur};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, post_formulaire, requete_json,
    saisie_conducteur, se_connecter, texte,
};

/// Un conducteur de test au nom, au prénom et au statut donnés (service « Logistique »).
async fn existant(pool: &PgPool, nom: &str, prenom: &str, statut: &str) -> i64 {
    conducteurs::creer(
        pool,
        &SaisieConducteur {
            prenom: prenom.to_string(),
            statut: statut.to_string(),
            ..saisie_conducteur(nom)
        },
    )
    .await
    .expect("création du conducteur")
    .id
}

async fn nombre(pool: &PgPool) -> usize {
    conducteurs::lister(pool).await.unwrap().len()
}

/// Le corps d'un formulaire « Nouveau conducteur », déjà encodé pour l'adresse.
fn corps(nom: &str, prenom: &str) -> String {
    format!(
        "nom={nom}&prenom={prenom}&service=Atelier&date_expiration_permis=2031-01-01&statut=actif"
    )
}

#[sqlx::test]
async fn un_homonyme_est_signale_sans_etre_enregistre(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let martin = existant(&pool, "Martin", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La casse ne compte pas : « MARTIN camille » est « Camille Martin ».
    let reponse = post_formulaire(
        &app,
        "/conducteurs",
        Some(&cookie),
        &corps("MARTIN", "camille"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;

    assert!(
        page.contains(r#"<div class="message-attention" role="alert">"#),
        "{page}"
    );
    // Le lien vers la fiche existante, avec son service : de quoi juger si c'est la même personne.
    assert!(
        page.contains(&format!(
            r#"<a href="/conducteurs/{martin}">Camille Martin</a>, Logistique"#
        )),
        "{page}"
    );
    // L'onglet le dit aussi, et le formulaire garde ce qui a été tapé.
    assert!(page.contains("Attention : Nouveau conducteur"), "{page}");
    assert!(page.contains(r#"name="nom" value="MARTIN""#), "{page}");
    assert!(
        page.contains(r#"formaction="/conducteurs?creer_quand_meme=1">Créer quand même</button>"#),
        "{page}"
    );
    assert_eq!(nombre(&pool).await, 1, "rien n'a été enregistré");
}

#[sqlx::test]
async fn les_accents_et_les_blancs_ne_comptent_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Durand", "Léa", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // « LEA » sans accent, « durand » en minuscules, des blancs autour.
    let reponse = post_formulaire(
        &app,
        "/conducteurs",
        Some(&cookie),
        &corps("+durand+", "LEA"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("message-attention"), "{page}");
    assert_eq!(nombre(&pool).await, 1);
}

#[sqlx::test]
async fn creer_quand_meme_enregistre_la_seconde_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // C'est l'adresse que porte le bouton « Créer quand même » (formaction).
    let reponse = post_formulaire(
        &app,
        "/conducteurs?creer_quand_meme=1",
        Some(&cookie),
        &corps("Martin", "Camille"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let lieu = reponse.headers()[header::LOCATION].to_str().unwrap();
    assert!(lieu.starts_with("/conducteurs/"), "{lieu}");
    assert_eq!(nombre(&pool).await, 2, "la seconde fiche existe");
}

#[sqlx::test]
async fn un_homonyme_inactif_invite_a_reactiver_sa_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let parti = existant(&pool, "Moreau", "Camille", "inactif").await;
    let actif = existant(&pool, "Moreau", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/conducteurs",
        Some(&cookie),
        &corps("Moreau", "Camille"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;

    // Les deux sont listés, et le pluriel suit.
    assert!(page.contains("Des conducteurs portent déjà"), "{page}");
    for id in [parti, actif] {
        assert!(
            page.contains(&format!(r#"href="/conducteurs/{id}""#)),
            "{page}"
        );
    }
    // Seul l'inactif invite à la réactivation, vers son formulaire de modification.
    assert!(page.contains("Inactif"), "{page}");
    assert!(
        page.contains(&format!(
            r#"<a href="/conducteurs/{parti}/modifier">réactivez cette fiche</a>"#
        )),
        "{page}"
    );
    assert!(
        !page.contains(&format!(r#"/conducteurs/{actif}/modifier"#)),
        "l'actif n'a pas à être réactivé\n{page}"
    );
}

#[sqlx::test]
async fn un_nom_ou_un_prenom_different_n_avertit_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    for (nom, prenom) in [("Martin", "Paul"), ("Dupont", "Camille")] {
        let reponse =
            post_formulaire(&app, "/conducteurs", Some(&cookie), &corps(nom, prenom)).await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{nom} {prenom}");
    }
    assert_eq!(nombre(&pool).await, 3);
}

#[sqlx::test]
async fn une_saisie_invalide_montre_son_erreur_pas_l_avertissement(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Homonyme, mais sans date de permis : l'erreur de saisie passe d'abord.
    let reponse = post_formulaire(
        &app,
        "/conducteurs",
        Some(&cookie),
        "nom=Martin&prenom=Camille&service=&date_expiration_permis=&statut=actif",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("message-erreur"), "{page}");
    assert!(!page.contains("message-attention"), "{page}");
}

#[sqlx::test]
async fn la_modification_n_avertit_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let durand = existant(&pool, "Durand", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Renommer Durand en Martin : un homonyme, mais seule la création avertit.
    let reponse = post_formulaire(
        &app,
        &format!("/conducteurs/{durand}"),
        Some(&cookie),
        &corps("Martin", "Camille"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
}

#[sqlx::test]
async fn l_api_cree_sans_avertir(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let json = r#"{"nom":"Martin","prenom":"Camille","service":"Atelier","date_expiration_permis":"2031-01-01","statut":"actif"}"#;
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/conducteurs",
        Some(&cookie),
        Some(json),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    assert_eq!(nombre(&pool).await, 2);
}

#[sqlx::test]
async fn le_profil_particulier_n_ecrit_pas_le_service_dans_l_avertissement(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    existant(&pool, "Martin", "Camille", "actif").await;
    let app = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&app, "gestion").await;

    let reponse = post_formulaire(
        &app,
        "/conducteurs",
        Some(&cookie),
        &corps("Martin", "Camille"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("message-attention"), "{page}");
    assert!(page.contains("Camille Martin</a>"), "{page}");
    // Le service existe en base, mais ce profil ne le montre nulle part.
    assert!(!page.contains("Logistique"), "{page}");
}
