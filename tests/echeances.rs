//! Une échéance ouverte est unique par véhicule, type et date limite, prouvée sur une vraie base :
//!
//! - l'application refuse la seconde avec un message lisible, à la création comme à la
//!   modification, dans le formulaire comme dans l'API ;
//! - une échéance traitée ne compte pas : elle ne bloque pas une nouvelle saisie à la même date,
//!   mais la rouvrir est refusé si une identique est ouverte ;
//! - la base refuse le doublon même quand on contourne l'application ;
//! - la migration retire les doublons déjà saisis, en gardant le plus ancien, avant de poser la
//!   règle.

mod commun;

use axum::http::StatusCode;
use chrono::NaiveDate;
use gestion_parc_auto::erreurs::{a_le_code, contrainte_violee, ErreurApp, CODE_VIOLATION_UNICITE};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, post_formulaire, requete_json, se_connecter,
    texte,
};

fn saisie(vehicule: i64, type_echeance: &str, date_limite: &str) -> SaisieEcheance {
    SaisieEcheance {
        vehicule_id: vehicule.to_string(),
        type_echeance: type_echeance.to_string(),
        date_limite: date_limite.to_string(),
        statut: String::new(),
    }
}

/// Le message d'un refus métier ; toute autre issue fait échouer le test.
fn message_du_refus<T: std::fmt::Debug>(resultat: Result<T, ErreurApp>) -> String {
    match resultat {
        Err(ErreurApp::Metier(message)) => message,
        autre => panic!("un refus métier était attendu, reçu : {autre:?}"),
    }
}

#[sqlx::test]
async fn une_echeance_ouverte_identique_est_refusee_avec_un_message_lisible(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-100-AA").await;
    let s = saisie(vehicule, "controle_technique", "2027-03-15");

    echeances::creer(&pool, &s).await.expect("première saisie");
    let message = message_du_refus(echeances::creer(&pool, &s).await);

    assert!(
        message.contains("Contrôle technique"),
        "message : {message}"
    );
    assert!(message.contains("15/03/2027"), "message : {message}");
    assert!(
        message.contains("pas encore traitée"),
        "message : {message}"
    );
    let toutes = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(toutes.len(), 1, "le doublon n'a pas été enregistré");
}

#[sqlx::test]
async fn ce_qui_differe_par_la_date_le_type_ou_le_vehicule_est_accepte(pool: PgPool) {
    let premier = creer_vehicule(&pool, "EC-200-AA").await;
    let second = creer_vehicule(&pool, "EC-200-BB").await;

    echeances::creer(&pool, &saisie(premier, "assurance", "2027-01-31"))
        .await
        .unwrap();
    for (vehicule, type_echeance, date) in [
        (premier, "assurance", "2027-02-01"), // un jour plus tard
        (premier, "controle_technique", "2027-01-31"), // un autre type
        (second, "assurance", "2027-01-31"),  // un autre véhicule
    ] {
        echeances::creer(&pool, &saisie(vehicule, type_echeance, date))
            .await
            .unwrap_or_else(|e| panic!("{vehicule}/{type_echeance}/{date} refusé : {e:?}"));
    }
}

#[sqlx::test]
async fn une_echeance_traitee_ne_bloque_pas_une_nouvelle_saisie_a_la_meme_date(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-300-AA").await;
    let s = saisie(vehicule, "assurance", "2027-01-31");

    let premiere = echeances::creer(&pool, &s).await.unwrap();
    echeances::traiter(&pool, premiere.id).await.unwrap();

    // La première est traitée : la même échéance peut être saisie de nouveau, puis traitée à son
    // tour, autant de fois qu'il le faut.
    let seconde = echeances::creer(&pool, &s)
        .await
        .expect("une échéance traitée ne compte pas");
    echeances::traiter(&pool, seconde.id).await.unwrap();
    echeances::creer(&pool, &s).await.unwrap();

    let toutes = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(toutes.len(), 3);
}

#[sqlx::test]
async fn rouvrir_une_echeance_traitee_est_refuse_si_une_identique_est_ouverte(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-400-AA").await;
    let s = saisie(vehicule, "assurance", "2027-01-31");

    let traitee = echeances::creer(&pool, &s).await.unwrap();
    echeances::traiter(&pool, traitee.id).await.unwrap();
    echeances::creer(&pool, &s).await.unwrap();

    // Repasser la première à « ouverte » reviendrait à doubler la seconde.
    let rouverte = SaisieEcheance {
        statut: "ouverte".to_string(),
        ..s
    };
    let message = message_du_refus(echeances::modifier(&pool, traitee.id, &rouverte).await);
    assert!(message.contains("Assurance"), "message : {message}");
    assert!(message.contains("31/01/2027"), "message : {message}");

    let apres = echeances::obtenir(&pool, traitee.id).await.unwrap();
    assert!(!apres.est_ouverte(), "elle reste traitée");
}

