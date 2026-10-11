//! Vue alertes et purge RGPD, sur base réelle.

mod commun;

use chrono::{Duration, NaiveDate, Utc};
use gestion_parc_auto::metier::alertes::{self, Niveau};
use gestion_parc_auto::metier::conducteurs::{self, SaisieConducteur};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{creer_conducteur, creer_vehicule};

fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, j).unwrap()
}

async fn echeance(pool: &PgPool, vehicule_id: i64, type_echeance: &str, date_limite: &str) -> i64 {
    echeances::creer(
        pool,
        &SaisieEcheance {
            vehicule_id: vehicule_id.to_string(),
            type_echeance: type_echeance.to_string(),
            date_limite: date_limite.to_string(),
            statut: String::new(),
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn depassees_proches_et_lointaines(pool: PgPool) {
    let aujourd_hui = jour(2026, 10, 11);
    let vehicule = creer_vehicule(&pool, "AL-001-AL").await;

    let depassee = echeance(&pool, vehicule, "controle_technique", "2026-10-01").await;
    echeance(&pool, vehicule, "assurance", "2026-10-30").await; // dans 19 jours
    echeance(&pool, vehicule, "entretien_periodique", "2026-12-25").await; // hors horizon

    // Permis : un actif qui expire bientôt, un inactif (ignoré), un actif lointain (ignoré).
    let bientot = creer_conducteur(&pool, "Bientot").await;
    let mut saisie = commun::saisie_conducteur("Bientot");
    saisie.date_expiration_permis = "2026-11-05".to_string();
    conducteurs::modifier(&pool, bientot, &saisie, commun::aujourd_hui())
        .await
        .unwrap();

    let inactif = creer_conducteur(&pool, "Inactif").await;
    let mut saisie = commun::saisie_conducteur("Inactif");
    saisie.date_expiration_permis = "2026-09-01".to_string();
    saisie.statut = "inactif".to_string();
    conducteurs::modifier(&pool, inactif, &saisie, commun::aujourd_hui())
        .await
        .unwrap();

    creer_conducteur(&pool, "Lointain").await;

    let alertes = alertes::calculer(&pool, aujourd_hui, 30).await.unwrap();
    let resume: Vec<(Niveau, &str, i64)> = alertes
        .iter()
        .map(|a| (a.niveau, a.motif.as_str(), a.jours))
        .collect();
    assert_eq!(
        resume,
        vec![
            (Niveau::Depassee, "Contrôle technique", -10),
            (Niveau::AVenir, "Assurance", 19),
            (Niveau::AVenir, "Permis de conduire", 25),
        ]
    );

    // Traiter l'échéance dépassée la retire des alertes.
    echeances::traiter(&pool, depassee).await.unwrap();
    let alertes = alertes::calculer(&pool, aujourd_hui, 30).await.unwrap();
    assert_eq!(alertes.len(), 2);
    assert!(alertes.iter().all(|a| a.niveau == Niveau::AVenir));
}

#[sqlx::test]
async fn purge_rgpd_des_conducteurs_inactifs(pool: PgPool) {
    let dossier = commun::dossier_documents_de_test();

    let ancien = creer_conducteur(&pool, "Ancien").await;
    let recent = creer_conducteur(&pool, "Recent").await;
    let actif = creer_conducteur(&pool, "Actif").await;

    for id in [ancien, recent] {
        let mut saisie = commun::saisie_conducteur("x");
        saisie.statut = "inactif".to_string();
        conducteurs::modifier(&pool, id, &saisie, commun::aujourd_hui())
            .await
            .unwrap();
    }
    // On vieillit artificiellement la fiche « Ancien » : inactive depuis 400 jours.
    sqlx::query("UPDATE conducteurs SET modifie_le = now() - interval '400 days' WHERE id = $1")
        .bind(ancien)
        .execute(&pool)
        .await
        .unwrap();

    let supprimes = conducteurs::purger_inactifs(&pool, &dossier, Utc::now(), 365)
        .await
        .unwrap();
    assert_eq!(supprimes, 1);

    let restants: Vec<i64> = conducteurs::lister(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(restants, vec![actif, recent]);

    // Avec une conservation plus courte, « Recent » (inactif depuis l'instant) reste protégé
    // tant que la durée n'est pas écoulée.
    let supprimes =
        conducteurs::purger_inactifs(&pool, &dossier, Utc::now() - Duration::days(1), 0)
            .await
            .unwrap();
    assert_eq!(supprimes, 0);
}

#[sqlx::test]
async fn validation_conducteur(pool: PgPool) {
    let erreur = conducteurs::creer(
        &pool,
        &SaisieConducteur {
            nom: "Sans".to_string(),
            prenom: "Date".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        erreur,
        gestion_parc_auto::erreurs::ErreurApp::Metier(_)
    ));
}
