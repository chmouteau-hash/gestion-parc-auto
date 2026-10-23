//! La contre-visite (#107, blocs 2 et 3) : un contrôle technique défavorable crée d'office
//! l'échéance « Contre-visite », à deux mois, qui remplace le « Contrôle technique » par le calcul ;
//! la contre-visite favorable les solde et propose le prochain contrôle, deux ans après le
//! contrôle initial ; défavorable ou trop tardive, un contrôle complet est à refaire.

mod commun;

use axum::http::{header, StatusCode};
use axum::response::Response;
use chrono::{Duration, Local, Months, NaiveDate};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::erreurs::ErreurApp;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    se_connecter, texte,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn dans(jours: i64) -> NaiveDate {
    aujourd_hui() + Duration::days(jours)
}

fn plus_mois(date: NaiveDate, mois: u32) -> NaiveDate {
    date.checked_add_months(Months::new(mois)).unwrap()
}

fn saisie(vehicule: i64, type_echeance: &str, date: NaiveDate) -> SaisieEcheance {
    SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: type_echeance.into(),
        date_limite: date.to_string(),
        ..Default::default()
    }
}

async fn echeance(pool: &PgPool, vehicule: i64, type_echeance: &str, date: NaiveDate) -> i64 {
    echeances::creer(pool, &saisie(vehicule, type_echeance, date))
        .await
        .unwrap()
        .id
}

/// Les échéances du véhicule : (type, date limite, statut), dans l'ordre de leur création.
async fn du_vehicule(pool: &PgPool, vehicule: i64) -> Vec<(String, NaiveDate, String)> {
    let mut liste = echeances::lister_par_vehicule(pool, vehicule)
        .await
        .unwrap();
    liste.sort_by_key(|e| e.id);
    liste
        .into_iter()
        .map(|e| (e.type_echeance, e.date_limite, e.statut))
        .collect()
}

/// Le formulaire « Nouvelle intervention » tel que le navigateur l'envoie (le véhicule de test
/// est à 12 000 km).
fn corps(vehicule: i64, type_intervention: &str, resultat: &str, date: NaiveDate) -> String {
    format!(
        "vehicule_id={vehicule}&type_intervention={type_intervention}&resultat_controle={resultat}\
         &date_intervention={date}&kilometrage=12000&cout=&prestataire=&notes="
    )
}

async fn saisir(
    app: &axum::Router,
    cookie: &str,
    vehicule: i64,
    type_intervention: &str,
    resultat: &str,
    date: NaiveDate,
) -> Response {
    let reponse = post_formulaire(
        app,
        "/interventions",
        Some(cookie),
        &corps(vehicule, type_intervention, resultat, date),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    reponse
}

fn vers(reponse: &Response) -> String {
    reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string()
}

/// Le code du message posé par la redirection (cookie `fait`).
fn fait(reponse: &Response) -> String {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .find(|c| c.starts_with("fait="))
        .unwrap_or_default()
}

#[sqlx::test]
async fn un_controle_defavorable_cree_la_contre_visite_qui_remplace_le_controle(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-100-AN").await;
    echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let controle = dans(-1);
    let reponse = saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        controle,
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert!(fait(&reponse).starts_with("fait=intervention-contre-visite;"));
    assert_eq!(
        du_vehicule(&pool, vehicule).await,
        vec![
            ("controle_technique".into(), dans(20), "ouverte".into()),
            (
                "contre_visite".into(),
                plus_mois(controle, 2),
                "ouverte".into()
            ),
        ]
    );

    // Le contrôle reste ouvert, remplacé par la contre-visite : sur la fiche, pas dans les alertes.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains("Remplacée par la contre-visite"), "{fiche}");
    assert!(fiche.contains("Contre-visite"), "{fiche}");
    let alertes = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(!alertes.contains("CV-100-AN"), "{alertes}");

    // La carte d'un particulier montre la contre-visite.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_p = se_connecter(&particulier, "gestion").await;
    let accueil = texte(get(&particulier, "/", Some(&cookie_p)).await).await;
    assert!(
        accueil.contains(&format!(
            "Contre-visite le {}",
            plus_mois(controle, 2).format("%d/%m/%Y")
        )),
        "{accueil}"
    );
}

#[sqlx::test]
async fn la_contre_visite_d_un_controle_critique_dit_circulation_interdite(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-110-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Il y a quarante jours : la contre-visite tombe dans les trente jours des alertes.
    let controle = dans(-40);
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "critique",
        controle,
    )
    .await;
    let alertes = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(alertes.contains("Contre-visite"), "{alertes}");
    assert!(
        alertes.contains(&format!(
            "circulation interdite depuis le contrôle du {}",
            controle.format("%d/%m/%Y")
        )),
        "{alertes}"
    );
}

