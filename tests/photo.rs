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
    creer_utilisateur, creer_vehicule, get, post_formulaire, post_multipart, post_multipart_nomme,
    se_connecter, texte,
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
    // L'en-tête montre la récente ; la précédente n'est plus que dans le bloc « Photos » (#66).
    let en_tete = |id: i64| {
        format!(
            r#"<a href="/documents/{id}"><img src="/documents/{id}/vignette" alt="Photo de PH-100-OT (Renault Kangoo)">"#
        )
    };
    assert!(fiche.contains(&en_tete(id2)), "{fiche}");
    assert!(!fiche.contains(&en_tete(id)), "{fiche}");
    assert!(
        fiche.contains(&format!(
            r#"<img src="/documents/{id}/vignette" alt="Photo déposée le"#
        )),
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

/// Les champs du formulaire « Nouveau véhicule », hors photo. Immatriculation fictive.
fn saisie_creation(immatriculation: &str) -> Vec<(&'static str, String)> {
    vec![
        ("immatriculation", immatriculation.to_string()),
        ("vin", String::new()),
        ("marque", "Renault".to_string()),
        ("modele", "Kangoo".to_string()),
        ("date_mise_en_service", "2023-05-02".to_string()),
        ("carburant", "diesel".to_string()),
        ("kilometrage", "12000".to_string()),
        ("statut", "en_service".to_string()),
    ]
}

async fn nombre_de_vehicules(pool: &PgPool, immatriculation: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM vehicules WHERE immatriculation = $1")
        .bind(immatriculation)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn la_photo_se_choisit_des_la_creation_du_vehicule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let (pro, _, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;

    // Le formulaire de création a le champ, en envoi `multipart` ; celui de modification non.
    let page = texte(get(&pro, "/vehicules/nouveau", Some(&cookie)).await).await;
    assert!(page.contains(r#"enctype="multipart/form-data""#), "{page}");
    assert!(
        page.contains(r#"<input type="file" name="photo" accept="image/*""#),
        "{page}"
    );
    let existant = creer_vehicule(&pool, "PH-400-OT").await;
    let page = texte(
        get(
            &pro,
            &format!("/vehicules/{existant}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(!page.contains("enctype="), "{page}");
    assert!(!page.contains(r#"name="photo""#), "{page}");

    // Création avec une photo : le véhicule et son document, la fiche montre la vignette.
    let saisie = saisie_creation("PH-500-OT");
    let champs: Vec<(&str, &str)> = saisie.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let reponse = post_multipart_nomme(
        &pro,
        &cookie,
        "/vehicules",
        "photo",
        &champs,
        &[("kangoo.png", "image/png", &png(1200, 800))],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let fiche_url = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(fiche_url.starts_with("/vehicules/"), "{fiche_url}");
    let cookies: Vec<String> = reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert!(
        cookies
            .iter()
            .any(|c| c.starts_with("fait=vehicule-cree-photo;")),
        "{cookies:?}"
    );
    let id = id_du_document(&pool, "kangoo.png").await;
    assert_eq!(fichiers_vignette(&dossier), 1);
    let fiche = texte(
        get(
            &pro,
            &fiche_url,
            Some(&format!("{cookie}; fait=vehicule-cree-photo")),
        )
        .await,
    )
    .await;
    assert!(
        fiche.contains("Véhicule enregistré, avec sa photo."),
        "{fiche}"
    );
    assert!(
        fiche.contains(&format!(
            r#"<img src="/documents/{id}/vignette" alt="Photo de PH-500-OT (Renault Kangoo)">"#
        )),
        "{fiche}"
    );
    assert!(fiche.contains("<td>Photo du véhicule</td>"), "{fiche}");

    // Sans photo (le navigateur envoie le champ vide) : comme avant, la silhouette.
    let saisie = saisie_creation("PH-600-OT");
    let champs: Vec<(&str, &str)> = saisie.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let reponse = post_multipart_nomme(
        &pro,
        &cookie,
        "/vehicules",
        "photo",
        &champs,
        &[("", "application/octet-stream", b"")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let cookies: Vec<String> = reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    assert!(
        cookies.iter().any(|c| c.starts_with("fait=vehicule-cree;")),
        "{cookies:?}"
    );
    let fiche_url = reponse.headers()[header::LOCATION].to_str().unwrap();
    let fiche = texte(get(&pro, fiche_url, Some(&cookie)).await).await;
    assert!(
        fiche.contains(r#"<span class="silhouette" aria-hidden="true"></span>"#),
        "{fiche}"
    );
    assert_eq!(fichiers_vignette(&dossier), 1);

    // Une photo illisible : le véhicule n'est pas créé, le formulaire revient avec le motif, la
    // saisie, et dit que la photo est à choisir de nouveau. Rien de plus sur le disque.
    let saisie = saisie_creation("PH-700-OT");
    let champs: Vec<(&str, &str)> = saisie.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let reponse = post_multipart_nomme(
        &pro,
        &cookie,
        "/vehicules",
        "photo",
        &champs,
        &[("cassee.jpg", "image/jpeg", b"pas une image")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("La photo n&#39;a pas pu être lue."), "{page}");
    assert!(page.contains(r#"value="PH-700-OT""#), "{page}");
    // Texte fixe du gabarit : l'apostrophe n'y est pas échappée, contrairement au message.
    assert!(
        page.contains("La photo choisie n'a pas été conservée : choisissez-la de nouveau."),
        "{page}"
    );
    assert_eq!(nombre_de_vehicules(&pool, "PH-700-OT").await, 0);
    assert_eq!(
        std::fs::read_dir(&dossier).map(|d| d.count()).unwrap_or(0),
        2,
        "la photo de PH-500-OT et sa vignette, rien d'autre"
    );

    // Une saisie refusée (immatriculation déjà prise) avec une bonne photo : rien n'est écrit
    // non plus, et la photo est à choisir de nouveau.
    let saisie = saisie_creation("PH-500-OT");
    let champs: Vec<(&str, &str)> = saisie.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let reponse = post_multipart_nomme(
        &pro,
        &cookie,
        "/vehicules",
        "photo",
        &champs,
        &[("doublon.png", "image/png", &png(300, 200))],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("est déjà utilisée par un autre véhicule."),
        "{page}"
    );
    assert!(page.contains("choisissez-la de nouveau."), "{page}");
    assert_eq!(nombre_de_vehicules(&pool, "PH-500-OT").await, 1);
    assert_eq!(
        std::fs::read_dir(&dossier).map(|d| d.count()).unwrap_or(0),
        2
    );

    // L'encodage classique (sans photo) reste accepté : c'est celui des autres tests.
    let reponse = post_formulaire(
        &pro,
        "/vehicules",
        Some(&cookie),
        "immatriculation=PH-800-OT&marque=Renault&modele=Kangoo&date_mise_en_service=2023-05-02&carburant=diesel&kilometrage=10&statut=en_service",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(nombre_de_vehicules(&pool, "PH-800-OT").await, 1);

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn un_pdf_comme_photo_a_la_creation_a_son_propre_motif(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let (pro, _, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;

    // Le formulaire de création n'a pas de choix de type : le motif ne parle pas de « choisir
    // un autre type », il renvoie à la fiche.
    let saisie = saisie_creation("PH-900-OT");
    let champs: Vec<(&str, &str)> = saisie.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let reponse = post_multipart_nomme(
        &pro,
        &cookie,
        "/vehicules",
        "photo",
        &champs,
        &[("doc.pdf", "application/pdf", b"%PDF-1.4")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(
            "Un autre fichier se dépose depuis la fiche, une fois le véhicule enregistré."
        ),
        "{page}"
    );
    assert!(!page.contains("choisissez un autre type"), "{page}");
    assert_eq!(nombre_de_vehicules(&pool, "PH-900-OT").await, 0);

    let _ = std::fs::remove_dir_all(dossier);
}

/// Dépose une image sur la fiche d'un véhicule, sous le type voulu, et rend l'identifiant du
/// document.
async fn deposer_image(
    app: &axum::Router,
    pool: &PgPool,
    cookie: &str,
    vehicule: i64,
    nom: &str,
    type_document: &str,
) -> i64 {
    let reponse = post_multipart(
        app,
        cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[("type_document", type_document)],
        &[(nom, "image/png", &png(640, 480))],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    id_du_document(pool, nom).await
}

/// Le code du message posé en cookie après un geste.
fn fait_pose(reponse: &axum::response::Response) -> Option<String> {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.strip_prefix("fait=")
                .map(|reste| reste.split(';').next().unwrap_or("").to_string())
        })
}

/// La vignette de l'en-tête d'une fiche : le lien vers l'original, puis la vignette.
fn vignette_d_en_tete(id: i64, immatriculation: &str) -> String {
    format!(
        r#"<a href="/documents/{id}"><img src="/documents/{id}/vignette" alt="Photo de {immatriculation}"#
    )
}

#[sqlx::test]
async fn la_photo_de_la_fiche_se_choisit_dans_le_bloc_photos(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PH-400-OT").await;
    let (pro, part, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;
    let chemin = format!("/vehicules/{vehicule}");

    // Une seule photo : c'est celle de l'en-tête, pas de bloc « Photos ».
    let premiere = deposer_image(
        &pro,
        &pool,
        &cookie,
        vehicule,
        "avant.png",
        "photo_vehicule",
    )
    .await;
    let page = texte(get(&pro, &chemin, Some(&cookie)).await).await;
    assert!(!page.contains(r#"<section id="photos">"#), "{page}");

    // Deux photos : la plus récente est en tête, et le bloc les montre toutes les deux.
    let seconde = deposer_image(
        &pro,
        &pool,
        &cookie,
        vehicule,
        "arriere.png",
        "photo_vehicule",
    )
    .await;
    let page = texte(get(&pro, &chemin, Some(&cookie)).await).await;
    assert!(page.contains(r#"<section id="photos">"#), "{page}");
    assert!(page.contains("Photos (2)"), "{page}");
    assert!(
        page.contains(&vignette_d_en_tete(seconde, "PH-400-OT")),
        "{page}"
    );
    // « Mettre en tête » n'est proposé que sur la photo qui n'y est pas.
    let geste = |id: i64| format!(r#"action="/documents/{id}/mettre-en-tete""#);
    assert!(page.contains(&geste(premiere)), "{page}");
    assert!(!page.contains(&geste(seconde)), "{page}");
    assert!(page.contains("Photo de la fiche"), "{page}");

    // Le choix : retour sur le bloc, avec le message ; la fiche et la carte d'accueil suivent.
    let reponse = post_formulaire(
        &pro,
        &format!("/documents/{premiere}/mettre-en-tete"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}#photos").as_str()
    );
    assert_eq!(fait_pose(&reponse).as_deref(), Some("photo-en-tete"));
    let page = texte(get(&pro, &chemin, Some(&cookie)).await).await;
    assert!(
        page.contains(&vignette_d_en_tete(premiere, "PH-400-OT")),
        "{page}"
    );
    assert!(page.contains(&geste(seconde)), "{page}");
    let cookie_part = se_connecter(&part, "gestion").await;
    let accueil = texte(get(&part, "/", Some(&cookie_part)).await).await;
    assert!(
        accueil.contains(&format!(
            r#"<a class="carte-photo" href="/vehicules/{vehicule}" tabindex="-1" aria-hidden="true"><img src="/documents/{premiere}/vignette" alt=""></a>"#
        )),
        "{accueil}"
    );

    // Une photo déposée ensuite ne reprend pas la tête : le choix tient.
    let troisieme =
        deposer_image(&pro, &pool, &cookie, vehicule, "cote.png", "photo_vehicule").await;
    let page = texte(get(&pro, &chemin, Some(&cookie)).await).await;
    assert!(page.contains("Photos (3)"), "{page}");
    assert!(
        page.contains(&vignette_d_en_tete(premiere, "PH-400-OT")),
        "{page}"
    );
    assert!(page.contains(&geste(troisieme)), "{page}");

    // Choisir une autre photo retire le choix précédent : une seule en tête par véhicule.
    let reponse = post_formulaire(
        &pro,
        &format!("/documents/{troisieme}/mettre-en-tete"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let en_tete: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM documents WHERE vehicule_id = $1 AND photo_de_tete")
            .bind(vehicule)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(en_tete, vec![troisieme]);

    // Supprimer la photo choisie : la plus récente reprend.
    let reponse = post_formulaire(
        &pro,
        &format!("/documents/{troisieme}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let page = texte(get(&pro, &chemin, Some(&cookie)).await).await;
    assert!(
        page.contains(&vignette_d_en_tete(seconde, "PH-400-OT")),
        "{page}"
    );

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn mettre_en_tete_est_reserve_a_l_administrateur_et_aux_photos(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "PH-500-OT").await;
    let (pro, _, dossier) = applications(&pool);
    let cookie = se_connecter(&pro, "gestion").await;
    let cookie_lecteur = se_connecter(&pro, "lecture").await;

    let photo = deposer_image(&pro, &pool, &cookie, vehicule, "a.png", "photo_vehicule").await;
    deposer_image(&pro, &pool, &cookie, vehicule, "b.png", "photo_vehicule").await;
    let facture = deposer_image(&pro, &pool, &cookie, vehicule, "f.png", "facture").await;

    // Un lecteur voit le bloc sans le geste, et ne peut pas l'appeler.
    let chemin = format!("/vehicules/{vehicule}");
    let page = texte(get(&pro, &chemin, Some(&cookie_lecteur)).await).await;
    assert!(page.contains(r#"<section id="photos">"#), "{page}");
    assert!(!page.contains("mettre-en-tete"), "{page}");
    assert!(!page.contains("Ajouter une photo"), "{page}");
    let reponse = post_formulaire(
        &pro,
        &format!("/documents/{photo}/mettre-en-tete"),
        Some(&cookie_lecteur),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);

    // Une facture n'est pas une photo, et un document inconnu n'existe pas : introuvables.
    for id in [facture, 999_999] {
        let reponse = post_formulaire(
            &pro,
            &format!("/documents/{id}/mettre-en-tete"),
            Some(&cookie),
            "",
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::NOT_FOUND, "document {id}");
    }
    let marques: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents WHERE photo_de_tete")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(marques, 0);

    // La base refuse elle-même le marqueur sur autre chose qu'une photo, et un second par
    // véhicule.
    let refus = sqlx::query("UPDATE documents SET photo_de_tete = TRUE WHERE id = $1")
        .bind(facture)
        .execute(&pool)
        .await;
    assert!(refus.is_err());
    sqlx::query("UPDATE documents SET photo_de_tete = TRUE WHERE id = $1")
        .bind(photo)
        .execute(&pool)
        .await
        .unwrap();
    let refus = sqlx::query(
        "UPDATE documents SET photo_de_tete = TRUE
         WHERE vehicule_id = $1 AND type_document = 'photo_vehicule' AND id <> $2",
    )
    .bind(vehicule)
    .bind(photo)
    .execute(&pool)
    .await;
    assert!(refus.is_err());

    let _ = std::fs::remove_dir_all(dossier);
}
