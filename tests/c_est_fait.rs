//! « C'est fait » sur une opération du plan d'entretien (#94, bloc 3) : depuis la fiche, la page
//! Alertes et l'accueil, il ouvre le formulaire d'intervention pré-rempli, l'opération cochée ;
//! enregistrer suffit, la prochaine se recalcule.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local, NaiveDate};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::plan_entretien::{self, SaisiePlan};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    se_connecter, texte,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

/// Le véhicule de test est à 12 000 km. Une vidange faite il y a 60 jours à 11 000 km, tous les
/// 1 800 km : la prochaine est à 12 800 km, dans 800 km, donc à venir. Rend le véhicule.
async fn vidange_a_venir(pool: &PgPool, immatriculation: &str) -> i64 {
    let vehicule = creer_vehicule(pool, immatriculation).await;
    interventions::creer(
        pool,
        &SaisieIntervention {
            vehicule_id: vehicule.to_string(),
            type_intervention: "revision".into(),
            date_intervention: (aujourd_hui() - Duration::days(60)).to_string(),
            kilometrage: "11000".into(),
            lignes: vec![SaisieLigne {
                operation: "vidange".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    plan_entretien::creer(
        pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "vidange".into(),
            periodicite_km: "1800".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    vehicule
}

/// Le texte de la page, les `&` décodés : Askama les écrit `&#38;`, les gabarits `&amp;`, et le
/// navigateur lit les deux comme `&`. On compare ainsi les adresses, pas leur écriture.
fn decoder(page: String) -> String {
    page.replace("&#38;", "&").replace("&amp;", "&")
}

/// L'adresse de « C'est fait », décodée.
fn lien(vehicule: i64, type_intervention: &str, operation: &str, retour: &str) -> String {
    format!(
        r#"href="/interventions/nouvelle?vehicule_id={vehicule}&kilometrage=12000&type_intervention={type_intervention}&operation={operation}&retour={retour}">C'est fait</a>"#
    )
}

#[sqlx::test]
async fn c_est_fait_se_propose_partout_ou_l_operation_se_voit(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = vidange_a_venir(&pool, "CF-100-AN").await;
    // Un contrôle, jamais noté : « C'est fait » le note en « Entretien courant ».
    plan_entretien::creer(
        &pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: "pression_pneus".into(),
            periodicite_mois: "1".into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La fiche : sur chaque ligne du plan, qu'elle presse ou non.
    let fiche =
        decoder(texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await);
    for attendu in [
        lien(vehicule, "revision", "vidange", "vehicule"),
        lien(vehicule, "entretien_courant", "pression_pneus", "vehicule"),
    ] {
        assert!(fiche.contains(&attendu), "{attendu} absent\n{fiche}");
    }

    // La page Alertes, à côté de « Ouvrir la fiche ».
    let page = decoder(texte(get(&app, "/alertes", Some(&cookie)).await).await);
    assert!(
        page.contains(&lien(vehicule, "revision", "vidange", "alertes")),
        "{page}"
    );

    // Le tableau de bord d'une entreprise : à la place de « Ouvrir la fiche ».
    let accueil = decoder(texte(get(&app, "/", Some(&cookie)).await).await);
    assert!(
        accueil.contains(&lien(vehicule, "revision", "vidange", "accueil")),
        "{accueil}"
    );
    assert!(!accueil.contains("Ouvrir la fiche"), "{accueil}");

    // La carte d'un particulier.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_p = se_connecter(&particulier, "gestion").await;
    let accueil = decoder(texte(get(&particulier, "/", Some(&cookie_p)).await).await);
    assert!(
        accueil.contains(&lien(vehicule, "revision", "vidange", "accueil")),
        "{accueil}"
    );

    // Le lecteur voit l'opération, sans le geste.
    let cookie_l = se_connecter(&app, "lecture").await;
    for chemin in [
        format!("/vehicules/{vehicule}"),
        "/alertes".into(),
        "/".into(),
    ] {
        let page = texte(get(&app, &chemin, Some(&cookie_l)).await).await;
        assert!(page.contains("Vidange"), "{chemin}\n{page}");
        assert!(!page.contains("C'est fait"), "{chemin}\n{page}");
    }
}

#[sqlx::test]
async fn le_formulaire_arrive_pre_rempli_et_annuler_ramene_d_ou_l_on_vient(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = vidange_a_venir(&pool, "CF-110-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let page = texte(
        get(
            &app,
            &format!(
                "/interventions/nouvelle?vehicule_id={vehicule}&kilometrage=12000\
                 &type_intervention=revision&operation=vidange&retour=alertes"
            ),
            Some(&cookie),
        )
        .await,
    )
    .await;
    for attendu in [
        format!(r#"<option value="{vehicule}" data-kilometrage="12000" selected>"#),
        r#"<option value="revision" selected>"#.to_string(),
        format!(
            r#"name="date_intervention" value="{}" required"#,
            aujourd_hui()
        ),
        r#"required data-prerempli="12000">"#.to_string(),
        r#"name="operation" value="vidange" checked>"#.to_string(),
        r#"action="/interventions?retour=alertes""#.to_string(),
        r#"<a href="/alertes" class="bouton-discret">Annuler</a>"#.to_string(),
    ] {
        assert!(page.contains(&attendu), "{attendu} absent\n{page}");
    }
    assert_eq!(page.matches(" checked>").count(), 1, "{page}");

    // Depuis la fiche, « Annuler » y ramène, au plan ; depuis l'accueil, à l'accueil.
    for (retour, annuler) in [
        ("vehicule", format!("/vehicules/{vehicule}#plan")),
        ("accueil", "/".to_string()),
    ] {
        let page = texte(
            get(
                &app,
                &format!(
                    "/interventions/nouvelle?vehicule_id={vehicule}&operation=vidange&retour={retour}"
                ),
                Some(&cookie),
            )
            .await,
        )
        .await;
        assert!(
            page.contains(&format!(
                r#"<a href="{annuler}" class="bouton-discret">Annuler</a>"#
            )),
            "{retour}\n{page}"
        );
    }

    // Une opération inconnue ne coche rien ; un `retour` inconnu ramène à la liste : l'adresse ne
    // porte qu'un mot, jamais une destination.
    let page = texte(
        get(
            &app,
            "/interventions/nouvelle?operation=inconnue&retour=https://ailleurs.example",
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(!page.contains(" checked>"), "{page}");
    assert!(page.contains(r#"action="/interventions""#), "{page}");
    assert!(
        page.contains(r#"<a href="/interventions" class="bouton-discret">Annuler</a>"#),
        "{page}"
    );
    assert!(!page.contains("ailleurs"), "{page}");
}

#[sqlx::test]
async fn enregistrer_suffit_la_prochaine_se_recalcule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = vidange_a_venir(&pool, "CF-120-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("<td>Vidange</td>"), "{page}");

    // Le formulaire tel que « C'est fait » l'a rempli, envoyé tel quel.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention={}\
         &kilometrage=12000&operation=vidange&cout=&prestataire=&notes=",
        aujourd_hui()
    );
    let reponse =
        post_formulaire(&app, "/interventions?retour=alertes", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let vers = reponse.headers()[header::LOCATION].to_str().unwrap();
    assert!(
        vers.starts_with(&format!("/vehicules/{vehicule}")),
        "{vers}"
    );

    // La prochaine part de cette vidange : 13 800 km, dans 1 800 km ; plus d'alerte.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains("13\u{202F}800 km · dans 1\u{202F}800 km"),
        "{fiche}"
    );
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Aucune alerte"), "{page}");
}

#[sqlx::test]
async fn un_avertissement_ou_un_refus_garde_l_origine(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = vidange_a_venir(&pool, "CF-130-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Daté d'avant la vidange d'il y a 60 jours, avec un kilométrage plus haut : la règle (a)
    // avertit, et le formulaire garde d'où l'on vient.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention={}\
         &kilometrage=11500&operation=vidange&cout=&prestataire=&notes=",
        aujourd_hui() - Duration::days(90)
    );
    let reponse =
        post_formulaire(&app, "/interventions?retour=accueil", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = decoder(texte(reponse).await);
    for attendu in [
        "<title>Attention : Nouvelle intervention",
        r#"action="/interventions?retour=accueil""#,
        r#"formaction="/interventions?retour=accueil&kilometrage_confirme=1">Enregistrer quand même</button>"#,
        r#"<a href="/" class="bouton-discret">Annuler</a>"#,
        r#"name="operation" value="vidange" checked>"#,
    ] {
        assert!(page.contains(attendu), "{attendu} absent\n{page}");
    }
    let reponse = post_formulaire(
        &app,
        "/interventions?retour=accueil&kilometrage_confirme=1",
        Some(&cookie),
        &corps,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    // Un refus (pas de type) : même chose.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=&date_intervention={}\
         &kilometrage=12000&operation=vidange&cout=&prestataire=&notes=",
        aujourd_hui()
    );
    let reponse = post_formulaire(
        &app,
        "/interventions?retour=vehicule",
        Some(&cookie),
        &corps,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("<title>Erreur : Nouvelle intervention"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"<a href="/vehicules/{vehicule}#plan" class="bouton-discret">Annuler</a>"#
        )),
        "{page}"
    );
    assert!(
        page.contains(r#"action="/interventions?retour=vehicule""#),
        "{page}"
    );
}
