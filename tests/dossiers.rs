//! Les dossiers de documents de la fiche véhicule (#109) : Achat, Administratif, Entretien,
//! Dépenses (#126, bloc 2), Autres. Le dossier se déduit du type ; la photo du véhicule n'y est pas ; l'administrateur voit
//! les dossiers vides et le lecteur non ; « Ajouter » ouvre le dépôt sur les types du dossier.

mod commun;

use std::io::Cursor;

use axum::http::StatusCode;
use image::{ImageFormat, Rgb, RgbImage};
use sqlx::PgPool;

use commun::{
    application, creer_conducteur, creer_utilisateur, creer_vehicule, get, post_multipart,
    se_connecter, texte,
};

fn png() -> Vec<u8> {
    let image = RgbImage::from_fn(320, 240, |x, y| Rgb([x as u8, y as u8, 90]));
    let mut tampon = Cursor::new(Vec::new());
    image.write_to(&mut tampon, ImageFormat::Png).unwrap();
    tampon.into_inner()
}

async fn deposer(
    app: &axum::Router,
    cookie: &str,
    vehicule: i64,
    type_document: &str,
    nom: &str,
    type_mime: &str,
    contenu: &[u8],
) {
    let reponse = post_multipart(
        app,
        cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[("type_document", type_document)],
        &[(nom, type_mime, contenu)],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{type_document} {nom}"
    );
}

/// La position d'un texte dans la page, pour vérifier l'ordre d'affichage.
fn position(page: &str, texte: &str) -> usize {
    page.find(texte)
        .unwrap_or_else(|| panic!("« {texte} » absent\n{page}"))
}

