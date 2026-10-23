//! Vue alertes et purge RGPD, sur base réelle.

mod commun;

use chrono::{Duration, NaiveDate, Utc};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::alertes::{self, Horizon, Niveau};
use gestion_parc_auto::metier::conducteurs::{self, SaisieConducteur};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, changer_conducteur, creer_conducteur, creer_utilisateur,
    creer_vehicule, get, se_connecter, texte,
};

const HORIZON: Horizon = Horizon {
    jours: 30,
    km: 1000,
};

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
            ..Default::default()
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

    let alertes = alertes::calculer(&pool, aujourd_hui, HORIZON)
        .await
        .unwrap();
    let resume: Vec<(Niveau, &str, Option<i64>)> = alertes
        .iter()
        .map(|a| (a.niveau, a.motif.as_str(), a.jours))
        .collect();
    assert_eq!(
        resume,
        vec![
            (Niveau::Depassee, "Contrôle technique", Some(-10)),
            (Niveau::AVenir, "Assurance", Some(19)),
            (Niveau::AVenir, "Permis de conduire", Some(25)),
        ]
    );

    // Traiter l'échéance dépassée la retire des alertes.
    echeances::traiter(&pool, depassee).await.unwrap();
    let alertes = alertes::calculer(&pool, aujourd_hui, HORIZON)
        .await
        .unwrap();
    assert_eq!(alertes.len(), 2);
    assert!(alertes.iter().all(|a| a.niveau == Niveau::AVenir));
}

#[sqlx::test]
async fn une_echeance_depassee_se_lit_sur_la_liste_et_sur_la_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "AL-002-AL").await;
    // Les pages se règlent sur la date du jour : celle du test aussi.
    let aujourd_hui = chrono::Local::now().date_naive();
    let hier = aujourd_hui.pred_opt().unwrap().to_string();
    let demain = aujourd_hui.succ_opt().unwrap().to_string();
    let depassee = echeance(&pool, vehicule, "controle_technique", &hier).await;
    echeance(&pool, vehicule, "assurance", &aujourd_hui.to_string()).await;
    echeance(&pool, vehicule, "entretien_periodique", &demain).await;
    // Une échéance au-delà de l'horizon des alertes : « à traiter », pas « à venir ».
    let loin = (aujourd_hui + Duration::days(60)).to_string();
    echeance(&pool, vehicule, "controle_technique", &loin).await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let pastille = |classe: &str, libelle: &str| {
        format!(r#"<span class="pastille echeance-{classe}">{libelle}</span>"#)
    };
    let fiche = format!("/vehicules/{vehicule}");
    let compter = |page: &str| {
        [
            page.matches(&pastille("depassee", "Dépassée")).count(),
            page.matches(&pastille("a_venir", "À venir")).count(),
            page.matches(&pastille("a_traiter", "À traiter")).count(),
            page.matches(&pastille("traitee", "Traitée")).count(),
        ]
    };

    // La liste et la fiche disent ce que dit la vue alertes : le jour même de sa date limite,
    // une échéance n'est pas encore dépassée, et celles de l'horizon sont « à venir ».
    for chemin in ["/echeances", fiche.as_str()] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert_eq!(compter(&page), [1, 2, 1, 0], "{chemin}\n{page}");
    }
    let niveaux: Vec<Niveau> = alertes::calculer(&pool, aujourd_hui, HORIZON)
        .await
        .unwrap()
        .iter()
        .map(|a| a.niveau)
        .collect();
    assert_eq!(
        niveaux,
        vec![Niveau::Depassee, Niveau::AVenir, Niveau::AVenir]
    );

    // Traitée, elle n'est plus dépassée nulle part.
    echeances::traiter(&pool, depassee).await.unwrap();
    for chemin in ["/echeances", fiche.as_str()] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert_eq!(compter(&page), [0, 2, 1, 1], "{chemin}\n{page}");
    }
}

#[sqlx::test]
async fn un_permis_expire_se_lit_sur_la_liste_et_sur_la_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let aujourd_hui = chrono::Local::now().date_naive().to_string();
    // Un permis valide, un qui expire aujourd'hui (encore valable ce jour-là), et deux expirés :
    // l'un d'un conducteur actif, l'autre d'un inactif.
    let valide = creer_conducteur(&pool, "Valide").await;
    let limite = creer_conducteur(&pool, "Limite").await;
    changer_conducteur(&pool, limite, "Limite", "actif", &aujourd_hui).await;
    let expire = creer_conducteur(&pool, "Expire").await;
    changer_conducteur(&pool, expire, "Expire", "actif", "2020-01-01").await;
    let parti = creer_conducteur(&pool, "Parti").await;
    changer_conducteur(&pool, parti, "Parti", "inactif", "2020-01-01").await;
    // Un permis qui expire dans dix jours : dans l'horizon des alertes, « à venir ».
    let dans_dix_jours = (chrono::Local::now().date_naive() + Duration::days(10)).to_string();
    let bientot = creer_conducteur(&pool, "Bientot").await;
    changer_conducteur(&pool, bientot, "Bientot", "actif", &dans_dix_jours).await;
    // « À venir » est un avertissement : un conducteur inactif au permis qui expire dans cinq
    // jours n'en reçoit pas, comme sur la page Alertes. « Expiré », un fait, s'écrit pour tous.
    let dans_cinq_jours = (chrono::Local::now().date_naive() + Duration::days(5)).to_string();
    let endormi = creer_conducteur(&pool, "Endormi").await;
    changer_conducteur(&pool, endormi, "Endormi", "inactif", &dans_cinq_jours).await;

    let mention = r#"<span class="pastille permis-expire">Expiré</span>"#;
    let a_venir = r#"<span class="pastille permis-a_venir">À venir</span>"#;
    for profil in [Profil::Entreprise, Profil::Particulier] {
        let app = application_avec_profil(pool.clone(), profil);
        let cookie = se_connecter(&app, "gestion").await;
        let page = texte(get(&app, "/conducteurs", Some(&cookie)).await).await;
        assert_eq!(page.matches(mention).count(), 2, "{page}");
        assert_eq!(page.matches(a_venir).count(), 2, "{page}");
        // Par conducteur : [mentions « Expiré », mentions « À venir »].
        let attendus = [
            (valide, [0, 0]),
            (limite, [0, 1]),
            (bientot, [0, 1]),
            (endormi, [0, 0]),
            (expire, [1, 0]),
            (parti, [1, 0]),
        ];
        for (id, attendu) in attendus {
            let chemin = format!("/conducteurs/{id}");
            let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
            let lu = [page.matches(mention).count(), page.matches(a_venir).count()];
            assert_eq!(lu, attendu, "{chemin}\n{page}");
        }
    }
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