#[sqlx::test]
async fn modifier_vers_une_echeance_identique_est_refuse_mais_pas_se_re_enregistrer(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-500-AA").await;
    let a = echeances::creer(&pool, &saisie(vehicule, "assurance", "2027-03-15"))
        .await
        .unwrap();
    let b = echeances::creer(&pool, &saisie(vehicule, "assurance", "2027-06-15"))
        .await
        .unwrap();

    // Enregistrer une échéance sans rien changer n'est pas un doublon d'elle-même.
    echeances::modifier(&pool, a.id, &saisie(vehicule, "assurance", "2027-03-15"))
        .await
        .expect("se re-enregistrer à l'identique");

    // Mais la déplacer sur la date d'une autre l'est.
    let message = message_du_refus(
        echeances::modifier(&pool, b.id, &saisie(vehicule, "assurance", "2027-03-15")).await,
    );
    assert!(message.contains("15/03/2027"), "message : {message}");
    let inchangee = echeances::obtenir(&pool, b.id).await.unwrap();
    assert_eq!(inchangee.date_limite.to_string(), "2027-06-15");
}

#[sqlx::test]
async fn la_base_refuse_le_doublon_meme_en_contournant_l_application(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-600-AA").await;
    let inserer = |statut: &'static str| {
        sqlx::query(
            "INSERT INTO echeances (vehicule_id, type_echeance, date_limite, statut)
             VALUES ($1, 'assurance', $2, $3)",
        )
        .bind(vehicule)
        .bind(NaiveDate::from_ymd_opt(2027, 1, 31).unwrap())
        .bind(statut)
        .execute(&pool)
    };

    inserer("ouverte").await.unwrap();
    // Une échéance traitée de même clé : la base l'accepte.
    inserer("traitee")
        .await
        .expect("l'index ne vise que les ouvertes");

    let erreur = inserer("ouverte")
        .await
        .expect_err("l'index unique partiel doit refuser le doublon");
    assert!(a_le_code(&erreur, CODE_VIOLATION_UNICITE), "{erreur}");
    assert_eq!(
        contrainte_violee(&erreur).as_deref(),
        Some("echeances_pas_de_doublon_ouverte")
    );
}

#[sqlx::test]
async fn la_migration_retire_les_doublons_ouverts_et_garde_le_plus_ancien(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "EC-700-AA").await;

    // Le décor d'une base qui existait avant la règle : sans l'index, les doublons s'enregistrent.
    sqlx::query("DROP INDEX echeances_pas_de_doublon_ouverte")
        .execute(&pool)
        .await
        .unwrap();
    let inserer = |type_echeance: &'static str, date: &'static str, statut: &'static str| {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO echeances (vehicule_id, type_echeance, date_limite, statut)
             VALUES ($1, $2, $3::date, $4) RETURNING id",
        )
        .bind(vehicule)
        .bind(type_echeance)
        .bind(date)
        .bind(statut)
        .fetch_one(&pool)
    };
    let plus_ancienne = inserer("assurance", "2027-01-31", "ouverte").await.unwrap();
    inserer("assurance", "2027-01-31", "ouverte").await.unwrap();
    inserer("assurance", "2027-01-31", "ouverte").await.unwrap();
    let seule = inserer("assurance", "2027-02-28", "ouverte").await.unwrap();
    inserer("assurance", "2027-01-31", "traitee").await.unwrap();
    inserer("assurance", "2027-01-31", "traitee").await.unwrap();

    // La migration elle-même, telle qu'elle s'exécutera au démarrage de l'application.
    sqlx::raw_sql(include_str!(
        "../migrations/20261014190000_echeances_pas_de_doublon_ouverte.sql"
    ))
    .execute(&pool)
    .await
    .expect("la migration passe sur des doublons existants");

    let ouvertes: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM echeances
         WHERE vehicule_id = $1 AND statut = 'ouverte' ORDER BY id",
    )
    .bind(vehicule)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ouvertes, vec![plus_ancienne, seule]);

    // Les échéances traitées, même identiques, sont intactes.
    let traitees: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM echeances WHERE vehicule_id = $1 AND statut = 'traitee'",
    )
    .bind(vehicule)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(traitees, 2);

    // Et la règle est posée.
    message_du_refus(echeances::creer(&pool, &saisie(vehicule, "assurance", "2027-01-31")).await);
}

#[sqlx::test]
async fn le_formulaire_et_l_api_refusent_le_doublon(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "EC-800-AA").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Formulaire : la première saisie redirige, la seconde ré-affiche le formulaire avec le motif.
    let corps = format!(
        "vehicule_id={vehicule}&type_echeance=assurance&date_limite=2027-01-31&statut=ouverte"
    );
    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"<p class="message-erreur" role="alert">"#),
        "{page}"
    );
    assert!(
        page.contains("a déjà une échéance « Assurance » au 31/01/2027"),
        "{page}"
    );

    // API : 400 et le même motif en JSON.
    let json = format!(
        r#"{{"vehicule_id":"{vehicule}","type_echeance":"assurance","date_limite":"2027-01-31"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/echeances",
        Some(&cookie),
        Some(&json),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    let corps: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let motif = corps["erreur"].as_str().expect("motif en texte");
    assert!(
        motif.contains("a déjà une échéance « Assurance »"),
        "{motif}"
    );

    // Une seule échéance a été enregistrée.
    let toutes = echeances::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(toutes.len(), 1);
}
