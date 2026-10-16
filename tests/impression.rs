//! Impression, sur base réelle : la page d'un document (image, PDF, autre fichier) et le bouton
//! « Imprimer » des fiches et des listes. Documents et numéros fictifs.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local};
use gestion_parc_auto::etat::Etat;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention};
use sqlx::PgPool;

use commun::{
    application, creer_conducteur, creer_utilisateur, creer_vehicule, get, post_multipart,
    se_connecter, texte,
};

type Fichier<'a> = (&'a str, &'a str, &'a [u8]);

const BOUTON_IMPRIMER: &str =
    r#"<button type="button" class="bouton-discret" data-imprimer>Imprimer</button>"#;

/// Dépose un fichier et renvoie l'identifiant du document créé.
async fn deposer(
    app: &axum::Router,
    pool: &PgPool,
    cookie: &str,
    chemin: &str,
    type_document: &str,
    fichier: Fichier<'_>,
) -> i64 {
    let reponse = post_multipart(
        app,
        cookie,
        chemin,
        &[("type_document", type_document)],
        &[fichier],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{}", fichier.0);
    sqlx::query_scalar("SELECT id FROM documents WHERE nom_original = $1")
        .bind(fichier.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn la_page_d_un_document_le_montre_et_propose_de_l_imprimer(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "IM-100-PR").await;
    let conducteur = creer_conducteur(&pool, "Dupont").await;
    let config = commun::config_de_test();
    let dossier = config.documents_dir.clone();
    let app = gestion_parc_auto::web::routeur(Etat::nouveau(pool.clone(), config).expect("état"));
    let cookie = se_connecter(&app, "gestion").await;

    let depot_vehicule = format!("/vehicules/{vehicule}/documents");
    let image = deposer(
        &app,
        &pool,
        &cookie,
        &depot_vehicule,
        "carte_grise",
        ("carte-grise.jpg", "image/jpeg", b"fausse photo"),
    )
    .await;
    let pdf = deposer(
        &app,
        &pool,
        &cookie,
        &depot_vehicule,
        "facture",
        ("facture.pdf", "application/pdf", b"%PDF-1.4 essai"),
    )
    .await;
    let autre = deposer(
        &app,
        &pool,
        &cookie,
        &depot_vehicule,
        "autre",
        (
            "devis.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            b"devis",
        ),
    )
    .await;
    let permis = deposer(
        &app,
        &pool,
        &cookie,
        &format!("/conducteurs/{conducteur}/documents"),
        "permis",
        ("permis.png", "image/png", b"fausse photo"),
    )
    .await;

    // La fiche mène à la page du document, plus au fichier brut ; « Télécharger » reste.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(
            r#"<a href="/documents/{image}">carte-grise.jpg</a>"#
        )),
        "{fiche}"
    );
    assert!(
        fiche.contains(&format!(
            r#"<a href="/documents/{image}/telecharger">Télécharger</a>"#
        )),
        "{fiche}"
    );

    // Une image : affichée, imprimable, téléchargeable, avec le retour vers la fiche. Pour le
    // lecteur aussi.
    let lecteur = se_connecter(&app, "lecture").await;
    for session in [&cookie, &lecteur] {
        let reponse = get(&app, &format!("/documents/{image}"), Some(session)).await;
        assert_eq!(reponse.status(), StatusCode::OK);
        let page = texte(reponse).await;
        for attendu in [
            "<title>Carte grise · IM-100-PR (Renault Kangoo) · Autoclair Pro</title>".to_string(),
            format!(r#"<a class="retour" href="/vehicules/{vehicule}#documents">IM-100-PR</a>"#),
            "<h1>Carte grise <small>IM-100-PR (Renault Kangoo)</small></h1>".to_string(),
            r#"<button type="button" class="bouton" data-imprimer>Imprimer</button>"#.to_string(),
            format!(
                r#"<a class="bouton-discret" href="/documents/{image}/telecharger">Télécharger</a>"#
            ),
            format!(
                r#"<img src="/documents/{image}/voir" alt="Carte grise, IM-100-PR (Renault Kangoo)">"#
            ),
            "<figcaption>Carte grise · IM-100-PR (Renault Kangoo) · déposé le ".to_string(),
            r#"<a href="/vehicules" data-ico="voiture" class="actif" aria-current="page">Véhicules</a>"#.to_string(),
            "carte-grise.jpg · 12 octets · déposé le ".to_string(),
        ] {
            assert!(page.contains(&attendu), "{attendu}\n{page}");
        }
        assert!(!page.contains("Ouvrir le PDF"), "{page}");
    }

    // Un PDF : pas de cadre (les en-têtes de sécurité l'interdisent), un lien vers un autre
    // onglet, d'où le lecteur du navigateur imprime.
    let page = texte(get(&app, &format!("/documents/{pdf}"), Some(&cookie)).await).await;
    assert!(
        page.contains(&format!(
            r#"<a class="bouton" href="/documents/{pdf}/voir" target="_blank" rel="noopener">Ouvrir le PDF</a>"#
        )),
        "{page}"
    );
    assert!(
        page.contains("<h1>Facture <small>IM-100-PR (Renault Kangoo)</small></h1>"),
        "{page}"
    );
    assert!(!page.contains(r#"class="visionneuse""#), "{page}");
    assert!(!page.contains("data-imprimer"), "{page}");

    // Tout autre fichier : téléchargement seulement.
    let page = texte(get(&app, &format!("/documents/{autre}"), Some(&cookie)).await).await;
    assert!(page.contains("téléchargez-le pour l'ouvrir"), "{page}");
    assert!(
        page.contains(&format!(r#"href="/documents/{autre}/telecharger""#)),
        "{page}"
    );
    assert!(!page.contains(r#"class="visionneuse""#), "{page}");
    assert!(!page.contains("Ouvrir le PDF"), "{page}");
    assert!(!page.contains("data-imprimer"), "{page}");

    // Le document d'un conducteur revient à sa fiche, et le menu marque « Conducteurs ».
    let page = texte(get(&app, &format!("/documents/{permis}"), Some(&cookie)).await).await;
    for attendu in [
        format!(
            r#"<a class="retour" href="/conducteurs/{conducteur}#documents">Camille Dupont</a>"#
        ),
        "<h1>Permis de conduire <small>Camille Dupont</small></h1>".to_string(),
        r#"<a href="/conducteurs" data-ico="personnes" class="actif" aria-current="page">Conducteurs</a>"#.to_string(),
        format!(r#"<img src="/documents/{permis}/voir""#),
    ] {
        assert!(page.contains(&attendu), "{attendu}\n{page}");
    }

    // Sans session, pas de page ; un document inconnu est introuvable.
    let reponse = get(&app, &format!("/documents/{image}"), None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/connexion");
    for chemin in ["/documents/999999", "/documents/abc"] {
        let reponse = get(&app, chemin, Some(&cookie)).await;
        assert_eq!(reponse.status(), StatusCode::NOT_FOUND, "{chemin}");
    }

    let _ = std::fs::remove_dir_all(dossier);
}

#[sqlx::test]
async fn les_fiches_et_les_listes_ont_un_bouton_imprimer(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "IM-200-PR").await;
    let conducteur = creer_conducteur(&pool, "Martin").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Une liste vide n'a rien à imprimer.
    for chemin in ["/echeances", "/interventions", "/alertes"] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(!page.contains("data-imprimer"), "{chemin}\n{page}");
    }

    // Une échéance dépassée (donc une alerte) et une intervention.
    let hier = (Local::now().date_naive() - Duration::days(1)).to_string();
    echeances::creer(
        &pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: "controle_technique".to_string(),
            date_limite: hier,
            statut: String::new(),
        },
    )
    .await
    .unwrap();
    interventions::creer(
        &pool,
        &SaisieIntervention {
            vehicule_id: vehicule.to_string(),
            type_intervention: "revision".to_string(),
            resultat_controle: String::new(),
            date_intervention: "2026-01-10".to_string(),
            kilometrage: "13000".to_string(),
            cout: "120".to_string(),
            prestataire: "Garage fictif".to_string(),
            notes: String::new(),
            lignes: Vec::new(),
        },
    )
    .await
    .unwrap();

    // Les fiches et les listes garnies ont le bouton, pour l'administrateur comme pour le lecteur.
    let lecteur = se_connecter(&app, "lecture").await;
    let fiche_vehicule = format!("/vehicules/{vehicule}");
    let fiche_conducteur = format!("/conducteurs/{conducteur}");
    for session in [&cookie, &lecteur] {
        for chemin in [
            fiche_vehicule.as_str(),
            fiche_conducteur.as_str(),
            "/echeances",
            "/interventions",
            "/alertes",
        ] {
            let page = texte(get(&app, chemin, Some(session)).await).await;
            assert!(page.contains(BOUTON_IMPRIMER), "{chemin}\n{page}");
            // Un seul bouton cyan par écran : « Imprimer » n'en est pas un.
            assert!(
                !page.contains(r#"class="bouton" data-imprimer"#),
                "{chemin}\n{page}"
            );
            // Le nom de l'application, que seule l'impression affiche.
            assert!(
                page.contains(r#"<p class="pour-impression">Autoclair Pro</p>"#),
                "{chemin}\n{page}"
            );
        }
    }

    // Les écrans d'action et les formulaires ne s'impriment pas.
    for chemin in [
        "/",
        "/vehicules",
        "/vehicules/nouveau",
        "/documents/nouveau",
    ] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(!page.contains("data-imprimer"), "{chemin}\n{page}");
    }
}

/// La feuille de style et le script portent leur part : sans l'une, le papier garde le menu ;
/// sans l'autre, le bouton ne fait rien.
#[test]
fn la_feuille_de_style_et_le_script_prennent_l_impression_en_charge() {
    let style = include_str!("../static/style.css");
    let script = include_str!("../static/app.js");
    assert!(style.contains("@media print"));
    assert!(style.contains(".pour-impression { display: none; }"));
    assert!(style.contains(".visionneuse figcaption { display: none; }"));
    assert!(script.contains("[data-imprimer]"));
    assert!(script.contains("window.print()"));
}
