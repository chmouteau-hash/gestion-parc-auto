//! L'échéance « À faire » (#98) : noter ce qu'il y a à remplacer ou à faire sur un véhicule, avec
//! sa date limite. Sur base réelle : le formulaire, le libellé et le constat partout où une
//! échéance s'affiche, les refus, l'unicité, l'absence de « suivante », l'API et l'export.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::erreurs::ErreurApp;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    requete_json, se_connecter, texte,
};

/// Dans une page, l'apostrophe de « Câble d'embrayage » est écrite `&#39;`.
const CABLE_DANS_LA_PAGE: &str = "À faire : Câble d&#39;embrayage";

fn dans_cinq_jours() -> String {
    (Local::now().date_naive() + Duration::days(5)).to_string()
}

fn a_faire(vehicule: i64, operation: &str, libelle: &str, date: &str) -> SaisieEcheance {
    SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: "a_faire".to_string(),
        date_limite: date.to_string(),
        operation: operation.to_string(),
        libelle: libelle.to_string(),
        ..Default::default()
    }
}

fn message_du_refus<T: std::fmt::Debug>(resultat: Result<T, ErreurApp>) -> String {
    match resultat {
        Err(ErreurApp::Metier(message)) => message,
        autre => panic!("un refus métier était attendu, reçu : {autre:?}"),
    }
}

