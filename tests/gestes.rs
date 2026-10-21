//! Les gestes du quotidien, sur base réelle : la prochaine échéance après « Marquer traitée »,
//! le relevé du kilométrage en un geste, le message qui confirme chaque action, la boîte de
//! confirmation, l'export CSV. Données fictives.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::depenses::{self, SaisieDepense};
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, aujourd_hui, changer_statut_vehicule, creer_conducteur,
    creer_utilisateur, creer_vehicule, get, post_formulaire, saisie_affectation, se_connecter,
    texte,
};

/// Le code du message posé par une redirection (cookie `fait`), s'il y en a un.
fn fait_pose(reponse: &axum::response::Response) -> Option<String> {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.strip_prefix("fait=")
                .map(|reste| reste.split(';').next().unwrap_or("").to_string())
        })
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
async fn marquer_traitee_propose_la_suivante_et_le_dit(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "GE-100-ST").await;
    let controle = echeance(&pool, vehicule, "controle_technique", "2027-03-15").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // L'action pose le message et ouvre la suivante, deux ans plus tard pour un contrôle.
    let reponse = post_formulaire(
        &app,
        &format!("/echeances/{controle}/traiter?retour=vehicule"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(fait_pose(&reponse).as_deref(), Some("echeance-traitee"));
    let suite = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(
        suite,
        format!("/echeances/nouvelle?vehicule_id={vehicule}&type_echeance=controle_technique&date_limite=2029-03-15&retour=vehicule")
    );

    // La page qui suit dit le message, propose la suivante pré-remplie, et efface le message.
    let avec_message = format!("{cookie}; fait=echeance-traitee");
    let reponse = get(&app, &suite, Some(&avec_message)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let efface = reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .unwrap_or("")
                .starts_with("fait=; Path=/; Max-Age=0")
        });
    assert!(efface, "le message ne s'affiche qu'une fois");
    let page = texte(reponse).await;
    for attendu in [
        r#"<p class="message-succes" role="status">Échéance marquée traitée.</p>"#.to_string(),
        "Voici la suivante, pré-remplie".to_string(),
        format!(r#"<option value="{vehicule}" selected>"#),
        r#"<option value="controle_technique" selected>"#.to_string(),
        r#"value="2029-03-15""#.to_string(),
        format!(r#"<a href="/vehicules/{vehicule}" class="bouton-discret">Annuler</a>"#),
    ] {
        assert!(page.contains(&attendu), "{attendu}\n{page}");
    }

    // Sans le cookie, ni message ni invitation : c'est le formulaire ordinaire.
    let page = texte(get(&app, &suite, Some(&cookie)).await).await;
    assert!(!page.contains("message-succes"), "{page}");
    assert!(!page.contains("Voici la suivante"), "{page}");

    // Un message inconnu n'affiche rien.
    let page = texte(get(&app, "/vehicules", Some(&format!("{cookie}; fait=zzz"))).await).await;
    assert!(!page.contains("message-succes"), "{page}");

    // Enregistrer la suivante crée bien une échéance ouverte en 2029 ; l'ancienne est traitée.
    let corps = format!("vehicule_id={vehicule}&type_echeance=controle_technique&date_limite=2029-03-15&statut=ouverte");
    let reponse = post_formulaire(&app, "/echeances", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(fait_pose(&reponse).as_deref(), Some("echeance-creee"));
    let toutes = echeances::lister(&pool).await.unwrap();
    assert_eq!(toutes.len(), 2);
    assert_eq!(toutes.iter().filter(|e| e.est_ouverte()).count(), 1);

    // Une assurance : un an plus tard.
    let assurance = echeance(&pool, vehicule, "assurance", "2027-06-30").await;
    let reponse = post_formulaire(
        &app,
        &format!("/echeances/{assurance}/traiter"),
        Some(&cookie),
        "",
    )
    .await;
    assert!(
        reponse.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .contains("date_limite=2028-06-30&retour=liste"),
        "{:?}",
        reponse.headers()[header::LOCATION]
    );
}

#[sqlx::test]
async fn le_kilometrage_se_releve_en_un_geste(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "GE-200-ST").await;
    let pro = application(pool.clone());
    let part = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&pro, "gestion").await;

    // Le champ est sur la fiche et sur la carte d'accueil, pré-rempli du compteur actuel.
    let fiche = texte(get(&pro, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    let formulaire = format!(
        r#"<form method="post" action="/vehicules/{vehicule}/kilometrage?retour=fiche" class="releve en-ligne"><input type="number" name="kilometrage" value="12000" min="12000""#
    );
    assert!(fiche.contains(&formulaire), "{fiche}");
    let cookie_part = se_connecter(&part, "gestion").await;
    let accueil = texte(get(&part, "/", Some(&cookie_part)).await).await;
    assert!(
        accueil.contains(&format!(
            r#"action="/vehicules/{vehicule}/kilometrage?retour=accueil" class="releve""#
        )),
        "{accueil}"
    );
    assert!(!accueil.contains("Kilométrage</a>"), "{accueil}");

    // Relevé depuis l'accueil : retour à l'accueil, avec le message ; le compteur a bougé.
    let reponse = post_formulaire(
        &part,
        &format!("/vehicules/{vehicule}/kilometrage?retour=accueil"),
        Some(&cookie_part),
        "kilometrage=12500",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/");
    assert_eq!(fait_pose(&reponse).as_deref(), Some("kilometrage-releve"));
    let page = texte(
        get(
            &part,
            "/",
            Some(&format!("{cookie_part}; fait=kilometrage-releve")),
        )
        .await,
    )
    .await;
    assert!(page.contains("Kilométrage mis à jour."), "{page}");
    assert!(page.contains("12\u{202f}500 km"), "{page}");

    // Depuis la fiche : retour à la fiche.
    let reponse = post_formulaire(
        &pro,
        &format!("/vehicules/{vehicule}/kilometrage?retour=fiche"),
        Some(&cookie),
        "kilometrage=12600",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}")
    );

    // Un compteur qui recule, ou n'importe quoi : refusé, la fiche dit pourquoi.
    for (saisie, motif) in [
        (
            "kilometrage=11000",
            "Le compteur est déjà à 12\u{202f}600 km : le kilométrage ne peut pas baisser.",
        ),
        ("kilometrage=abc", "kilométrage"),
    ] {
        let reponse = post_formulaire(
            &pro,
            &format!("/vehicules/{vehicule}/kilometrage"),
            Some(&cookie),
            saisie,
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::OK, "{saisie}");
        let page = texte(reponse).await;
        assert!(
            page.contains(r#"class="message-erreur" role="alert""#),
            "{saisie}\n{page}"
        );
        assert!(page.contains(motif), "{saisie}\n{page}");
    }

    // Un lecteur ne relève rien.
    let lecteur = se_connecter(&pro, "lecture").await;
    let reponse = post_formulaire(
        &pro,
        &format!("/vehicules/{vehicule}/kilometrage"),
        Some(&lecteur),
        "kilometrage=20000",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::FORBIDDEN);
    let fiche = texte(get(&pro, &format!("/vehicules/{vehicule}"), Some(&lecteur)).await).await;
    assert!(!fiche.contains("class=\"releve"), "{fiche}");
}

#[sqlx::test]
async fn chaque_action_dit_ce_qu_elle_a_fait_et_les_suppressions_se_confirment(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Créer un véhicule : le message, sur la fiche où l'on arrive.
    let corps = "immatriculation=GE-300-ST&marque=Renault&modele=Clio&date_mise_en_service=2022-03-15&carburant=essence&kilometrage=100&statut=en_service";
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(fait_pose(&reponse).as_deref(), Some("vehicule-cree"));
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(fiche.starts_with("/vehicules/"), "{fiche}");
    let page = texte(get(&app, &fiche, Some(&format!("{cookie}; fait=vehicule-cree"))).await).await;
    assert!(
        page.contains(r#"<p class="message-succes" role="status">Véhicule enregistré.</p>"#),
        "{page}"
    );

    // Supprimer une échéance : le message, sur la liste.
    let vehicule: i64 = fiche.trim_start_matches("/vehicules/").parse().unwrap();
    let id = echeance(&pool, vehicule, "assurance", "2027-01-01").await;
    let reponse = post_formulaire(
        &app,
        &format!("/echeances/{id}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.headers()[header::LOCATION], "/echeances");
    assert_eq!(fait_pose(&reponse).as_deref(), Some("echeance-supprimee"));
    let page = texte(
        get(
            &app,
            "/echeances",
            Some(&format!("{cookie}; fait=echeance-supprimee")),
        )
        .await,
    )
    .await;
    assert!(page.contains("Échéance supprimée."), "{page}");

    // Un conducteur, créé puis supprimé.
    let conducteur = creer_conducteur(&pool, "Geste").await;
    let reponse = post_formulaire(
        &app,
        &format!("/conducteurs/{conducteur}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(fait_pose(&reponse).as_deref(), Some("conducteur-supprime"));

    // La boîte de confirmation est dans chaque page, et le script la prend en charge.
    assert!(
        page.contains(r#"<dialog class="confirmation" data-confirmation>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<p data-confirmation-texte></p>"#),
        "{page}"
    );
    let script = include_str!("../static/app.js");
    assert!(script.contains("showModal") && script.contains("[data-confirmation]"));
    assert!(
        script.contains("window.confirm"),
        "le repli sans <dialog> reste"
    );
    let style = include_str!("../static/style.css");
    assert!(style.contains(".confirmation::backdrop"));
}

#[sqlx::test]
async fn les_listes_s_exportent_en_csv(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "GE-400-ST").await;
    creer_conducteur(&pool, "Export").await;
    echeance(&pool, vehicule, "assurance", "2027-01-01").await;
    interventions::creer(
        &pool,
        &SaisieIntervention {
            vehicule_id: vehicule.to_string(),
            type_intervention: "revision".to_string(),
            resultat_controle: String::new(),
            date_intervention: "2026-05-10".to_string(),
            kilometrage: "13000".to_string(),
            cout: "149,90".to_string(),
            prestataire: "Garage \"Fictif\"".to_string(),
            notes: "Vidange ; filtres".to_string(),
            lignes: vec![
                SaisieLigne {
                    operation: "vidange".to_string(),
                    ..Default::default()
                },
                SaisieLigne {
                    operation: "filtre_huile".to_string(),
                    reference: "W712".to_string(),
                    prix: "12,50".to_string(),
                    ..Default::default()
                },
            ],
        },
    )
    .await
    .unwrap();
    depenses::creer(
        &pool,
        &SaisieDepense {
            vehicule_id: vehicule.to_string(),
            type_depense: "peage".to_string(),
            date_depense: "2026-05-11".to_string(),
            montant: "12,30".to_string(),
            libelle: "Lyon – Grenoble".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let pro = application(pool.clone());
    let cookie = se_connecter(&pro, "gestion").await;

    // Le lien, sur chaque liste d'Autoclair Pro.
    for (chemin, export) in [
        ("/vehicules", "/vehicules.csv"),
        ("/conducteurs", "/conducteurs.csv"),
        ("/echeances", "/echeances.csv"),
        ("/interventions", "/interventions.csv"),
        ("/depenses", "/depenses.csv"),
    ] {
        let page = texte(get(&pro, chemin, Some(&cookie)).await).await;
        assert!(
            page.contains(&format!(
                r#"<a class="bouton-discret export-csv" href="{export}">Exporter (CSV)</a>"#
            )),
            "{chemin}\n{page}"
        );
    }
    // Pas chez un particulier (l'adresse répond quand même).
    let part = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_part = se_connecter(&part, "gestion").await;
    let page = texte(get(&part, "/vehicules", Some(&cookie_part)).await).await;
    assert!(!page.contains("Exporter (CSV)"), "{page}");
    assert_eq!(
        get(&part, "/vehicules.csv", Some(&cookie_part))
            .await
            .status(),
        StatusCode::OK
    );

    // Le fichier : marque UTF-8, en-têtes, point-virgule, dates françaises, nombres nus.
    let reponse = get(&pro, "/vehicules.csv", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert_eq!(
        reponse.headers()[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );
    let disposition = reponse.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.starts_with("attachment; filename=\"vehicules-20")
            && disposition.ends_with(".csv\""),
        "{disposition}"
    );
    let csv = texte(reponse).await;
    assert_eq!(
        csv,
        "\u{FEFF}Immatriculation;VIN;Marque;Modèle;Couleur;Mise en service;Carburant;Kilométrage (km);Statut\r\nGE-400-ST;;Renault;Kangoo;;01/06/2023;Diesel;13000;En service\r\n"
    );

    // Les champs qui contiennent le séparateur ou un guillemet sont protégés.
    let csv = texte(get(&pro, "/interventions.csv", Some(&cookie)).await).await;
    assert!(
        csv.starts_with(
            "\u{FEFF}Date;Véhicule;Type;Résultat;Opérations;Kilométrage (km);Coût (€);Prestataire;Notes\r\n"
        ),
        "{csv}"
    );
    assert!(csv.contains("10/05/2026;GE-400-ST;Révision;;Vidange · Filtre à huile (réf. W712, 12,50\u{202F}€);13000;149,90;\"Garage \"\"Fictif\"\"\";\"Vidange ; filtres\"\r\n"), "{csv}");

    let csv = texte(get(&pro, "/conducteurs.csv", Some(&cookie)).await).await;
    assert!(
        csv.contains("Export;Camille;Logistique;01/01/2031;Actif\r\n"),
        "{csv}"
    );
    let csv = texte(get(&pro, "/echeances.csv", Some(&cookie)).await).await;
    assert!(
        csv.contains("GE-400-ST;Assurance;01/01/2027;À traiter;\r\n"),
        "{csv}"
    );
    let csv = texte(get(&pro, "/affectations.csv", Some(&cookie)).await).await;
    assert_eq!(csv, "\u{FEFF}Véhicule;Conducteur;Début;Fin;État\r\n");

    // Le lecteur exporte aussi ; sans session, non.
    let lecteur = se_connecter(&pro, "lecture").await;
    assert_eq!(
        get(&pro, "/echeances.csv", Some(&lecteur)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&pro, "/echeances.csv", None).await.status(),
        StatusCode::SEE_OTHER
    );
}

/// Un véhicule réformé par erreur se remet en service d'un clic, depuis la liste ou la fiche (#87).
/// Les affectations que la réforme a terminées ne reviennent pas, et le message le dit.
#[sqlx::test]
async fn un_vehicule_reforme_par_erreur_se_remet_en_service(pool: PgPool) {
    use gestion_parc_auto::metier::{affectations, vehicules};

    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "consultation", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "RS-111-RS").await;
    let en_service = creer_vehicule(&pool, "RS-222-RS").await;
    let martin = creer_conducteur(&pool, "Martin").await;
    let durand = creer_conducteur(&pool, "Durand").await;
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, martin, ""),
        aujourd_hui(),
    )
    .await
    .expect("affectation en cours");
    // La réforme met fin à l'affectation en cours.
    changer_statut_vehicule(&pool, vehicule, "RS-111-RS", "reforme").await;

    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let remettre = format!("/vehicules/{vehicule}/remettre-en-service");
    let action_liste = format!(r#"action="{remettre}?retour=liste""#);
    let action_fiche = format!(r#"action="{remettre}?retour=fiche""#);

    // Le bouton est dans la section « Réformés » et sur la fiche du réformé, nulle part ailleurs.
    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(liste.contains(&action_liste), "{liste}");
    assert!(liste.contains("Remettre en service"), "{liste}");
    assert_eq!(liste.matches("remettre-en-service").count(), 1, "{liste}");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(&action_fiche), "{fiche}");
    let fiche_en_service =
        texte(get(&app, &format!("/vehicules/{en_service}"), Some(&cookie)).await).await;
    assert!(
        !fiche_en_service.contains("remettre-en-service"),
        "{fiche_en_service}"
    );

    // Un lecteur ne voit pas le bouton et ne peut pas le déclencher.
    let lecteur = se_connecter(&app, "consultation").await;
    for chemin in ["/vehicules".to_string(), format!("/vehicules/{vehicule}")] {
        let page = texte(get(&app, &chemin, Some(&lecteur)).await).await;
        assert!(!page.contains("remettre-en-service"), "{chemin}\n{page}");
    }
    let refus = post_formulaire(&app, &remettre, Some(&lecteur), "").await;
    assert_eq!(refus.status(), StatusCode::FORBIDDEN);
    assert!(vehicules::obtenir(&pool, vehicule)
        .await
        .unwrap()
        .est_reforme());

    // L'administrateur : le véhicule revient « En service » et la page d'origine le dit.
    let reponse =
        post_formulaire(&app, &format!("{remettre}?retour=liste"), Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/vehicules");
    assert_eq!(
        fait_pose(&reponse).as_deref(),
        Some("vehicule-remis-en-service")
    );
    let remis = vehicules::obtenir(&pool, vehicule).await.unwrap();
    assert_eq!(remis.statut, "en_service");
    let page = texte(
        get(
            &app,
            "/vehicules",
            Some(&format!("{cookie}; fait=vehicule-remis-en-service")),
        )
        .await,
    )
    .await;
    assert!(
        page.contains("Les affectations que la réforme a terminées ne sont pas rétablies."),
        "{page}"
    );
    assert!(!page.contains("Réformés ("), "plus de réformé\n{page}");

    // L'affectation que la réforme avait terminée le reste ; le véhicule est de nouveau affectable.
    let anciennes = affectations::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(anciennes.len(), 1);
    assert!(anciennes[0].date_fin.is_some(), "affectation rouverte");
    affectations::creer(
        &pool,
        &saisie_affectation(vehicule, durand, ""),
        aujourd_hui(),
    )
    .await
    .expect("affectable de nouveau");

    // Deuxième geste, depuis la fiche : retour à la fiche. Deuxième clic (autre administrateur) :
    // le véhicule n'est plus réformé, rien ne change et rien n'échoue.
    changer_statut_vehicule(&pool, vehicule, "RS-111-RS", "reforme").await;
    let depuis_fiche = format!("{remettre}?retour=fiche");
    let reponse = post_formulaire(&app, &depuis_fiche, Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}").as_str()
    );
    let reponse = post_formulaire(&app, &depuis_fiche, Some(&cookie), "").await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        vehicules::obtenir(&pool, vehicule).await.unwrap().statut,
        "en_service"
    );

    // Un véhicule en maintenance n'est pas touché : l'action ne sert qu'aux réformés.
    changer_statut_vehicule(&pool, en_service, "RS-222-RS", "en_maintenance").await;
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{en_service}/remettre-en-service"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        vehicules::obtenir(&pool, en_service).await.unwrap().statut,
        "en_maintenance"
    );

    // Un véhicule inconnu : introuvable.
    let reponse = post_formulaire(
        &app,
        "/vehicules/999999/remettre-en-service",
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}

/// Le profil particulier n'a pas d'affectations : son message n'en parle pas.
#[sqlx::test]
async fn la_remise_en_service_du_particulier_ne_parle_pas_d_affectations(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "RT-111-RT").await;
    changer_statut_vehicule(&pool, vehicule, "RT-111-RT", "reforme").await;

    let app = application_avec_profil(pool, Profil::Particulier);
    let cookie = se_connecter(&app, "gestion").await;
    let reponse = post_formulaire(
        &app,
        &format!("/vehicules/{vehicule}/remettre-en-service?retour=liste"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        fait_pose(&reponse).as_deref(),
        Some("vehicule-remis-en-service-seul")
    );
    let page = texte(
        get(
            &app,
            "/vehicules",
            Some(&format!("{cookie}; fait=vehicule-remis-en-service-seul")),
        )
        .await,
    )
    .await;
    assert!(page.contains("Véhicule remis en service."), "{page}");
    assert!(!page.contains("ne sont pas rétablies"), "{page}");
}
