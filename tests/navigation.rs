//! Navigation par profil, sur base réelle : la barre latérale et la recherche d'Autoclair Pro,
//! les quatre onglets et la page « Plus » d'Autoclair. Données fictives.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_conducteur, creer_utilisateur, creer_vehicule, get,
    se_connecter, texte,
};

#[sqlx::test]
async fn autoclair_pro_a_une_barre_laterale_avec_la_recherche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "NA-100-VP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    for attendu in [
        r#"<body class="pro">"#,
        r#"<header class="entete laterale">"#,
        r#"<img class="logo" src="/static/logos/autoclair-pro-negatif.svg" alt="Autoclair Pro">"#,
        r#"<button type="button" class="menu-bascule" data-menu aria-controls="menu" aria-expanded="false"><span class="menu-libelle">Menu</span></button>"#,
        r#"<form class="recherche" method="get" action="/recherche" role="search">"#,
        "data-raccourci",
        r#"<a href="/" data-ico="tableau" class="actif" aria-current="page">Tableau de bord</a>"#,
        r#"<a href="/alertes" data-ico="cloche">Alertes</a>"#,
        r#"<a href="/affectations" data-ico="echange">Affectations</a>"#,
        r#"<a href="/depenses" data-ico="carburant">Dépenses</a>"#,
        r#"<a href="/documents/nouveau" data-ico="depot">Déposer un document</a>"#,
        // Les titres de section ont leur icône.
        r#"<h2 data-ico="cloche">Par urgence</h2>"#,
    ] {
        assert!(page.contains(attendu), "{attendu}\n{page}");
    }
    // Pas d'onglet « Plus » : tout est dans la barre.
    assert!(!page.contains(r#"href="/plus""#), "{page}");

    // Une fiche allume « Véhicules » et porte ses icônes de section.
    let page = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    for attendu in [
        r#"<a href="/vehicules" data-ico="voiture" class="actif" aria-current="page">Véhicules</a>"#,
        r#"<h2 data-ico="calendrier">Échéances</h2>"#,
        r#"<h2 data-ico="cle">Entretien</h2>"#,
        r#"<h2 data-ico="carburant">Dépenses</h2>"#,
        r#"<h2 data-ico="historique">Historique des affectations</h2>"#,
        r#"<h2 data-ico="fichier">Documents</h2>"#,
    ] {
        assert!(page.contains(attendu), "{attendu}\n{page}");
    }

    // Le lecteur a la barre et la recherche, sans « Déposer un document ».
    let lecteur = se_connecter(&app, "lecture").await;
    let page = texte(get(&app, "/vehicules", Some(&lecteur)).await).await;
    assert!(page.contains("data-raccourci"), "{page}");
    assert!(!page.contains("/documents/nouveau"), "{page}");
}

#[sqlx::test]
async fn autoclair_a_quatre_onglets_et_une_page_plus(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "NA-200-VP").await;
    let conducteur = creer_conducteur(&pool, "Navarro").await;
    let app = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&app, "gestion").await;

    const ONGLETS: [&str; 4] = [
        r#"<a href="/" data-ico="voiture""#,
        r#"<a href="/echeances" data-ico="calendrier""#,
        r#"<a href="/interventions" data-ico="cle""#,
        r#"<a href="/plus" data-ico="plus""#,
    ];
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(page.contains(r#"<body class="part">"#), "{page}");
    assert!(page.contains(r#"<nav class="menu onglets">"#), "{page}");
    for onglet in ONGLETS {
        assert!(page.contains(onglet), "{onglet}\n{page}");
    }
    // Ni recherche dans l'en-tête, ni entrées de menu au-delà des quatre.
    assert!(!page.contains("data-raccourci"), "{page}");
    assert!(!page.contains(r#"data-ico="cloche""#), "{page}");
    assert!(
        !page.contains(r#"href="/documents/nouveau" data-ico"#),
        "{page}"
    );

    // Chaque page allume un onglet, et un seul.
    for (chemin, onglet) in [
        ("/".to_string(), "Véhicules"),
        (format!("/vehicules/{vehicule}"), "Véhicules"),
        ("/vehicules".to_string(), "Véhicules"),
        ("/echeances".to_string(), "Échéances"),
        ("/interventions".to_string(), "Entretien"),
        ("/alertes".to_string(), "Plus"),
        (format!("/conducteurs/{conducteur}"), "Plus"),
        ("/documents/nouveau".to_string(), "Plus"),
        ("/plus".to_string(), "Plus"),
        ("/recherche?q=na".to_string(), "Plus"),
    ] {
        let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
        let actif = format!(r#" class="actif" aria-current="page">{onglet}</a>"#);
        assert!(page.contains(&actif), "{chemin}\n{page}");
        assert_eq!(page.matches("aria-current").count(), 1, "{chemin}\n{page}");
    }

    // « Plus » : le reste du menu, la recherche, la déconnexion.
    let page = texte(get(&app, "/plus", Some(&cookie)).await).await;
    for attendu in [
        "<title>Plus · Autoclair</title>",
        r#"<a href="/alertes" data-ico="cloche">Alertes</a>"#,
        r#"<a href="/vehicules" data-ico="voiture">Tous les véhicules</a>"#,
        r#"<a href="/conducteurs" data-ico="personnes">Conducteurs</a>"#,
        r#"<a href="/documents/nouveau" data-ico="depot">Déposer un document</a>"#,
        r#"action="/recherche""#,
        r#"<form method="post" action="/deconnexion" class="actions">"#,
    ] {
        assert!(page.contains(attendu), "{attendu}\n{page}");
    }
    // Un particulier n'a pas d'affectations ; un lecteur ne dépose rien.
    assert!(!page.contains(r#"href="/affectations""#), "{page}");
    let lecteur = se_connecter(&app, "lecture").await;
    let page = texte(get(&app, "/plus", Some(&lecteur)).await).await;
    assert!(!page.contains("/documents/nouveau"), "{page}");
    assert!(page.contains("lecture seule"), "{page}");

    // Dans une entreprise, « Plus » existe aussi (adresse directe) et liste les affectations.
    let pro = application(pool.clone());
    let cookie_pro = se_connecter(&pro, "gestion").await;
    let page = texte(get(&pro, "/plus", Some(&cookie_pro)).await).await;
    assert!(
        page.contains(r#"<a href="/affectations" data-ico="echange">Affectations</a>"#),
        "{page}"
    );

    // Sans session, pas de page.
    let reponse = get(&app, "/plus", None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/connexion");
}

#[sqlx::test]
async fn la_recherche_trouve_vehicules_et_conducteurs(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let kangoo = creer_vehicule(&pool, "NA-300-VP").await;
    let autre = creer_vehicule(&pool, "NA-400-VP").await;
    let conducteur = creer_conducteur(&pool, "Navarro").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Par un bout d'immatriculation, en minuscules.
    let page = texte(get(&app, "/recherche?q=na-3", Some(&cookie)).await).await;
    assert!(
        page.contains("<title>Recherche · Autoclair Pro</title>"),
        "{page}"
    );
    assert!(page.contains(&format!(r#"<a href="/vehicules/{kangoo}"><span class="plaque">NA-300-VP</span></a> Renault Kangoo"#)), "{page}");
    assert!(!page.contains("NA-400-VP"), "{page}");
    assert!(page.contains(r#"value="na-3""#), "{page}");

    // Par le modèle : les deux véhicules, aucun conducteur.
    let page = texte(get(&app, "/recherche?q=KANGOO", Some(&cookie)).await).await;
    assert!(
        page.contains("NA-300-VP") && page.contains("NA-400-VP"),
        "{page}"
    );
    assert!(
        page.contains(r#"<h2 data-ico="voiture">Véhicules</h2>"#),
        "{page}"
    );
    assert!(
        !page.contains(r#"<h2 data-ico="personnes">Conducteurs</h2>"#),
        "{page}"
    );

    // Par le nom : le conducteur, avec ses initiales.
    let page = texte(get(&app, "/recherche?q=navar", Some(&cookie)).await).await;
    assert!(
        page.contains(&format!(
            r#"<span class="initiales" aria-hidden="true">CN</span><a href="/conducteurs/{conducteur}">Camille Navarro</a>"#
        )),
        "{page}"
    );
    assert!(!page.contains("NA-300-VP"), "{page}");
    let _ = autre;

    // Rien : une question vide invite, une question sans réponse le dit.
    let page = texte(get(&app, "/recherche", Some(&cookie)).await).await;
    assert!(page.contains("Tapez une immatriculation"), "{page}");
    let page = texte(get(&app, "/recherche?q=+zzz+", Some(&cookie)).await).await;
    assert!(page.contains("Rien ne correspond à « zzz »."), "{page}");

    // `%` et `_` sont cherchés tels quels, pas comme jokers.
    let page = texte(get(&app, "/recherche?q=%25", Some(&cookie)).await).await;
    assert!(page.contains("Rien ne correspond à « % »."), "{page}");
    let page = texte(get(&app, "/recherche?q=_", Some(&cookie)).await).await;
    assert!(page.contains("Rien ne correspond à « _ »."), "{page}");

    // Sans session, pas de recherche.
    let reponse = get(&app, "/recherche?q=na", None).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
}

/// La liste des conducteurs porte les initiales ; la feuille de style et le script portent la
/// navigation.
#[sqlx::test]
async fn les_initiales_et_la_feuille_de_style_suivent(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let conducteur = creer_conducteur(&pool, "Navarro").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/conducteurs", Some(&cookie)).await).await;
    assert!(
        page.contains(&format!(
            r#"<span class="initiales" aria-hidden="true">CN</span><a href="/conducteurs/{conducteur}">Navarro</a>"#
        )),
        "{page}"
    );
    let page = texte(get(&app, &format!("/conducteurs/{conducteur}"), Some(&cookie)).await).await;
    assert!(
        page.contains(
            r#"<h1><span class="initiales" aria-hidden="true">CN</span>Camille Navarro</h1>"#
        ),
        "{page}"
    );

    let style = include_str!("../static/style.css");
    let script = include_str!("../static/app.js");
    for attendu in [
        "--ico-voiture:",
        "--ico-calendrier:",
        "--ico-cle:",
        "--ico-plus:",
        "--ico-tableau:",
        "--ico-loupe:",
        ".laterale",
        ".onglets",
        ".menu-ouvert",
        ".initiales",
        "[data-ico=",
    ] {
        assert!(style.contains(attendu), "style.css : {attendu}");
    }
    assert!(script.contains("[data-menu]") && script.contains("menu-ouvert"));
    assert!(script.contains("[data-raccourci]"));
}