#[sqlx::test]
async fn une_chose_a_faire_se_note_et_se_retrouve_partout(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-100-AA").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le bouton de la fiche ouvre le formulaire sur le bon type, la date proposée dans un mois.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(
            r#"href="/echeances/nouvelle?vehicule_id={vehicule}&amp;type_echeance=a_faire""#
        )),
        "{fiche}"
    );
    let page = texte(
        get(
            &app,
            &format!("/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=a_faire"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(page.contains(r#"<div data-pour-type="a_faire">"#), "{page}");
    assert!(page.contains(r#"name="operation""#), "{page}");
    assert!(
        page.contains(r#"<option value="a_faire" selected>À faire</option>"#),
        "{page}"
    );
    let dans_un_mois = Local::now()
        .date_naive()
        .checked_add_months(chrono::Months::new(1))
        .unwrap();
    assert!(
        page.contains(&format!(r#"name="date_limite" value="{dans_un_mois}""#)),
        "{page}"
    );

    // Le câble d'embrayage n'est pas dans la liste : « Autre », avec son libellé et un constat.
    let corps = format!(
        "vehicule_id={vehicule}&type_echeance=a_faire&operation=autre&libelle=C%C3%A2ble+d%27embrayage&constat=c%C3%A2ble+qui+accroche&date_limite={}&statut=ouverte",
        dans_cinq_jours()
    );
    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}").as_str()
    );

    // La fiche, la liste, les alertes : le libellé « À faire : … » et le constat dessous.
    for chemin in [
        format!("/vehicules/{vehicule}"),
        "/echeances".to_string(),
        "/alertes".to_string(),
    ] {
        let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
        assert!(page.contains(CABLE_DANS_LA_PAGE), "{chemin}\n{page}");
        assert!(
            page.contains("<br><small>câble qui accroche</small>"),
            "{chemin}\n{page}"
        );
    }

    // L'export CSV : le type en toutes lettres, et le constat dans sa colonne.
    let csv = texte(get(&app, "/echeances.csv", Some(&cookie)).await).await;
    assert!(
        csv.starts_with("\u{FEFF}Véhicule;Type;Date limite;Statut;Constat\r\n"),
        "{csv}"
    );
    assert!(
        csv.contains("AF-100-AA;À faire : Câble d'embrayage;"),
        "{csv}"
    );
    assert!(csv.contains(";câble qui accroche\r\n"), "{csv}");

    // Le formulaire de modification rend ce qui a été saisi.
    let id = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()[0]
        .id;
    let page = texte(get(&app, &format!("/echeances/{id}/modifier"), Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<option value="autre" selected>Autre</option>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"name="libelle" value="Câble d&#39;embrayage""#),
        "{page}"
    );
    assert!(
        page.contains(r#"name="constat" value="câble qui accroche""#),
        "{page}"
    );
}

#[sqlx::test]
async fn sur_l_accueil_la_chose_a_faire_dit_ce_qu_il_y_a_a_faire(pool: PgPool) {
    creer_utilisateur(&pool, "christophe", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-200-AA").await;
    echeances::creer(&pool, &a_faire(vehicule, "balais", "", &dans_cinq_jours()))
        .await
        .unwrap();
    let app = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&app, "christophe").await;
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(
        page.contains("À faire : Balais d&#39;essuie-glace"),
        "{page}"
    );
}

#[sqlx::test]
async fn une_chose_a_faire_n_a_pas_de_suivante(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-300-AA").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // « Marquer traitée » revient d'où l'on vient, sans formulaire de la prochaine.
    let origines = [
        ("vehicule", format!("/vehicules/{vehicule}")),
        ("liste", "/echeances".to_string()),
        ("alertes", "/alertes".to_string()),
        ("accueil", "/".to_string()),
    ];
    for (rang, (retour, vers)) in origines.into_iter().enumerate() {
        let date = format!("2031-01-0{}", rang + 1);
        let e = echeances::creer(&pool, &a_faire(vehicule, "bougies", "", &date))
            .await
            .unwrap();
        let reponse = post_formulaire(
            &app,
            &format!("/echeances/{}/traiter?retour={retour}", e.id),
            Some(&cookie),
            "",
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER, "{retour}");
        assert_eq!(
            reponse.headers()[header::LOCATION],
            vers.as_str(),
            "{retour}"
        );

        let relue = echeances::obtenir(&pool, e.id).await.unwrap();
        assert!(!relue.est_ouverte(), "{retour} : traitée");
        assert!(
            echeances::prochaine(&relue, Local::now().date_naive()).is_none(),
            "{retour} : pas de suivante"
        );
    }
}

#[sqlx::test]
async fn ce_qui_n_a_pas_de_sens_est_refuse_avec_le_motif(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-400-AA").await;
    let date = dans_cinq_jours();

    // Un « à faire » sans rien à faire.
    let message =
        message_du_refus(echeances::creer(&pool, &a_faire(vehicule, "", "", &date)).await);
    assert!(
        message.contains("Choisissez ce qu'il y a à faire"),
        "{message}"
    );

    // « Autre » sans libellé.
    let message =
        message_du_refus(echeances::creer(&pool, &a_faire(vehicule, "autre", "  ", &date)).await);
    assert!(
        message.contains("préciser ce qu'il y a à faire"),
        "{message}"
    );

    // Un libellé avec une opération qui n'est pas « autre ».
    let message = message_du_refus(
        echeances::creer(&pool, &a_faire(vehicule, "vidange", "Quelque chose", &date)).await,
    );
    assert!(message.contains("ne va qu'avec « Autre »"), "{message}");

    // Une opération hors du vocabulaire.
    let message = message_du_refus(
        echeances::creer(&pool, &a_faire(vehicule, "teleportation", "", &date)).await,
    );
    assert!(message.contains("teleportation"), "{message}");

    // Une opération, un libellé ou un constat sur un autre type.
    for saisie in [
        SaisieEcheance {
            type_echeance: "assurance".into(),
            operation: "vidange".into(),
            ..a_faire(vehicule, "", "", &date)
        },
        SaisieEcheance {
            type_echeance: "controle_technique".into(),
            constat: "voyant allumé".into(),
            ..a_faire(vehicule, "", "", &date)
        },
    ] {
        let message = message_du_refus(echeances::creer(&pool, &saisie).await);
        assert!(
            message.contains("ne vont qu'avec le type « À faire »"),
            "{message}"
        );
    }

    // Par le formulaire : le même refus, la saisie n'est pas perdue.
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let corps = format!(
        "vehicule_id={vehicule}&type_echeance=a_faire&operation=autre&libelle=&constat=voyant+allum%C3%A9&date_limite={date}&statut=ouverte"
    );
    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("préciser ce qu&#39;il y a à faire"), "{page}");
    assert!(
        page.contains(r#"name="constat" value="voyant allumé""#),
        "{page}"
    );

    // Et rien n'a été enregistré.
    assert!(echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .is_empty());
}

#[sqlx::test]
async fn deux_choses_differentes_a_la_meme_date_sont_deux_echeances(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AF-500-AA").await;
    let date = dans_cinq_jours();

    echeances::creer(&pool, &a_faire(vehicule, "balais", "", &date))
        .await
        .expect("balais");
    echeances::creer(&pool, &a_faire(vehicule, "plaquettes", "", &date))
        .await
        .expect("plaquettes, même date : une autre chose");
    echeances::creer(
        &pool,
        &a_faire(vehicule, "autre", "Câble d'embrayage", &date),
    )
    .await
    .expect("câble d'embrayage");
    echeances::creer(&pool, &a_faire(vehicule, "autre", "Chaîne", &date))
        .await
        .expect("chaîne : un autre libellé");

    // La même chose à la même date : une double saisie, refusée en nommant la chose et la date.
    let message =
        message_du_refus(echeances::creer(&pool, &a_faire(vehicule, "balais", "", &date)).await);
    assert!(
        message.contains("À faire : Balais d'essuie-glace"),
        "{message}"
    );
    assert!(message.contains("pas encore traitée"), "{message}");
    let message = message_du_refus(
        echeances::creer(
            &pool,
            &a_faire(vehicule, "autre", "Câble d'embrayage", &date),
        )
        .await,
    );
    assert!(message.contains("À faire : Câble d'embrayage"), "{message}");

    // Un type d'avant garde sa règle : même type, même date, refusé.
    let ct = SaisieEcheance {
        type_echeance: "controle_technique".into(),
        ..a_faire(vehicule, "", "", "2029-05-05")
    };
    echeances::creer(&pool, &ct)
        .await
        .expect("contrôle technique");
    let message = message_du_refus(echeances::creer(&pool, &ct).await);
    assert!(message.contains("« Contrôle technique »"), "{message}");
}

#[sqlx::test]
async fn on_change_une_chose_a_faire_en_un_autre_type_sans_rien_garder(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AF-600-AA").await;
    let e = echeances::creer(&pool, &a_faire(vehicule, "balais", "", &dans_cinq_jours()))
        .await
        .unwrap();
    assert!(e.est_a_faire());
    assert_eq!(e.type_libelle(), "À faire : Balais d'essuie-glace");

    // Le type change et la pièce reste : refusé, sans rien écrire.
    let mut saisie = SaisieEcheance::depuis(&e);
    saisie.type_echeance = "assurance".into();
    let message = message_du_refus(echeances::modifier(&pool, e.id, &saisie).await);
    assert!(
        message.contains("ne vont qu'avec le type « À faire »"),
        "{message}"
    );
    assert_eq!(
        echeances::obtenir(&pool, e.id).await.unwrap().type_echeance,
        "a_faire"
    );

    // La pièce est vidée en même temps : accepté.
    saisie.operation.clear();
    let relue = echeances::modifier(&pool, e.id, &saisie).await.unwrap();
    assert_eq!(relue.type_libelle(), "Assurance");
    assert!(relue.operation.is_empty() && relue.libelle.is_empty() && relue.constat.is_empty());
}

#[sqlx::test]
async fn la_base_tient_les_regles_meme_sans_l_application(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "AF-700-AA").await;
    for (type_echeance, operation, libelle, constat, contrainte) in [
        ("a_faire", "", "", "", "echeances_a_faire_precise"),
        ("assurance", "vidange", "", "", "echeances_a_faire_precise"),
        (
            "assurance",
            "",
            "",
            "un constat",
            "echeances_constat_seulement_a_faire",
        ),
        ("a_faire", "autre", "", "", "echeances_autre_precisee"),
        (
            "a_faire",
            "vidange",
            "Un libellé",
            "",
            "echeances_libelle_seulement_autre",
        ),
    ] {
        let erreur = sqlx::query(
            "INSERT INTO echeances (vehicule_id, type_echeance, date_limite, operation, libelle, constat)
             VALUES ($1, $2, '2030-01-01', $3, $4, $5)",
        )
        .bind(vehicule)
        .bind(type_echeance)
        .bind(operation)
        .bind(libelle)
        .bind(constat)
        .execute(&pool)
        .await
        .expect_err("la base doit refuser");
        assert!(
            erreur.to_string().contains(contrainte),
            "{contrainte} : {erreur}"
        );
    }
}

#[sqlx::test]
async fn l_api_lit_et_ecrit_ce_qu_il_y_a_a_faire(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AF-800-AA").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let json = format!(
        r#"{{"vehicule_id":"{vehicule}","type_echeance":"a_faire","date_limite":"{}","operation":"autre","libelle":"Câble d'embrayage","constat":"câble qui accroche"}}"#,
        dans_cinq_jours()
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/echeances",
        Some(&cookie),
        Some(&json),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let corps: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(corps["operation"], "autre");
    assert_eq!(corps["libelle"], "Câble d'embrayage");
    assert_eq!(corps["constat"], "câble qui accroche");

    // Un refus rend le motif en 400.
    let sans_libelle = format!(
        r#"{{"vehicule_id":"{vehicule}","type_echeance":"a_faire","date_limite":"2031-01-01","operation":"autre"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/echeances",
        Some(&cookie),
        Some(&sans_libelle),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);

    // Les alertes de l'API disent le motif en toutes lettres, avec le constat.
    let reponse = requete_json(&app, "GET", "/api/v1/alertes", Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let alertes = texte(reponse).await;
    assert!(alertes.contains("À faire : Câble d'embrayage"), "{alertes}");
    assert!(
        alertes.contains(r#""constat":"câble qui accroche""#),
        "{alertes}"
    );
}
