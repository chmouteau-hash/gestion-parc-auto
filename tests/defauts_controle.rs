//! Les défauts relevés au contrôle technique (#107, bloc 4) : sur la fiche, la contre-visite
//! ouverte propose « Noter un défaut à corriger », qui ouvre une chose à faire pré-remplie pour sa
//! date limite. L'intervention qui contient la même opération solde la chose à faire, comme toute
//! chose à faire notée avant elle (#98).

mod commun;

use axum::http::{header, StatusCode};
use axum::response::Response;
use chrono::{Duration, Local, Months, NaiveDate};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, requete_json,
    se_connecter, texte,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn dans(jours: i64) -> NaiveDate {
    aujourd_hui() + Duration::days(jours)
}

/// Askama écrit `&` en `&#38;` ou `&amp;` selon l'endroit : les adresses se comparent décodées.
fn decoder(page: &str) -> String {
    page.replace("&#38;", "&").replace("&amp;", "&")
}

/// Une chose à faire du véhicule, notée maintenant, à faire dans un mois.
async fn a_faire(pool: &PgPool, vehicule: i64, operation: &str, libelle: &str) -> i64 {
    echeances::creer(
        pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: "a_faire".into(),
            date_limite: dans(30).to_string(),
            operation: operation.into(),
            libelle: libelle.into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

async fn statut(pool: &PgPool, id: i64) -> String {
    echeances::obtenir(pool, id).await.unwrap().statut
}

/// Le formulaire « Nouvelle intervention » tel que le navigateur l'envoie, avec ses opérations
/// cochées et leur libellé (le véhicule de test est à 12 000 km).
fn corps(
    vehicule: i64,
    type_intervention: &str,
    resultat: &str,
    date: NaiveDate,
    operations: &[(&str, &str)],
) -> String {
    let mut corps = format!(
        "vehicule_id={vehicule}&type_intervention={type_intervention}&resultat_controle={resultat}&date_intervention={date}&kilometrage=12000&cout=&prestataire=&notes="
    );
    for (operation, libelle) in operations {
        corps.push_str(&format!(
            "&operation={operation}&libelle_{operation}={libelle}"
        ));
    }
    corps
}

async fn saisir(app: &axum::Router, cookie: &str, corps: &str) -> Response {
    let reponse = post_formulaire(app, "/interventions", Some(cookie), corps).await;
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

/// L'adresse de « Noter un défaut à corriger » sur la fiche, décodée.
fn lien_du_defaut(page: &str) -> String {
    let page = decoder(page);
    let fin = page
        .find(r#"">Noter un défaut à corriger</a>"#)
        .expect("le lien est sur la fiche");
    let debut = page[..fin].rfind(r#"href=""#).unwrap() + r#"href=""#.len();
    page[debut..fin].to_string()
}

async fn fiche(app: &axum::Router, vehicule: i64, cookie: &str) -> String {
    texte(get(app, &format!("/vehicules/{vehicule}"), Some(cookie)).await).await
}

#[sqlx::test]
async fn un_defaut_releve_au_controle_se_note_depuis_la_contre_visite_puis_se_solde(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "DC-100-AN").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Sans contre-visite, rien à proposer.
    assert!(!fiche(&app, vehicule, &cookie)
        .await
        .contains("Noter un défaut à corriger"));

    let controle = dans(-1);
    let reponse = saisir(
        &app,
        &cookie,
        &corps(vehicule, "controle_technique", "defavorable", controle, &[]),
    )
    .await;
    assert!(fait(&reponse).starts_with("fait=intervention-contre-visite;"));
    let limite = controle.checked_add_months(Months::new(2)).unwrap();

    // La ligne de la contre-visite propose le défaut, pour sa date limite, avec le contrôle nommé.
    let lien = lien_du_defaut(&fiche(&app, vehicule, &cookie).await);
    assert!(
        lien.starts_with(&format!(
            "/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=a_faire&date_limite={limite}&constat="
        )),
        "{lien}"
    );
    assert!(lien.ends_with("&retour=vehicule"), "{lien}");
    // Le constat voyage encodé dans l'adresse ; le formulaire le reçoit en clair.
    let constat = format!(
        "Défaut relevé au contrôle technique du {}",
        controle.format("%d/%m/%Y")
    );
    let reponse = get(&app, &lien, Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let formulaire = texte(reponse).await;
    assert!(formulaire.contains(&constat), "{formulaire}");
    assert!(
        formulaire.contains(&format!(r#"value="{limite}""#)),
        "{formulaire}"
    );

    // Le lecteur voit la contre-visite, pas le lien.
    let lecteur = se_connecter(&app, "lecture").await;
    let page = fiche(&app, vehicule, &lecteur).await;
    assert!(page.contains("Contre-visite"));
    assert!(!page.contains("Noter un défaut à corriger"));

    // On choisit la pièce et on enregistre, comme le navigateur.
    let reponse = post_formulaire(
        &app,
        "/echeances?retour=vehicule",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&type_echeance=a_faire&operation=plaquettes&libelle=&constat={}&date_limite={limite}&statut=ouverte",
            constat.replace(' ', "+").replace('é', "%C3%A9").replace('ô', "%C3%B4").replace('/', "%2F")
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let notee = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.type_echeance == "a_faire")
        .expect("la chose à faire est enregistrée");
    assert_eq!(notee.operation, "plaquettes");
    assert_eq!(notee.constat, constat);
    assert_eq!(notee.date_limite, limite);

    // La réparation qui change les plaquettes la solde ; la contre-visite reste à faire.
    let reponse = saisir(
        &app,
        &cookie,
        &corps(
            vehicule,
            "reparation",
            "",
            aujourd_hui(),
            &[("plaquettes", "")],
        ),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert!(
        fait(&reponse).starts_with("fait=intervention-solde-a-faire;"),
        "{}",
        fait(&reponse)
    );
    assert_eq!(statut(&pool, notee.id).await, "traitee");
    let du_vehicule = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    let contre_visite = du_vehicule
        .iter()
        .find(|e| e.type_echeance == "contre_visite")
        .unwrap();
    assert_eq!(contre_visite.statut, "ouverte");
}

#[sqlx::test]
async fn une_intervention_solde_les_choses_a_faire_de_meme_operation_notees_avant_elle(
    pool: PgPool,
) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DC-200-AN").await;
    let autre_vehicule = creer_vehicule(&pool, "DC-201-AN").await;
    let plaquettes = a_faire(&pool, vehicule, "plaquettes", "").await;
    let disques = a_faire(&pool, vehicule, "disques", "").await;
    let balais = a_faire(&pool, vehicule, "balais", "").await;
    let retroviseur = a_faire(&pool, vehicule, "autre", "Rétroviseur").await;
    let ailleurs = a_faire(&pool, autre_vehicule, "plaquettes", "").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Faite hier, avant que la chose soit notée : la saisie du passé ne solde rien.
    let reponse = saisir(
        &app,
        &cookie,
        &corps(vehicule, "reparation", "", dans(-1), &[("plaquettes", "")]),
    )
    .await;
    assert!(fait(&reponse).starts_with("fait=intervention-creee;"));
    assert_eq!(statut(&pool, plaquettes).await, "ouverte");

    // Aujourd'hui, plaquettes, disques et un rétroviseur : les deux premières sont soldées.
    // « Autre » ne se reconnaît pas, les balais ne sont pas dans l'intervention, et l'autre
    // véhicule n'est pas concerné.
    let reponse = saisir(
        &app,
        &cookie,
        &corps(
            vehicule,
            "reparation",
            "",
            aujourd_hui(),
            &[
                ("plaquettes", ""),
                ("disques", ""),
                ("autre", "R%C3%A9troviseur"),
            ],
        ),
    )
    .await;
    assert_eq!(vers(&reponse), format!("/vehicules/{vehicule}"));
    assert!(
        fait(&reponse).starts_with("fait=intervention-solde-a-faire-plusieurs;"),
        "{}",
        fait(&reponse)
    );
    assert_eq!(statut(&pool, plaquettes).await, "traitee");
    assert_eq!(statut(&pool, disques).await, "traitee");
    assert_eq!(statut(&pool, balais).await, "ouverte");
    assert_eq!(statut(&pool, retroviseur).await, "ouverte");
    assert_eq!(statut(&pool, ailleurs).await, "ouverte");
}

#[sqlx::test]
async fn une_revision_solde_l_entretien_periodique_et_la_chose_a_faire(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DC-300-AN").await;
    let entretien = echeances::creer(
        &pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: "entretien_periodique".into(),
            date_limite: dans(20).to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    let filtre = a_faire(&pool, vehicule, "filtre_air", "").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le message de l'entretien périodique passe en premier, avec sa suivante ; la chose à faire
    // est soldée quand même.
    let reponse = saisir(
        &app,
        &cookie,
        &corps(
            vehicule,
            "revision",
            "",
            aujourd_hui(),
            &[("filtre_air", "")],
        ),
    )
    .await;
    assert!(vers(&reponse).starts_with("/echeances/nouvelle?"));
    assert!(fait(&reponse).starts_with("fait=intervention-solde-ep;"));
    assert_eq!(statut(&pool, entretien).await, "traitee");
    assert_eq!(statut(&pool, filtre).await, "traitee");
}

#[sqlx::test]
async fn l_api_solde_aussi_la_chose_a_faire(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DC-400-AN").await;
    let plaquettes = a_faire(&pool, vehicule, "plaquettes", "").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let json = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"reparation","date_intervention":"{}","kilometrage":"12000","lignes":[{{"operation":"plaquettes"}}]}}"#,
        aujourd_hui()
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/interventions",
        Some(&cookie),
        Some(&json),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    assert_eq!(statut(&pool, plaquettes).await, "traitee");
}
