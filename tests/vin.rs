//! Le numéro d'identification du véhicule (VIN), sur base réelle : formulaire, fiche, API,
//! unicité. Tous les numéros sont fictifs.

mod commun;

use axum::http::{header, StatusCode};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, requete_json,
    se_connecter, texte,
};

const FORMULAIRE: &str = "immatriculation=VI-100-NN&marque=Renault&modele=Clio&date_mise_en_service=2022-03-15&carburant=essence&kilometrage=48200&statut=en_service";

#[sqlx::test]
async fn le_vin_est_facultatif_mis_en_forme_et_affiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Sans VIN : accepté, la fiche le dit.
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), FORMULAIRE).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(page.contains("Numéro d'identification (VIN)"), "{page}");
    assert!(page.contains("<em>non renseigné</em>"), "{page}");

    // Avec un VIN tapé comme sur la carte grise (minuscules, espaces) : enregistré en
    // majuscules, affiché sur la fiche, pré-rempli dans le formulaire de modification.
    let corps = format!("{FORMULAIRE}&vin=vf1r+fb00x+12345678");
    let reponse = post_formulaire(&app, &fiche, Some(&cookie), &corps).await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<span class="vin">VF1RFB00X12345678</span>"#),
        "{page}"
    );
    let page = texte(get(&app, &format!("{fiche}/modifier"), Some(&cookie)).await).await;
    assert!(
        page.contains(r#"name="vin" value="VF1RFB00X12345678""#),
        "{page}"
    );

    // L'API le renvoie aussi.
    let reponse = requete_json(&app, "GET", &format!("/api/v1{fiche}"), Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse)
        .await
        .contains(r#""vin":"VF1RFB00X12345678""#));
}

#[sqlx::test]
async fn un_vin_mal_forme_ou_deja_pris_est_refuse_avec_la_regle(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let autre = creer_vehicule(&pool, "VI-200-NN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // 16 caractères, puis une lettre interdite : le formulaire revient avec la règle.
    for mauvais in ["VF1RFB00X1234567", "VF1RFB00O12345678"] {
        let corps = format!("{FORMULAIRE}&vin={mauvais}");
        let page = texte(post_formulaire(&app, "/vehicules", Some(&cookie), &corps).await).await;
        assert!(
            page.contains("17 caractères, lettres et chiffres"),
            "{mauvais}\n{page}"
        );
        assert!(
            page.contains(r#"class="message-erreur" role="alert""#),
            "{mauvais}\n{page}"
        );
    }

    // Le même VIN sur deux véhicules : refusé, le message dit lequel est pris.
    let corps = format!("{FORMULAIRE}&vin=VF1RFB00X12345678");
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let corps = "immatriculation=VI-200-NN&marque=Renault&modele=Kangoo&date_mise_en_service=2023-06-01&carburant=diesel&kilometrage=12000&statut=en_service&vin=VF1RFB00X12345678";
    let page =
        texte(post_formulaire(&app, &format!("/vehicules/{autre}"), Some(&cookie), corps).await)
            .await;
    // Le gabarit échappe les apostrophes du message (`&#39;`).
    assert!(
        page.contains("« VF1RFB00X12345678 » est déjà celui d&#39;un autre véhicule."),
        "{page}"
    );

    // Deux véhicules sans VIN, eux, ne se gênent pas.
    let corps = "immatriculation=VI-300-NN&marque=Renault&modele=Clio&date_mise_en_service=2022-03-15&carburant=essence&kilometrage=1&statut=en_service&vin=";
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    // L'API applique la même règle.
    let corps = r#"{"immatriculation":"VI-400-NN","marque":"Renault","modele":"Clio","date_mise_en_service":"2022-03-15","carburant":"essence","kilometrage":"1","statut":"en_service","vin":"TROP-COURT"}"#;
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/vehicules",
        Some(&cookie),
        Some(corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert!(texte(reponse).await.contains("17 caractères"));
}