#[sqlx::test]
async fn la_fiche_range_les_documents_en_dossiers(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "DO-109-SS").await;
    let sans_rien = creer_vehicule(&pool, "DO-209-SS").await;
    let conducteur = creer_conducteur(&pool, "Camille Dossier").await;
    let app = application(pool.clone());
    let admin = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;
    let fiche = format!("/vehicules/{vehicule}");

    // Fiche vide, administrateur : les cinq dossiers, chacun vide, chacun avec « Ajouter ».
    let page = texte(get(&app, &fiche, Some(&admin)).await).await;
    for (code, libelle) in [
        ("achat", "Achat"),
        ("administratif", "Administratif"),
        ("entretien", "Entretien"),
        ("depenses", "Dépenses"),
        ("autres", "Autres"),
    ] {
        assert!(
            page.contains(&format!("<h3>{libelle}</h3>")),
            "{libelle}\n{page}"
        );
        assert!(
            page.contains(&format!(
                r#"href="/documents/nouveau?vehicule_id={vehicule}&dossier={code}" aria-label="Ajouter au dossier {libelle}">Ajouter</a>"#
            )),
            "{code}\n{page}"
        );
    }
    assert_eq!(page.matches("Aucun document.").count(), 5, "{page}");
    // Les sous-titres suivent le titre de la section, dans l'ordre des dossiers.
    let documents = position(&page, r#"<section id="documents">"#);
    assert!(documents < position(&page, "<h3>Achat</h3>"));
    assert!(position(&page, "<h3>Achat</h3>") < position(&page, "<h3>Administratif</h3>"));
    assert!(position(&page, "<h3>Administratif</h3>") < position(&page, "<h3>Entretien</h3>"));
    assert!(position(&page, "<h3>Entretien</h3>") < position(&page, "<h3>Dépenses</h3>"));
    assert!(position(&page, "<h3>Dépenses</h3>") < position(&page, "<h3>Autres</h3>"));

    // Fiche vide, lecteur : rien que « Aucun document. », ni dossier ni bouton.
    let page = texte(get(&app, &fiche, Some(&lecteur)).await).await;
    assert!(!page.contains("<h3>"), "{page}");
    assert!(!page.contains(">Ajouter</a>"), "{page}");
    assert_eq!(page.matches("Aucun document.").count(), 1, "{page}");

    // Un document par dossier, et une photo.
    deposer(
        &app,
        &admin,
        vehicule,
        "facture_achat",
        "facture-achat.pdf",
        "application/pdf",
        b"%PDF-1.4 achat",
    )
    .await;
    deposer(
        &app,
        &admin,
        vehicule,
        "carte_grise",
        "carte-grise.pdf",
        "application/pdf",
        b"%PDF-1.4 cg",
    )
    .await;
    deposer(
        &app,
        &admin,
        vehicule,
        "facture",
        "facture-revision.pdf",
        "application/pdf",
        b"%PDF-1.4 rev",
    )
    .await;
    deposer(
        &app,
        &admin,
        vehicule,
        "photo_vehicule",
        "photo-avant.png",
        "image/png",
        &png(),
    )
    .await;

    // Administrateur : chaque document sous son sous-titre, la photo nulle part dans les tableaux,
    // les dossiers « Dépenses » et « Autres » vides restent visibles.
    let page = texte(get(&app, &fiche, Some(&admin)).await).await;
    let achat = position(&page, "<h3>Achat</h3>");
    let administratif = position(&page, "<h3>Administratif</h3>");
    let entretien = position(&page, "<h3>Entretien</h3>");
    let autres = position(&page, "<h3>Autres</h3>");
    let facture_achat = position(&page, "facture-achat.pdf");
    let carte_grise = position(&page, "carte-grise.pdf");
    let facture = position(&page, "facture-revision.pdf");
    assert!(
        achat < facture_achat && facture_achat < administratif,
        "{page}"
    );
    assert!(
        administratif < carte_grise && carte_grise < entretien,
        "{page}"
    );
    assert!(entretien < facture && facture < autres, "{page}");
    assert!(
        !page.contains("photo-avant.png"),
        "la photo a son bloc\n{page}"
    );
    assert!(page.contains(">Facture d&#39;achat</td>"), "{page}");
    assert_eq!(page.matches("Aucun document.").count(), 2, "{page}");

    // Lecteur : seuls les dossiers pleins, sans bouton.
    let page = texte(get(&app, &fiche, Some(&lecteur)).await).await;
    assert!(page.contains("<h3>Achat</h3>"), "{page}");
    assert!(page.contains("<h3>Administratif</h3>"), "{page}");
    assert!(page.contains("<h3>Entretien</h3>"), "{page}");
    assert!(!page.contains("<h3>Dépenses</h3>"), "{page}");
    assert!(!page.contains("<h3>Autres</h3>"), "{page}");
    assert!(!page.contains(">Ajouter</a>"), "{page}");
    assert!(!page.contains("Aucun document."), "{page}");
    assert!(page.contains("facture-achat.pdf"), "{page}");

    // La page d'un document le supprime (administrateur seulement) : c'est le seul endroit pour une
    // photo, qui n'est plus dans le tableau.
    let id: i64 =
        sqlx::query_scalar("SELECT id FROM documents WHERE nom_original = 'photo-avant.png'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let suppression = format!(r#"action="/documents/{id}/supprimer""#);
    let page = texte(get(&app, &format!("/documents/{id}"), Some(&admin)).await).await;
    assert!(page.contains(&suppression), "{page}");
    assert!(page.contains(">Supprimer</button>"), "{page}");
    let page = texte(get(&app, &format!("/documents/{id}"), Some(&lecteur)).await).await;
    assert!(!page.contains(&suppression), "{page}");

    // Un autre véhicule n'a rien : ses dossiers sont vides, quels que soient les dépôts ailleurs.
    let page = texte(get(&app, &format!("/vehicules/{sans_rien}"), Some(&admin)).await).await;
    assert_eq!(page.matches("Aucun document.").count(), 5, "{page}");
    assert!(!page.contains("facture-achat.pdf"), "{page}");

    // La fiche d'un conducteur ne change pas : un tableau à plat, sans dossier ; sa liste de
    // dépôt ne propose pas les types d'achat.
    let page = texte(get(&app, &format!("/conducteurs/{conducteur}"), Some(&admin)).await).await;
    assert!(!page.contains("<h3>"), "{page}");
    assert!(page.contains("Aucun document."), "{page}");
    assert!(page.contains(r#"<option value="permis">"#), "{page}");
    assert!(!page.contains(r#"value="facture_achat""#), "{page}");
}

#[sqlx::test]
async fn ajouter_depuis_un_dossier_ne_propose_que_ses_types(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DO-309-SS").await;
    let app = application(pool.clone());
    let admin = se_connecter(&app, "gestion").await;

    // Depuis le dossier « Achat » : ses quatre types, le premier pré-choisi.
    let page = texte(
        get(
            &app,
            &format!("/documents/nouveau?vehicule_id={vehicule}&dossier=achat"),
            Some(&admin),
        )
        .await,
    )
    .await;
    let liste = &page[position(&page, r#"name="type_document""#)..];
    let liste = &liste[..position(liste, "</select>")];
    assert!(
        liste.contains(r#"<option value="facture_achat" selected>Facture d&#39;achat</option>"#),
        "{liste}"
    );
    for code in ["certificat_cession", "contrat_achat", "garantie"] {
        assert!(
            liste.contains(&format!(r#"<option value="{code}">"#)),
            "{code}\n{liste}"
        );
    }
    for code in [
        "photo_vehicule",
        "facture",
        "controle_technique",
        "carte_grise",
        "assurance",
        "autre",
    ] {
        assert!(
            !liste.contains(&format!(r#"value="{code}""#)),
            "{code}\n{liste}"
        );
    }
    assert_eq!(
        liste.matches("<option").count(),
        5,
        "— choisir — et quatre types\n{liste}"
    );
    // Le véhicule de la fiche reste choisi.
    assert!(
        page.contains(&format!(r#"<option value="{vehicule}" selected>"#)),
        "{page}"
    );

    // Un type nommé dans l'adresse l'emporte sur le premier du dossier.
    let page = texte(
        get(
            &app,
            &format!(
                "/documents/nouveau?vehicule_id={vehicule}&dossier=achat&type_document=garantie"
            ),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"<option value="garantie" selected>"#),
        "{page}"
    );
    assert!(
        !page.contains(r#"<option value="facture_achat" selected>"#),
        "{page}"
    );

    // Depuis « Administratif » : carte grise pré-choisie, assurance proposée.
    let page = texte(
        get(
            &app,
            &format!("/documents/nouveau?vehicule_id={vehicule}&dossier=administratif"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"<option value="carte_grise" selected>"#),
        "{page}"
    );
    assert!(page.contains(r#"<option value="assurance">"#), "{page}");
    assert!(!page.contains(r#"value="facture_achat""#), "{page}");

    // Un dossier inconnu, ou pas de dossier : la liste complète, rien de pré-choisi.
    for adresse in [
        format!("/documents/nouveau?vehicule_id={vehicule}&dossier=inconnu"),
        format!("/documents/nouveau?vehicule_id={vehicule}"),
    ] {
        let page = texte(get(&app, &adresse, Some(&admin)).await).await;
        let liste = &page[position(&page, r#"name="type_document""#)..];
        let liste = &liste[..position(liste, "</select>")];
        assert_eq!(liste.matches("<option").count(), 12, "{adresse}\n{liste}");
        assert!(!liste.contains(" selected"), "{adresse}\n{liste}");
    }
}
