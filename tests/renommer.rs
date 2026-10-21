//! Renommer un document : un nom qui parle à la place de celui de l'appareil photo. Le nom change
//! à l'écran et au téléchargement ; le fichier sur disque et son extension ne bougent pas.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::metier::documents as metier_documents;
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, post_multipart,
    se_connecter, texte,
};

const NOM_DEPOSE: &str = "1792151374112342849675.pdf";

/// Dépose un PDF sous un nom d'appareil photo ; renvoie l'identifiant du document.
async fn deposer(app: &axum::Router, cookie: &str, pool: &PgPool, vehicule: i64) -> i64 {
    let reponse = post_multipart(
        app,
        cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[("type_document", "controle_technique")],
        &[(NOM_DEPOSE, "application/pdf", b"%PDF-1.4 test")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    metier_documents::lister_par_vehicule(pool, vehicule)
        .await
        .unwrap()[0]
        .id
}

#[sqlx::test]
async fn un_administrateur_renomme_un_document_et_garde_son_extension(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "RE-100-NM").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let id = deposer(&app, &cookie, &pool, vehicule).await;
    let avant = metier_documents::obtenir(&pool, id).await.unwrap();

    // La page du document et la liste de la fiche proposent « Renommer ».
    let lien = format!("/documents/{id}/renommer");
    let page = texte(get(&app, &format!("/documents/{id}"), Some(&cookie)).await).await;
    assert!(page.contains(&format!(r#"href="{lien}""#)), "{page}");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(&format!(r#"href="{lien}""#)), "{fiche}");

    // Le formulaire montre la racine du nom, l'extension à part.
    let page = texte(get(&app, &lien, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"name="nom" value="1792151374112342849675""#),
        "{page}"
    );
    assert!(page.contains(r#"<span class="extension""#), "{page}");
    assert!(page.contains(">.pdf</span>"), "{page}");

    // Une date tapée avec des barres devient des tirets ; l'extension est remise.
    let reponse = post_formulaire(
        &app,
        &lien,
        Some(&cookie),
        "nom=Contr%C3%B4le+technique+12%2F03%2F2025",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    // La redirection pose le cookie du message de confirmation, que le navigateur renvoie.
    let fait = reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.strip_prefix("fait=")
                .map(|r| r.split(';').next().unwrap().to_string())
        })
        .expect("le cookie du message");
    assert_eq!(fait, "document-renomme");
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/documents/{id}").as_str()
    );
    let apres = metier_documents::obtenir(&pool, id).await.unwrap();
    assert_eq!(apres.nom_original, "Contrôle technique 12-03-2025.pdf");
    assert_eq!(
        apres.chemin, avant.chemin,
        "le fichier sur disque ne bouge pas"
    );
    assert_eq!(apres.type_mime, avant.type_mime);

    // La page dit « Document renommé » et montre le nouveau nom ; le téléchargement l'emploie.
    let avec_message = format!("{cookie}; fait={fait}");
    let page = texte(get(&app, &format!("/documents/{id}"), Some(&avec_message)).await).await;
    assert!(page.contains("Document renommé."), "{page}");
    assert!(page.contains("Contrôle technique 12-03-2025.pdf"), "{page}");
    let reponse = get(&app, &format!("/documents/{id}/telecharger"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let disposition = reponse.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    // Le nom accentué est dit en UTF-8 encodé (RFC 6266), pas en octets bruts.
    assert!(
        disposition.contains("filename*=UTF-8''Contr%C3%B4le%20technique%2012-03-2025.pdf"),
        "{disposition}"
    );

    // Une extension tapée en trop n'est pas doublée.
    let reponse = post_formulaire(&app, &lien, Some(&cookie), "nom=Carte+grise.PDF").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let apres = metier_documents::obtenir(&pool, id).await.unwrap();
    assert_eq!(apres.nom_original, "Carte grise.pdf");
}

#[sqlx::test]
async fn un_nom_vide_ou_trop_long_est_refuse_avec_le_motif(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "RE-200-NM").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let id = deposer(&app, &cookie, &pool, vehicule).await;
    let lien = format!("/documents/{id}/renommer");

    // Vide, fait de caractères retirés ou réduit à l'extension (#117) : refusé, le formulaire
    // revient avec le motif.
    for corps in ["nom=", "nom=+++", "nom=%2A%3F", "nom=.pdf", "nom=.PDF"] {
        let reponse = post_formulaire(&app, &lien, Some(&cookie), corps).await;
        assert_eq!(reponse.status(), StatusCode::OK, "{corps}");
        let page = texte(reponse).await;
        assert!(page.contains("ne peut pas être vide"), "{corps}\n{page}");
    }

    // Trop long : refusé, ce qui a été tapé reste dans le champ.
    let long = "a".repeat(201);
    let reponse = post_formulaire(&app, &lien, Some(&cookie), &format!("nom={long}")).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("dépasse 200 caractères"), "{page}");
    assert!(page.contains(&format!(r#"value="{long}""#)), "{page}");

    // Rien n'a changé.
    let document = metier_documents::obtenir(&pool, id).await.unwrap();
    assert_eq!(document.nom_original, NOM_DEPOSE);
}

#[sqlx::test]
async fn un_nom_de_200_caracteres_garde_son_extension_au_telechargement(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "RE-250-NM").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let id = deposer(&app, &cookie, &pool, vehicule).await;
    let lien = format!("/documents/{id}/renommer");

    // 196 caractères + « .pdf » : la limite du renommage, plus longue que celle du téléchargement.
    let racine = "a".repeat(196);
    let reponse = post_formulaire(&app, &lien, Some(&cookie), &format!("nom={racine}")).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let document = metier_documents::obtenir(&pool, id).await.unwrap();
    assert_eq!(document.nom_original, format!("{racine}.pdf"));

    // Le fichier reçu est raccourci, mais il finit toujours par « .pdf ».
    let reponse = get(&app, &format!("/documents/{id}/telecharger"), Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let disposition = reponse.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(disposition.ends_with(".pdf\""), "{disposition}");
    assert!(!disposition.contains(&racine), "{disposition}");
}

#[sqlx::test]
async fn un_lecteur_ne_renomme_pas(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecteur", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "RE-300-NM").await;
    let app = application(pool.clone());
    let admin = se_connecter(&app, "gestion").await;
    let id = deposer(&app, &admin, &pool, vehicule).await;
    let cookie = se_connecter(&app, "lecteur").await;
    let lien = format!("/documents/{id}/renommer");

    // Pas de bouton sur la page du document, ni dans la liste.
    let page = texte(get(&app, &format!("/documents/{id}"), Some(&cookie)).await).await;
    assert!(!page.contains(&lien), "{page}");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(!fiche.contains(&lien), "{fiche}");

    // Et la route elle-même est refusée, en lecture comme en écriture.
    let reponse = get(&app, &lien, Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let reponse = post_formulaire(&app, &lien, Some(&cookie), "nom=Piratage").await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let document = metier_documents::obtenir(&pool, id).await.unwrap();
    assert_eq!(document.nom_original, NOM_DEPOSE);
}

#[sqlx::test]
async fn renommer_un_document_inconnu_est_introuvable(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let reponse = get(&app, "/documents/99999/renommer", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
    let reponse = post_formulaire(&app, "/documents/99999/renommer", Some(&cookie), "nom=x").await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}
