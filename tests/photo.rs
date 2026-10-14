//! La photo du véhicule, sur base réelle : dépôt, vignette calculée par le serveur, affichage sur
//! la fiche et sur l'accueil Autoclair, suppression. Images fabriquées par le test.

mod commun;

use std::io::Cursor;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::config::Config;
use gestion_parc_auto::etat::Etat;
use image::{ImageFormat, Rgb, RgbImage};
use sqlx::PgPool;

use commun::{
    creer_utilisateur, creer_vehicule, get, post_formulaire, post_multipart, se_connecter, texte,
};

/// Une image fabriquée, aux dimensions voulues.
fn png(largeur: u32, hauteur: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(largeur, hauteur, |x, y| Rgb([x as u8, y as u8, 120]));
    let mut tampon = Cursor::new(Vec::new());
    image.write_to(&mut tampon, ImageFormat::Png).unwrap();
    tampon.into_inner()
}

async fn id_du_document(pool: &PgPool, nom: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM documents WHERE nom_original = $1 ORDER BY id DESC")
        .bind(nom)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Les deux applications (Autoclair Pro, Autoclair) sur la même base et le même dossier.
fn applications(pool: &PgPool) -> (axum::Router, axum::Router, std::path::PathBuf) {
    let dossier = commun::dossier_documents_de_test();
    let config = |profil| Config {
        profil,
        documents_dir: dossier.clone(),
        ..commun::config_de_test()
    };
    let pro = gestion_parc_auto::web::routeur(
        Etat::nouveau(pool.clone(), config(Profil::Entreprise)).expect("état"),
    );
    let part = gestion_parc_auto::web::routeur(
        Etat::nouveau(pool.clone(), config(Profil::Particulier)).expect("état"),
    );
    (pro, part, dossier)
}

fn fichiers_vignette(dossier: &std::path::Path) -> usize {
    std::fs::read_dir(dossier)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().ends_with(".vignette.jpg"))
                .count()
        })
        .unwrap_or(0)
}

