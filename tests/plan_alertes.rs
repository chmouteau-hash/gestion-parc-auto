//! Le plan d'entretien dans les alertes et sur l'accueil (#94, bloc 2b) : « Vidange dans 800 km »,
//! l'« Entretien périodique » remplacé par le plan, et la règle (a) du kilométrage qui croît avec
//! la date.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local, NaiveDate};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::plan_entretien::{self, SaisiePlan};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    requete_json, se_connecter, texte,
};

/// Le serveur compte depuis la vraie date du jour : les tests aussi.
fn aujourd_hui() -> NaiveDate {
    Local::now().date_naive()
}

fn il_y_a(jours: i64) -> NaiveDate {
    aujourd_hui() - Duration::days(jours)
}

fn intervention(
    vehicule: i64,
    date: NaiveDate,
    kilometrage: i32,
    operation: &str,
) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".into(),
        date_intervention: date.to_string(),
        kilometrage: kilometrage.to_string(),
        lignes: vec![SaisieLigne {
            operation: operation.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

async fn suivre(pool: &PgPool, vehicule: i64, operation: &str, km: &str, mois: &str) -> i64 {
    plan_entretien::creer(
        pool,
        &SaisiePlan {
            vehicule_id: vehicule.to_string(),
            operation: operation.into(),
            periodicite_km: km.into(),
            periodicite_mois: mois.into(),
            ..Default::default()
        },
        aujourd_hui(),
    )
    .await
    .unwrap()
    .0
    .id
}

/// Le véhicule de test est à 12 000 km. Une vidange faite à 11 000 km, tous les 1 800 km : la
/// prochaine est à 12 800 km, dans 800 km, sous l'horizon de 1 000 km.
#[sqlx::test]
async fn une_operation_qui_approche_est_une_alerte_partout(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-500-AN").await;
    interventions::creer(&pool, &intervention(vehicule, il_y_a(60), 11000, "vidange"))
        .await
        .unwrap();
    let ligne = suivre(&pool, vehicule, "vidange", "1800", "").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // La fiche : « À venir », en orange.
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(r#"<span class="pastille plan-a_venir">À venir</span>"#),
        "{fiche}"
    );
    assert!(fiche.contains("12\u{202F}800 km · dans 800 km"), "{fiche}");

    // La page Alertes.
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("<th>Limite</th>"), "{page}");
    for attendu in [
        "<td>Vidange</td>".to_string(),
        "<td>12\u{202F}800 km</td>".to_string(),
        "<td>dans 800 km</td>".to_string(),
        format!(r#"<a href="/vehicules/{vehicule}#plan">Ouvrir la fiche</a>"#),
        "à moins de 1\u{202F}000 km ou 30 jours".to_string(),
    ] {
        assert!(page.contains(&attendu), "{attendu} absent\n{page}");
    }
    assert!(!page.contains("Marquer traitée"), "{page}");

    // Le tableau de bord d'une entreprise : compté « à venir », et rangé avec sa barre.
    let accueil = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(
        accueil.contains("<strong class=\"chiffre-attention\">1</strong><span>À venir sous 30 jours ou 1\u{202F}000 km</span>"),
        "{accueil}"
    );
    assert!(accueil.contains("Vidange · 12\u{202F}800 km"), "{accueil}");
    assert!(
        accueil.contains(r#"class="trajet alerte-a-venir trajet-20""#),
        "{accueil}"
    );

    // L'accueil d'un particulier : la carte dit « Vidange à 12 800 km · dans 800 km », la
    // borne compte en kilomètres, et le bandeau en fait sa phrase.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_p = se_connecter(&particulier, "gestion").await;
    let accueil = texte(get(&particulier, "/", Some(&cookie_p)).await).await;
    for attendu in [
        "Vidange à 12\u{202F}800 km <span class=\"delai\">· dans 800 km</span>",
        "<strong>800</strong><small>km</small>",
        "borne-a-venir",
        "<strong>Une chose à faire</strong>",
    ] {
        assert!(accueil.contains(attendu), "{attendu} absent\n{accueil}");
    }
    // « C'est fait » ne solde qu'une échéance : rien encore pour une opération du plan.
    assert!(!accueil.contains("C'est fait"), "{accueil}");

    // L'API : la borne en kilomètres, sans date.
    let reponse = requete_json(&app, "GET", "/api/v1/alertes", Some(&cookie), None).await;
    let alertes: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(alertes[0]["motif"], "Vidange");
    assert_eq!(alertes[0]["niveau"], "a_venir");
    assert_eq!(alertes[0]["kilometrage_limite"], 12800);
    assert_eq!(alertes[0]["kilometres"], 800);
    assert_eq!(alertes[0]["plan_id"], ligne);
    assert!(alertes[0].get("date_limite").is_none());
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/plan-entretien"),
        Some(&cookie),
        None,
    )
    .await;
    let plan: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(plan[0]["etat"], "a_venir");

    // Une nouvelle vidange : la prochaine s'éloigne, l'alerte disparaît.
    interventions::creer(
        &pool,
        &intervention(vehicule, aujourd_hui(), 12000, "vidange"),
    )
    .await
    .unwrap();
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Aucune alerte"), "{page}");
}

/// Décidé le 23/10/2026 : l'« Entretien périodique » d'un véhicule qui a un plan est masqué par le
/// calcul, sans rien changer en base ; retirer le plan le fait revenir.
#[sqlx::test]
async fn le_plan_remplace_l_entretien_periodique(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-510-AN").await;
    let periodique = echeances::creer(
        &pool,
        &SaisieEcheance {
            vehicule_id: vehicule.to_string(),
            type_echeance: "entretien_periodique".into(),
            date_limite: (aujourd_hui() + Duration::days(10)).to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Entretien périodique"), "{page}");

    // Un plan : un filtre à air tous les 30 000 km, à jour.
    let ligne = suivre(&pool, vehicule, "filtre_air", "30000", "").await;
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Aucune alerte"), "{page}");
    for chemin in [format!("/vehicules/{vehicule}"), "/echeances".to_string()] {
        let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
        assert!(
            page.contains(r#"<span class="pastille echeance-remplacee">Remplacée par le plan d"#),
            "{chemin}\n{page}"
        );
    }

    // Une nouvelle « Entretien périodique » est refusée sur ce véhicule.
    let corps = format!(
        "vehicule_id={vehicule}&type_echeance=entretien_periodique&date_limite={}&statut=ouverte",
        aujourd_hui() + Duration::days(300)
    );
    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("Ce véhicule a un plan d"), "{page}");
    assert!(page.contains("il remplace l"), "{page}");

    // « Marquer traitée » ne propose pas de suivante : retour à la fiche.
    let reponse = post_formulaire(
        &app,
        &format!("/echeances/{periodique}/traiter?retour=vehicule"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}")
    );

    // Sans plan, une nouvelle « Entretien périodique » revient dans les alertes.
    plan_entretien::supprimer(&pool, ligne).await.unwrap();
    let reponse = post_formulaire(
        &app,
        "/echeances",
        Some(&cookie),
        &format!(
            "vehicule_id={vehicule}&type_echeance=entretien_periodique&date_limite={}&statut=ouverte",
            aujourd_hui() + Duration::days(5)
        ),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let page = texte(get(&app, "/alertes", Some(&cookie)).await).await;
    assert!(page.contains("Entretien périodique"), "{page}");
}

/// Règle (a) : le kilométrage croît avec la date. Une intervention qui contredit une autre du même
/// véhicule n'est pas refusée : le formulaire revient avec un avertissement et « Enregistrer
/// quand même ».
#[sqlx::test]
async fn un_kilometrage_qui_contredit_une_autre_intervention_demande_confirmation(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "PL-520-AN").await;
    interventions::creer(
        &pool,
        &intervention(vehicule, il_y_a(300), 30000, "vidange"),
    )
    .await
    .unwrap();
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Après elle, mais à 25 000 km : suspendu, rien n'est enregistré.
    let date = il_y_a(100);
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention={date}\
         &kilometrage=25000&cout=&prestataire=&notes="
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("<title>Attention : Nouvelle intervention"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            "ce véhicule était déjà à 30\u{202F}000 km ; vous avez saisi 25\u{202F}000 km pour le {}",
            date.format("%d/%m/%Y")
        )),
        "{page}"
    );
    assert!(
        page.contains(
            r#"formaction="/interventions?kilometrage_confirme=1">Enregistrer quand même</button>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(r#"name="kilometrage" value="25000""#),
        "{page}"
    );
    assert_eq!(
        interventions::lister_par_vehicule(&pool, vehicule)
            .await
            .unwrap()
            .len(),
        1
    );

    // « Enregistrer quand même » : enregistrée.
    let reponse = post_formulaire(
        &app,
        "/interventions?kilometrage_confirme=1",
        Some(&cookie),
        &corps,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let liste = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(liste.len(), 2);

    // Une intervention cohérente part sans question ; la modifier aussi.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention={}\
         &kilometrage=31000&cout=&prestataire=&notes=",
        il_y_a(50)
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let recente = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .find(|i| i.kilometrage == 31000)
        .unwrap();
    let reponse = post_formulaire(
        &app,
        &format!("/interventions/{}", recente.id),
        Some(&cookie),
        &corps.replace("31000", "31500"),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    // Le kilométrage pré-rempli depuis la fiche porte sa marque, que `app.js` efface si la date
    // passe dans le passé ; sans pré-remplissage, rien.
    let page = texte(
        get(
            &app,
            &format!("/interventions/nouvelle?vehicule_id={vehicule}&kilometrage=31500"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(r#"required data-prerempli="31500">"#),
        "{page}"
    );
    let page = texte(get(&app, "/interventions/nouvelle", Some(&cookie)).await).await;
    assert!(!page.contains("data-prerempli"), "{page}");
    let script = include_str!("../static/app.js");
    assert!(script.contains("form input[name=kilometrage][data-prerempli]"));
}
