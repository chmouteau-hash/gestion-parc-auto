//! L'accueil de chaque profil, sur base réelle : cartes Autoclair, tableau de bord Autoclair Pro.
//!
//! Les dates sont relatives au jour du test : l'accueil, comme la page Alertes, compte depuis
//! aujourd'hui.

mod commun;

use axum::http::{header, StatusCode};
use chrono::{Duration, Local};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, changer_conducteur, changer_statut_vehicule,
    creer_conducteur, creer_utilisateur, creer_vehicule, get, post_formulaire, se_connecter, texte,
};

fn dans(jours: i64) -> String {
    (Local::now().date_naive() + Duration::days(jours)).to_string()
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
async fn l_accueil_d_un_particulier_dit_ce_qu_il_y_a_a_faire_par_vehicule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let clio = creer_vehicule(&pool, "AA-111-AA").await;
    let deux_cent_huit = creer_vehicule(&pool, "BB-222-BB").await;
    let transit = creer_vehicule(&pool, "CC-333-CC").await;
    let sandero = creer_vehicule(&pool, "DD-444-DD").await;
    changer_statut_vehicule(&pool, sandero, "DD-444-DD", "reforme").await;
    echeance(&pool, clio, "controle_technique", &dans(15)).await;
    echeance(&pool, clio, "assurance", &dans(200)).await; // plus lointaine : pas la prochaine
    let assurance = echeance(&pool, deux_cent_huit, "assurance", &dans(-5)).await;
    echeance(&pool, transit, "entretien_periodique", &dans(120)).await;

    let app = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&app, "gestion").await;
    let reponse = get(&app, "/", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;

    // Le menu nomme l'accueil et le marque comme page en cours.
    assert!(
        page.contains(
            r#"<a href="/" data-ico="voiture" class="actif" aria-current="page">Véhicules</a>"#
        ),
        "{page}"
    );
    assert!(
        page.contains("<title>Accueil · Autoclair</title>"),
        "{page}"
    );

    // Deux alertes dans l'horizon (assurance dépassée, contrôle technique dans 15 jours) : le
    // bandeau les compte et cite la plus pressante.
    assert!(page.contains("2 choses à faire"), "{page}");
    assert!(
        page.contains("Assurance · Véhicule BB-222-BB, depuis 5 jours."),
        "{page}"
    );
    assert!(
        page.contains(r#"<a href="/alertes">Tout voir</a>"#),
        "{page}"
    );

    // Une carte par véhicule au parc, le plus pressé en premier. Le réformé n'a pas de carte :
    // l'accueil dit seulement qu'il existe.
    let position = |texte: &str| {
        page.find(texte)
            .unwrap_or_else(|| panic!("absent : {texte}\n{page}"))
    };
    assert!(position("BB-222-BB") < position("AA-111-AA"));
    assert!(position("AA-111-AA") < position("CC-333-CC"));
    assert!(!page.contains("DD-444-DD"), "{page}");
    assert!(
        page.contains(
            r#"1 véhicule réformé, en bas de la <a href="/vehicules#reformes">liste des véhicules</a>."#
        ),
        "{page}"
    );

    // La borne : couleur, chiffre, unité.
    assert!(page.contains(r#"class="borne borne-depassee""#), "{page}");
    assert!(
        page.contains("<strong>5</strong><small>retard</small>"),
        "{page}"
    );
    assert!(page.contains(r#"class="borne borne-a-venir""#), "{page}");
    assert!(
        page.contains("<strong>15</strong><small>jours</small>"),
        "{page}"
    );
    assert!(page.contains(r#"class="borne borne-a-jour""#), "{page}");
    assert!(
        page.contains("<strong>120</strong><small>jours</small>"),
        "{page}"
    );
    assert!(!page.contains("borne-neutre"), "{page}");

    // La plaque dessinée, le délai à côté de la date, le kilométrage lisible.
    assert!(
        page.contains(r#"<span class="plaque">AA-111-AA</span>"#),
        "{page}"
    );
    assert!(page.contains("· dans 15 jours"), "{page}");
    assert!(page.contains("12\u{202F}000 km"), "{page}");

    // Un seul bouton principal : ajouter un véhicule. « C'est fait » est un bouton discret.
    assert_eq!(page.matches(r#"class="bouton""#).count(), 1, "{page}");
    assert!(
        page.contains(r#"<a class="bouton" href="/vehicules/nouveau">Ajouter un véhicule</a>"#),
        "{page}"
    );
    let action = format!(r#"action="/echeances/{assurance}/traiter?retour=accueil""#);
    assert!(page.contains(&action), "{page}");
    // « C'est fait » ne se propose que pour ce qui presse : pas pour le Transit (120 jours).
    assert_eq!(page.matches("C'est fait").count(), 2, "{page}");

    // « C'est fait » propose la prochaine échéance, dont « Annuler » ramène à l'accueil, où il ne
    // reste que le contrôle technique.
    let chemin = format!("/echeances/{assurance}/traiter?retour=accueil");
    let reponse = post_formulaire(&app, &chemin, Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let suite = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        suite.starts_with("/echeances/nouvelle?vehicule_id=") && suite.ends_with("&retour=accueil"),
        "{suite}"
    );
    let page = texte(get(&app, &suite, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<a href="/" class="bouton-discret">Annuler</a>"#),
        "{page}"
    );
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(!page.contains("choses à faire"), "{page}");
    assert!(
        page.contains("<strong>Une chose à faire</strong>"),
        "{page}"
    );
    assert!(
        page.contains("Contrôle technique · Véhicule AA-111-AA, dans 15 jours."),
        "{page}"
    );
    assert!(!page.contains("Tout voir"), "{page}");
}

#[sqlx::test]
async fn l_accueil_vide_invite_a_ajouter_un_vehicule(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let app = application_avec_profil(pool.clone(), Profil::Particulier);

    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(page.contains("Tout est à jour."), "{page}");
    assert!(page.contains(r#"<svg class="a-route""#), "{page}");
    assert!(page.contains("Aucun véhicule pour le moment."), "{page}");
    assert!(
        page.contains(
            r#"<a class="bouton" href="/vehicules/nouveau">Ajouter votre premier véhicule</a>"#
        ),
        "{page}"
    );

    // Un lecteur voit l'écran vide sans le bouton : il ne peut rien ajouter.
    let lecteur = se_connecter(&app, "lecture").await;
    let page = texte(get(&app, "/", Some(&lecteur)).await).await;
    assert!(page.contains("Aucun véhicule pour le moment."), "{page}");
    assert!(!page.contains("Ajouter votre premier véhicule"), "{page}");

    // Un véhicule dont l'échéance est hors de l'horizon : rien à faire, et le bandeau dit quand
    // ce sera le tour.
    let kangoo = creer_vehicule(&pool, "AA-111-AA").await;
    echeance(&pool, kangoo, "entretien_periodique", &dans(120)).await;
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(page.contains("<strong>Tout est à jour.</strong>"), "{page}");
    assert!(
        page.contains(
            "Prochaine échéance : Entretien périodique de Renault Kangoo, dans 120 jours."
        ),
        "{page}"
    );
    assert!(page.contains(r#"class="borne borne-a-jour""#), "{page}");
}

#[sqlx::test]
async fn le_tableau_de_bord_d_une_entreprise_compte_et_classe_par_urgence(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let clio = creer_vehicule(&pool, "AA-111-AA").await;
    let deux_cent_huit = creer_vehicule(&pool, "BB-222-BB").await;
    let transit = creer_vehicule(&pool, "CC-333-CC").await;
    changer_statut_vehicule(&pool, transit, "CC-333-CC", "en_maintenance").await;
    echeance(&pool, clio, "controle_technique", &dans(15)).await;
    let assurance = echeance(&pool, deux_cent_huit, "assurance", &dans(-5)).await;
    let expire = creer_conducteur(&pool, "Petit").await;
    changer_conducteur(&pool, expire, "Petit", "actif", &dans(-20)).await;
    let bientot = creer_conducteur(&pool, "Durand").await;
    changer_conducteur(&pool, bientot, "Durand", "actif", &dans(12)).await;
    interventions::creer(
        &pool,
        &SaisieIntervention {
            vehicule_id: clio.to_string(),
            type_intervention: "revision".to_string(),
            resultat_controle: String::new(),
            date_intervention: dans(-90),
            kilometrage: "45000".to_string(),
            cout: "149,90".to_string(),
            prestataire: "Garage fictif".to_string(),
            notes: String::new(),
            lignes: Vec::new(),
        },
    )
    .await
    .unwrap();

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let page = texte(get(&app, "/", Some(&cookie)).await).await;

    assert!(
        page.contains("<title>Tableau de bord · Autoclair Pro</title>"),
        "{page}"
    );
    assert!(
        page.contains(r#"<a href="/" data-ico="tableau" class="actif" aria-current="page">Tableau de bord</a>"#),
        "{page}"
    );

    // Les chiffres : 2 dépassées (assurance + permis), 2 à venir, 1 en maintenance, 1 permis
    // expiré, 2 en service.
    assert!(
        page.contains(
            r#"<strong class="chiffre-danger">2</strong><span>Échéances dépassées</span>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(
            "<strong class=\"chiffre-attention\">2</strong><span>À venir sous 30 jours ou 1\u{202F}000 km</span>"
        ),
        "{page}"
    );
    assert!(
        page.contains("<strong>1</strong><span>En maintenance</span>"),
        "{page}"
    );
    assert!(
        page.contains(r#"<strong class="chiffre-danger">1</strong><span>Permis expiré</span>"#),
        "{page}"
    );
    assert!(
        page.contains("<strong>2</strong><span>En service</span>"),
        "{page}"
    );

    // Par urgence : la plus ancienne date en premier, avec la barre de trajet remplie.
    let position = |texte: &str| {
        page.find(texte)
            .unwrap_or_else(|| panic!("absent : {texte}\n{page}"))
    };
    assert!(position("Conducteur Camille Petit") < position("Véhicule BB-222-BB"));
    assert!(position("Véhicule BB-222-BB") < position("Conducteur Camille Durand"));
    assert!(position("Conducteur Camille Durand") < position("Véhicule AA-111-AA"));
    assert!(
        page.contains(r#"class="trajet alerte-depassee trajet-100""#),
        "{page}"
    );
    assert!(
        page.contains(r#"class="trajet alerte-a-venir trajet-50""#),
        "{page}"
    );
    assert!(
        page.contains(r#"class="trajet alerte-a-venir trajet-60""#),
        "{page}"
    );
    let action = format!(r#"action="/echeances/{assurance}/traiter?retour=accueil""#);
    assert!(page.contains(&action), "{page}");

    // Sans conducteur : les deux véhicules en service, pas celui en maintenance.
    assert!(page.contains("Véhicules sans conducteur"), "{page}");
    let sans = &page[position("Véhicules sans conducteur")..position("Dernières interventions")];
    assert!(
        sans.contains("AA-111-AA") && sans.contains("BB-222-BB"),
        "{sans}"
    );
    assert!(!sans.contains("CC-333-CC"), "{sans}");

    // Dernières interventions : la révision, avec son kilométrage lisible et son coût.
    assert!(page.contains("Révision, 45\u{202F}000 km"), "{page}");
    assert!(page.contains("149,90\u{202F}€"), "{page}");

    // Un seul bouton principal.
    assert_eq!(page.matches(r#"class="bouton""#).count(), 1, "{page}");
}

#[sqlx::test]
async fn le_delai_s_ecrit_partout_ou_une_date_limite_s_affiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let clio = creer_vehicule(&pool, "AA-111-AA").await;
    echeance(&pool, clio, "controle_technique", &dans(15)).await;
    let conducteur = creer_conducteur(&pool, "Petit").await;
    changer_conducteur(&pool, conducteur, "Petit", "actif", &dans(-20)).await;
    let lointain = creer_conducteur(&pool, "Martin").await;
    changer_conducteur(&pool, lointain, "Martin", "actif", &dans(400)).await;

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let delai = r#"<span class="delai">· dans 15 jours</span>"#;
    for chemin in ["/echeances", &format!("/vehicules/{clio}")] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(page.contains(delai), "{chemin}\n{page}");
    }
    let expire = r#"<span class="delai">· depuis 20 jours</span>"#;
    for chemin in ["/conducteurs", &format!("/conducteurs/{conducteur}")] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(page.contains(expire), "{chemin}\n{page}");
    }
    // Un permis valable encore longtemps n'a pas de délai : rien ne presse.
    let page = texte(get(&app, &format!("/conducteurs/{lointain}"), Some(&cookie)).await).await;
    assert!(!page.contains(r#"class="delai""#), "{page}");
}

#[sqlx::test]
async fn les_fiches_et_les_formulaires_commencent_par_un_lien_de_retour(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let clio = creer_vehicule(&pool, "AA-111-AA").await;
    let conducteur = creer_conducteur(&pool, "Petit").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    for (chemin, retour) in [
        (
            format!("/vehicules/{clio}"),
            r#"<a class="retour" href="/vehicules">Véhicules</a>"#,
        ),
        (
            "/vehicules/nouveau".to_string(),
            r#"<a class="retour" href="/vehicules">Véhicules</a>"#,
        ),
        (
            format!("/vehicules/{clio}/modifier"),
            r#"<a class="retour" href="/vehicules">Véhicules</a>"#,
        ),
        (
            format!("/conducteurs/{conducteur}"),
            r#"<a class="retour" href="/conducteurs">Conducteurs</a>"#,
        ),
        (
            "/echeances/nouvelle".to_string(),
            r#"<a class="retour" href="/echeances">Échéances</a>"#,
        ),
        (
            "/interventions/nouvelle".to_string(),
            r#"<a class="retour" href="/interventions">Entretien</a>"#,
        ),
        (
            "/affectations/nouvelle".to_string(),
            r#"<a class="retour" href="/affectations">Affectations</a>"#,
        ),
        (
            "/documents/nouveau".to_string(),
            r#"<a class="retour" href="/">Tableau de bord</a>"#,
        ),
    ] {
        let page = texte(get(&app, &chemin, Some(&cookie)).await).await;
        assert!(page.contains(retour), "{chemin}\n{page}");
        // Le lien précède le contenu : c'est le premier repère de la page.
        assert!(page.find(retour) < page.find("<h1>"), "{chemin}");
    }
    // Une liste n'en a pas : le menu suffit.
    let page = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(!page.contains(r#"class="retour""#), "{page}");
}

#[sqlx::test]
async fn les_listes_vides_montrent_le_motif_et_l_action(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    for (chemin, texte_vide, action) in [
        (
            "/vehicules",
            "Aucun véhicule enregistré.",
            "Nouveau véhicule",
        ),
        (
            "/conducteurs",
            "Aucun conducteur enregistré.",
            "Nouveau conducteur",
        ),
        (
            "/echeances",
            "Aucune échéance enregistrée.",
            "Nouvelle échéance",
        ),
        (
            "/interventions",
            "Aucune intervention enregistrée.",
            "Nouvelle intervention",
        ),
        (
            "/affectations",
            "Aucune affectation.",
            "Nouvelle affectation",
        ),
    ] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(page.contains(r#"<svg class="a-route""#), "{chemin}\n{page}");
        assert!(page.contains(texte_vide), "{chemin}\n{page}");
        // L'action de l'écran vide est le seul bouton principal de la page.
        assert_eq!(
            page.matches(r#"class="bouton""#).count(),
            1,
            "{chemin}\n{page}"
        );
        assert!(
            page.contains(&format!(r#"class="bouton" href="{chemin}/nouve"#)),
            "{chemin}\n{page}"
        );
        assert!(page.contains(&format!(">{action}</a>")), "{chemin}\n{page}");
    }
}