#[sqlx::test]
async fn une_contre_visite_favorable_solde_et_le_controle_suivant_part_du_controle_initial(
    pool: PgPool,
) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-120-AN").await;
    echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let initial = dans(-10);
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        initial,
    )
    .await;
    let reponse = saisir(
        &app,
        &cookie,
        vehicule,
        "contre_visite",
        "favorable",
        dans(-1),
    )
    .await;
    assert_eq!(
        vers(&reponse),
        format!(
            "/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=controle_technique&date_limite={}&retour=vehicule",
            plus_mois(initial, 24)
        )
    );
    assert!(fait(&reponse).starts_with("fait=intervention-solde-cv;"));
    assert_eq!(
        du_vehicule(&pool, vehicule).await,
        vec![
            ("controle_technique".into(), dans(20), "traitee".into()),
            (
                "contre_visite".into(),
                plus_mois(initial, 2),
                "traitee".into()
            ),
        ]
    );
}

#[sqlx::test]
async fn une_contre_visite_defavorable_ou_tardive_demande_un_controle_complet(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Défavorable : la contre-visite est soldée, le contrôle reste ouvert et revient à l'écran.
    let vehicule = creer_vehicule(&pool, "CV-130-AN").await;
    echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-10),
    )
    .await;
    let reponse = saisir(
        &app,
        &cookie,
        vehicule,
        "contre_visite",
        "defavorable",
        dans(-1),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert!(fait(&reponse).starts_with("fait=intervention-controle-a-refaire;"));
    let echeances_du_vehicule = du_vehicule(&pool, vehicule).await;
    assert_eq!(echeances_du_vehicule[0].2, "ouverte");
    assert_eq!(echeances_du_vehicule[1].2, "traitee");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("Remplacée par la contre-visite"), "{fiche}");

    // Favorable, mais après les deux mois : même chose.
    let autre = creer_vehicule(&pool, "CV-131-AN").await;
    echeance(&pool, autre, "contre_visite", dans(-5)).await;
    let reponse = saisir(
        &app,
        &cookie,
        autre,
        "contre_visite",
        "favorable",
        aujourd_hui(),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{autre}"));
    assert!(fait(&reponse).starts_with("fait=intervention-controle-a-refaire;"));
    assert_eq!(du_vehicule(&pool, autre).await[0].2, "traitee");
}

#[sqlx::test]
async fn marquer_traitee_ne_propose_rien_ni_pour_la_contre_visite_ni_pour_le_controle_remplace(
    pool: PgPool,
) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-140-AN").await;
    let controle = echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-1),
    )
    .await;
    let contre_visite = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.type_echeance == "contre_visite")
        .unwrap()
        .id;

    for id in [controle, contre_visite] {
        let reponse =
            post_formulaire(&app, &format!("/echeances/{id}/traiter"), Some(&cookie), "").await;
        assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
        assert!(
            !vers(&reponse).contains("/echeances/nouvelle"),
            "{}",
            vers(&reponse)
        );
    }
}

#[sqlx::test]
async fn une_seule_contre_visite_a_la_fois(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-150-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Un second contrôle défavorable remplace la contre-visite du premier.
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-10),
    )
    .await;
    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-1),
    )
    .await;
    assert_eq!(
        du_vehicule(&pool, vehicule).await,
        vec![
            (
                "contre_visite".into(),
                plus_mois(dans(-10), 2),
                "traitee".into()
            ),
            (
                "contre_visite".into(),
                plus_mois(dans(-1), 2),
                "ouverte".into()
            ),
        ]
    );
    // En ajouter une à la main est refusé, avec le motif.
    match echeances::creer(&pool, &saisie(vehicule, "contre_visite", dans(30))).await {
        Err(ErreurApp::Metier(message)) => {
            assert!(
                message.contains("déjà une contre-visite à faire"),
                "{message}"
            )
        }
        autre => panic!("refus attendu : {autre:?}"),
    }
}

#[sqlx::test]
async fn la_saisie_du_passe_ne_cree_pas_de_contre_visite(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-160-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Il y a trois mois : le délai de la contre-visite est déjà passé.
    let reponse = saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-90),
    )
    .await;
    assert!(fait(&reponse).starts_with("fait=intervention-creee;"));
    assert!(du_vehicule(&pool, vehicule).await.is_empty());
}

#[sqlx::test]
async fn un_controle_favorable_solde_aussi_la_contre_visite_ouverte(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "CV-170-AN").await;
    echeance(&pool, vehicule, "controle_technique", dans(20)).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "defavorable",
        dans(-10),
    )
    .await;
    // Plutôt qu'une contre-visite, un contrôle complet, favorable : il solde tout.
    let visite = dans(-1);
    let reponse = saisir(
        &app,
        &cookie,
        vehicule,
        "controle_technique",
        "favorable",
        visite,
    )
    .await;
    assert!(fait(&reponse).starts_with("fait=intervention-solde-ct;"));
    assert!(
        vers(&reponse).contains(&format!("&date_limite={}&", plus_mois(visite, 24))),
        "{}",
        vers(&reponse)
    );
    assert!(du_vehicule(&pool, vehicule)
        .await
        .iter()
        .all(|(_, _, statut)| statut == "traitee"));
}