#[sqlx::test]
async fn la_photo_du_vehicule_a_sa_vignette_sur_la_fiche_et_l_accueil(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let kangoo = creer_vehicule(&pool, "PH-100-OT").await;
    let sans_photo = creer_vehicule(&pool, "PH-200-OT").await;
    let (pro, part, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;
    let depot = format!("/vehicules/{kangoo}/documents");

    // Sans photo : la silhouette, et le lien pour en prendre une, qui présélectionne le type.
    let fiche = texte(get(&pro, &format!("/vehicules/{kangoo}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(r#"<span class="silhouette" aria-hidden="true"></span>"#),
        "{fiche}"
    );
    let lien =
        format!(r#"href="/documents/nouveau?vehicule_id={kangoo}&type_document=photo_vehicule""#);
    assert!(fiche.contains(&lien), "{fiche}");
    let page = texte(
        get(
            &pro,
            &format!("/documents/nouveau?vehicule_id={kangoo}&type_document=photo_vehicule"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"<option value="photo_vehicule" selected>Photo du véhicule</option>"#),
        "{page}"
    );

    // Dépôt d'une photo de 1 600 × 900 : acceptée, vignette de 640 × 360.
    let photo = png(1600, 900);
    let reponse = post_multipart(
        &pro,
        &cookie,
        &depot,
        &[("type_document", "photo_vehicule")],
        &[("voiture.png", "image/png", &photo)],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let id = id_du_document(&pool, "voiture.png").await;
    assert_eq!(fichiers_vignette(&dossier), 1);

    let reponse = get(&pro, &format!("/documents/{id}/vignette"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert_eq!(reponse.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        reponse.headers()[header::CACHE_CONTROL],
        "private, max-age=86400"
    );
    let octets = reponse.into_body();
    let octets = http_body_util::BodyExt::collect(octets)
        .await
        .unwrap()
        .to_bytes();
    let vignette = image::load_from_memory_with_format(&octets, ImageFormat::Jpeg).unwrap();
    assert_eq!((vignette.width(), vignette.height()), (640, 360));

    // La fiche la montre, et l'original reste dans la liste des documents.
    let fiche = texte(get(&pro, &format!("/vehicules/{kangoo}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(r#"<a href="/documents/{id}"><img src="/documents/{id}/vignette" alt="Photo de PH-100-OT (Renault Kangoo)"></a>"#)),
        "{fiche}"
    );
    assert!(fiche.contains("<td>Photo du véhicule</td>"), "{fiche}");
    assert!(!fiche.contains("Ajouter une photo"), "{fiche}");

    // L'accueil Autoclair : la vignette sur la carte de ce véhicule, la silhouette sur l'autre.
    let cookie_part = se_connecter(&part, "gestion").await;
    let accueil = texte(get(&part, "/", Some(&cookie_part)).await).await;
    assert!(
        accueil.contains(&format!(r#"<a class="carte-photo" href="/vehicules/{kangoo}" tabindex="-1" aria-hidden="true"><img src="/documents/{id}/vignette" alt=""></a>"#)),
        "{accueil}"
    );
    assert!(
        accueil.contains(&format!(r#"<a class="carte-photo" href="/vehicules/{sans_photo}" tabindex="-1" aria-hidden="true"><span class="silhouette"></span></a>"#)),
        "{accueil}"
    );

    // Une seconde photo : c'est la plus récente qui sert.
    let reponse = post_multipart(
        &pro,
        &cookie,
        &depot,
        &[("type_document", "photo_vehicule")],
        &[("voiture-2.png", "image/png", &png(800, 800))],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let id2 = id_du_document(&pool, "voiture-2.png").await;
    let fiche = texte(get(&pro, &format!("/vehicules/{kangoo}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(r#"<img src="/documents/{id2}/vignette""#)),
        "{fiche}"
    );
    assert!(
        !fiche.contains(&format!(r#"<img src="/documents/{id}/vignette""#)),
        "{fiche}"
    );
    assert_eq!(fichiers_vignette(&dossier), 2);

    // Supprimer la photo récente efface sa vignette ; la précédente reprend.
    let reponse = post_formulaire(
        &pro,
        &format!("/documents/{id2}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(fichiers_vignette(&dossier), 1);
    let reponse = get(&pro, &format!("/documents/{id2}/vignette"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
    let fiche = texte(get(&pro, &format!("/vehicules/{kangoo}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(r#"<img src="/documents/{id}/vignette""#)),
        "{fiche}"
    );

    // Sans session, pas de vignette.
    let reponse = get(&pro, &format!("/documents/{id}/vignette"), None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn une_photo_illisible_est_refusee_et_un_autre_document_n_a_pas_de_vignette(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PH-300-OT").await;
    let (pro, _, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;
    let depot = format!("/vehicules/{vehicule}/documents");
    let photo = [("type_document", "photo_vehicule")];

    // Un PDF déclaré « photo du véhicule » : refusé, la fiche revient avec le motif.
    let reponse = post_multipart(
        &pro,
        &cookie,
        &depot,
        &photo,
        &[("doc.pdf", "application/pdf", b"%PDF-1.4")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(
            "Document non déposé. Une photo du véhicule est une image (JPEG, PNG ou WebP)."
        ),
        "{page}"
    );

    // Une « image » que le serveur ne sait pas lire : refusée aussi, rien sur le disque.
    let reponse = post_multipart(
        &pro,
        &cookie,
        &depot,
        &photo,
        &[("cassee.jpg", "image/jpeg", b"pas une image")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    // Le gabarit échappe l'apostrophe du message.
    assert!(
        page.contains("Document non déposé. La photo n&#39;a pas pu être lue."),
        "{page}"
    );
    assert!(page.contains("Aucun document."), "{page}");
    assert_eq!(
        std::fs::read_dir(&dossier).map(|d| d.count()).unwrap_or(0),
        0
    );

    // Une image déposée sous un autre type (carte grise) n'a pas de vignette.
    let reponse = post_multipart(
        &pro,
        &cookie,
        &depot,
        &[("type_document", "carte_grise")],
        &[("cg.png", "image/png", &png(400, 300))],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let id = id_du_document(&pool, "cg.png").await;
    assert_eq!(fichiers_vignette(&dossier), 0);
    let reponse = get(&pro, &format!("/documents/{id}/vignette"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
    let fiche = texte(get(&pro, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(r#"<span class="silhouette" aria-hidden="true"></span>"#),
        "{fiche}"
    );

    let _ = std::fs::remove_dir_all(dossier);
}
