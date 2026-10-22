//! Les dépenses pour rouler (#126, bloc 1) : les pleins de carburant et les tickets de péage, de
//! la saisie à la fiche, par le formulaire HTML et par l'API JSON ; le compteur du véhicule qui
//! suit un plein ; l'export CSV ; la navigation des deux marques ; le ticket joint (bloc 2) ;
//! la consommation, les totaux, les filtres et le chiffre du tableau de bord (bloc 3). Données
//! fictives.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::erreurs::{a_le_code, ErreurApp, CODE_VIOLATION_CHECK};
use gestion_parc_auto::metier::depenses::{self, SaisieDepense};
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::vehicules::{self, SaisieVehicule};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    post_multipart, post_multipart_nomme, requete_json, saisie_vehicule, se_connecter, texte,
};

fn plein(
    vehicule: i64,
    date: &str,
    montant: &str,
    quantite: &str,
    kilometrage: &str,
) -> SaisieDepense {
    SaisieDepense {
        vehicule_id: vehicule.to_string(),
        type_depense: "carburant".to_string(),
        date_depense: date.to_string(),
        montant: montant.to_string(),
        carburant: "gazole".to_string(),
        quantite: quantite.to_string(),
        kilometrage: kilometrage.to_string(),
        plein_complet: "oui".to_string(),
        libelle: "Station fictive".to_string(),
        notes: String::new(),
    }
}

fn peage(vehicule: i64, date: &str, montant: &str, trajet: &str) -> SaisieDepense {
    SaisieDepense {
        vehicule_id: vehicule.to_string(),
        type_depense: "peage".to_string(),
        date_depense: date.to_string(),
        montant: montant.to_string(),
        libelle: trajet.to_string(),
        ..Default::default()
    }
}

async fn kilometrage(pool: &PgPool, vehicule: i64) -> i32 {
    vehicules::obtenir(pool, vehicule)
        .await
        .unwrap()
        .kilometrage
}

#[sqlx::test]
async fn un_plein_releve_le_compteur_et_un_peage_ne_porte_que_son_montant(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "DP-100-DP").await;
    let autre = creer_vehicule(&pool, "DP-101-DP").await;
    assert_eq!(kilometrage(&pool, vehicule).await, 12000);

    // Un plein avec son compteur relève le véhicule.
    let p1 = depenses::creer(
        &pool,
        &plein(vehicule, "2026-10-12", "68,40", "45,2", "61 200"),
    )
    .await
    .unwrap();
    assert_eq!(p1.montant_centimes, 6840);
    assert_eq!(p1.quantite_centiemes, 4520);
    assert_eq!(p1.kilometrage, Some(61200));
    assert!(p1.plein_complet);
    assert_eq!(p1.immatriculation, "DP-100-DP");
    assert_eq!(p1.carburant, "gazole");
    assert_eq!(
        p1.detail_fr(),
        "45,2 L Gazole (B7) · 61\u{202F}200 km · Station fictive"
    );
    assert_eq!(p1.montant_fr(), "68,40\u{202F}€");
    assert_eq!(kilometrage(&pool, vehicule).await, 61200);

    // Un plein ancien, avec moins de kilomètres, ne fait pas reculer le compteur.
    depenses::creer(&pool, &plein(vehicule, "2026-01-05", "60", "40", "8000"))
        .await
        .unwrap();
    assert_eq!(kilometrage(&pool, vehicule).await, 61200);

    // Sans compteur ni quantité : accepté, rien ne bouge.
    let mut sans = plein(vehicule, "2026-10-20", "30", "", "");
    sans.plein_complet = String::new();
    let p3 = depenses::creer(&pool, &sans).await.unwrap();
    assert_eq!(
        (p3.quantite_centiemes, p3.kilometrage, p3.plein_complet),
        (0, None, false)
    );
    assert_eq!(
        p3.detail_fr(),
        "Gazole (B7) · plein partiel · Station fictive"
    );

    // Le carburant choisi prime sur celui proposé ; un inconnu est refusé.
    let mut sp98 = plein(vehicule, "2026-10-15", "70", "40", "");
    sp98.carburant = "sp98".to_string();
    let p4 = depenses::creer(&pool, &sp98).await.unwrap();
    assert_eq!(p4.carburant, "sp98");
    assert_eq!(p4.unite(), "L");
    sp98.carburant = "kerosene".to_string();
    assert!(matches!(
        depenses::creer(&pool, &sp98).await,
        Err(ErreurApp::Metier(m)) if m.contains("kerosene")
    ));
    sp98.carburant = String::new();
    assert!(matches!(
        depenses::creer(&pool, &sp98).await,
        Err(ErreurApp::Metier(m)) if m.contains("carburant")
    ));
    sp98.carburant = String::new();
    assert!(matches!(
        depenses::creer(&pool, &sp98).await,
        Err(ErreurApp::Metier(m)) if m.contains("carburant")
    ));
    depenses::supprimer(&pool, p4.id).await.unwrap();
    assert_eq!(kilometrage(&pool, vehicule).await, 61200);

    // Un péage : le trajet, le montant, rien d'autre.
    let pe = depenses::creer(
        &pool,
        &peage(vehicule, "2026-10-13", "12,30", "Lyon – Grenoble"),
    )
    .await
    .unwrap();
    assert!(!pe.est_un_plein());
    assert_eq!(pe.type_libelle(), "Péage");
    assert_eq!(pe.detail_fr(), "Lyon – Grenoble");
    assert_eq!(pe.montant_centimes, 1230);

    // Les listes : les plus récentes en premier, et par véhicule.
    let toutes = depenses::lister(&pool).await.unwrap();
    let dates: Vec<String> = toutes.iter().map(|d| d.date_depense.to_string()).collect();
    assert_eq!(
        dates,
        ["2026-10-20", "2026-10-13", "2026-10-12", "2026-01-05"]
    );
    assert_eq!(
        depenses::lister_par_vehicule(&pool, vehicule)
            .await
            .unwrap()
            .len(),
        4
    );
    assert!(depenses::lister_par_vehicule(&pool, autre)
        .await
        .unwrap()
        .is_empty());

    // Modifier : le compteur suit encore ; un péage avec une quantité est refusé sans rien changer.
    let modifie = depenses::modifier(
        &pool,
        p1.id,
        &plein(vehicule, "2026-10-12", "70", "46", "62000"),
    )
    .await
    .unwrap();
    assert_eq!(modifie.montant_centimes, 7000);
    assert_eq!(kilometrage(&pool, vehicule).await, 62000);
    let mut faux = peage(vehicule, "2026-10-13", "12,30", "Lyon – Grenoble");
    faux.quantite = "10".to_string();
    let refus = depenses::modifier(&pool, pe.id, &faux).await;
    assert!(matches!(refus, Err(ErreurApp::Metier(m)) if m.contains("péage")));
    assert_eq!(
        depenses::obtenir(&pool, pe.id)
            .await
            .unwrap()
            .quantite_centiemes,
        0
    );
    let refus = depenses::creer(&pool, &plein(999_999, "2026-10-12", "1", "", "")).await;
    assert!(matches!(refus, Err(ErreurApp::Metier(m)) if m.contains("n'existe plus")));

    // Supprimer, une fois.
    depenses::supprimer(&pool, p3.id).await.unwrap();
    assert_eq!(depenses::lister(&pool).await.unwrap().len(), 3);
    assert!(matches!(
        depenses::supprimer(&pool, p3.id).await,
        Err(ErreurApp::Introuvable)
    ));
    assert!(matches!(
        depenses::obtenir(&pool, p3.id).await,
        Err(ErreurApp::Introuvable)
    ));

    // La base tient ses règles même en la contournant : un péage sans détail, un type connu.
    for (sql, cas) in [
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, quantite_centiemes)
             VALUES ($1, 'peage', '2026-10-13', 10)",
            "quantité sur un péage",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, kilometrage)
             VALUES ($1, 'peage', '2026-10-13', 100)",
            "kilométrage sur un péage",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, plein_complet)
             VALUES ($1, 'peage', '2026-10-13', TRUE)",
            "plein complet sur un péage",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, carburant)
             VALUES ($1, 'parking', '2026-10-13', 'gazole')",
            "type inconnu",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense)
             VALUES ($1, 'carburant', '2026-10-13')",
            "plein sans carburant",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, carburant)
             VALUES ($1, 'peage', '2026-10-13', 'gazole')",
            "carburant sur un péage",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, carburant)
             VALUES ($1, 'carburant', '2026-10-13', 'kerosene')",
            "carburant inconnu",
        ),
        (
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, carburant, montant_centimes)
             VALUES ($1, 'carburant', '2026-10-13', 'gazole', -1)",
            "montant négatif",
        ),
    ] {
        let erreur = sqlx::query(sql)
            .bind(vehicule)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(a_le_code(&erreur, CODE_VIOLATION_CHECK), "{cas} : {erreur}");
    }

    // Supprimer le véhicule emporte ses dépenses (cascade).
    vehicules::supprimer(&pool, &std::env::temp_dir(), vehicule)
        .await
        .unwrap();
    assert!(depenses::lister(&pool).await.unwrap().is_empty());
}

#[sqlx::test]
async fn le_formulaire_note_un_plein_et_un_peage_depuis_la_fiche(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DP-200-DP").await;
    let electrique = vehicules::creer(
        &pool,
        &SaisieVehicule {
            carburant: "electrique".to_string(),
            ..saisie_vehicule("DP-201-EL")
        },
    )
    .await
    .unwrap()
    .id;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let fiche = format!("/vehicules/{vehicule}");

    // Une liste vide : rien à imprimer, et le geste proposé.
    let page = texte(get(&app, "/depenses", Some(&cookie)).await).await;
    assert!(page.contains("Aucune dépense enregistrée."), "{page}");
    assert!(!page.contains("data-imprimer"), "{page}");

    // La fiche propose les deux gestes, et son bloc est vide.
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<h2 data-ico="carburant">Dépenses</h2>"#),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"href="/depenses/nouvelle?vehicule_id={vehicule}&amp;type_depense=carburant">Noter un plein</a>"#
        )),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"href="/depenses/nouvelle?vehicule_id={vehicule}&amp;type_depense=peage">Noter un péage</a>"#
        )),
        "{page}"
    );
    assert!(page.contains("Aucune dépense."), "{page}");

    // « Noter un plein » : le véhicule et le type sont choisis, la case cochée, l'unité en
    // litres, et le kilométrage vide (il se lit au compteur, pas sur la fiche).
    let page = texte(
        get(
            &app,
            &format!("/depenses/nouvelle?vehicule_id={vehicule}&type_depense=carburant"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(&format!(
            r#"<option value="{vehicule}" data-carburant="gazole" data-kilometrage="12000" selected>"#
        )),
        "{page}"
    );
    assert!(page.contains(r#"value="carburant" selected"#), "{page}");
    // Le carburant du plein est proposé d'après le véhicule, dans sa liste, avec son unité.
    assert!(
        page.contains(r#"<option value="gazole" data-unite="L" selected>Gazole (B7)</option>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<option value="sp98" data-unite="L">SP98</option>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"name="plein_complet" value="oui" checked"#),
        "{page}"
    );
    assert!(page.contains("<span data-unite>L</span>"), "{page}");
    // Le saut de kilométrage suspect se confirme aussi sur un plein (#4).
    assert!(
        page.contains(r#"name="kilometrage" value="" data-seuil-km="20000">"#),
        "{page}"
    );
    assert!(page.contains(r#"name="kilometrage" value="""#), "{page}");
    assert!(
        page.contains(r#"<div data-pour-type="carburant">"#),
        "{page}"
    );
    // Pour un électrique, l'unité est le kilowattheure.
    let page = texte(
        get(
            &app,
            &format!("/depenses/nouvelle?vehicule_id={electrique}"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(page.contains("<span data-unite>kWh</span>"), "{page}");
    assert!(
        page.contains(&format!(
            r#"<option value="{electrique}" data-carburant="electricite" data-kilometrage="12000" selected>"#
        )),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<option value="electricite" data-unite="kWh" selected>Électricité</option>"#
        ),
        "{page}"
    );

    // Le plein s'enregistre, le message le dit, la fiche le montre et le compteur suit.
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=carburant&date_depense=2026-10-12&montant=68%2C40\
         &carburant=sp98&quantite=45%2C2&kilometrage=61200&plein_complet=oui&libelle=Station+fictive&notes="
    );
    let reponse = post_formulaire(&app, "/depenses", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], fiche.as_str());
    assert!(
        reponse.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("fait=plein-enregistre;"),
        "{:?}",
        reponse.headers()
    );
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains("45,2 L SP98 · 61\u{202F}200 km · Station fictive"),
        "{page}"
    );
    assert!(page.contains("68,40\u{202F}€"), "{page}");
    assert!(page.contains("61\u{202F}200 km"), "{page}");

    // Le péage aussi, avec son message.
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=peage&date_depense=2026-10-13&montant=12%2C30\
         &libelle=Lyon+%E2%80%93+Grenoble&notes=Retour"
    );
    let reponse = post_formulaire(&app, "/depenses", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(
        reponse.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("fait=peage-enregistre;"),
        "{:?}",
        reponse.headers()
    );

    // Un péage avec une quantité (formulaire sans script) : refusé, le formulaire revient
    // avec le motif et la saisie.
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=peage&date_depense=2026-10-13&montant=12%2C30\
         &quantite=10&kilometrage=&plein_complet=oui&libelle=Lyon&notes="
    );
    let reponse = post_formulaire(&app, "/depenses", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"role="alert""#) && page.contains("péage"),
        "{page}"
    );
    assert!(page.contains(r#"value="peage" selected"#), "{page}");
    assert!(page.contains(r#"name="montant" value="12,30""#), "{page}");
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=peage&date_depense=2026-10-13&montant=12%2C30\
         &carburant=gazole&libelle=Lyon&notes="
    );
    let reponse = post_formulaire(&app, "/depenses", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"role="alert""#) && page.contains("pas de carburant"),
        "{page}"
    );
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=peage&date_depense=2026-10-13&montant=12%2C30\
         &carburant=gazole&libelle=Lyon&notes="
    );
    let reponse = post_formulaire(&app, "/depenses", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"role="alert""#) && page.contains("pas de carburant"),
        "{page}"
    );

    // La liste, en entier puis restreinte au véhicule ; un véhicule inconnu est introuvable.
    let page = texte(get(&app, "/depenses", Some(&cookie)).await).await;
    assert!(page.contains("Lyon – Grenoble"), "{page}");
    assert!(page.contains("<small>Retour</small>"), "{page}");
    assert!(page.contains("Station fictive"), "{page}");
    assert!(page.contains("data-imprimer"), "{page}");
    assert!(
        page.contains(
            r#"<a class="bouton-discret export-csv" href="/depenses.csv">Exporter (CSV)</a>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(r#" class="actif" aria-current="page">Dépenses</a>"#),
        "{page}"
    );
    let page = texte(
        get(
            &app,
            &format!("/depenses?vehicule_id={vehicule}"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(page.contains("Dépenses de"), "{page}");
    assert!(
        page.contains(r#"<a href="/depenses">Toutes les dépenses</a>"#),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"<a class="retour" href="/vehicules/{vehicule}">DP-200-DP</a>"#
        )),
        "{page}"
    );
    assert_eq!(
        get(&app, "/depenses?vehicule_id=999999", Some(&cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    // Modifier : le formulaire remontre la saisie ; l'enregistrement a son message.
    let id = depenses::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.est_un_plein())
        .unwrap()
        .id;
    let page = texte(get(&app, &format!("/depenses/{id}/modifier"), Some(&cookie)).await).await;
    assert!(page.contains(r#"name="quantite" value="45,2""#), "{page}");
    assert!(
        page.contains(r#"<option value="sp98" data-unite="L" selected>SP98</option>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"name="kilometrage" value="61200""#),
        "{page}"
    );
    assert!(page.contains(r#"name="montant" value="68,40""#), "{page}");
    assert!(
        page.contains(r#"name="plein_complet" value="oui" checked"#),
        "{page}"
    );
    let corps = format!(
        "vehicule_id={vehicule}&type_depense=carburant&date_depense=2026-10-12&montant=70\
         &carburant=gazole&quantite=46&kilometrage=62000&libelle=&notes="
    );
    let reponse = post_formulaire(&app, &format!("/depenses/{id}"), Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(
        reponse.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("fait=depense-modifiee;"),
        "{:?}",
        reponse.headers()
    );
    let modifie = depenses::obtenir(&pool, id).await.unwrap();
    assert_eq!(modifie.montant_centimes, 7000);
    assert!(!modifie.plein_complet, "case décochée : non envoyée");
    assert_eq!(modifie.carburant, "gazole");
    assert_eq!(kilometrage(&pool, vehicule).await, 62000);

    // Supprimer : retour à la liste, avec le message.
    let reponse = post_formulaire(
        &app,
        &format!("/depenses/{id}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(reponse.headers()[header::LOCATION], "/depenses");
    assert!(
        reponse.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("fait=depense-supprimee;"),
        "{:?}",
        reponse.headers()
    );
    assert_eq!(
        get(&app, &format!("/depenses/{id}/modifier"), Some(&cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    // La fiche garde les dix dernières et renvoie à la liste pour le reste.
    for jour in 1..=11 {
        depenses::creer(
            &pool,
            &peage(vehicule, &format!("2026-09-{jour:02}"), "5", "Trajet"),
        )
        .await
        .unwrap();
    }
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert_eq!(page.matches(r#"<a href="/depenses/"#).count(), 10, "{page}");
    assert!(
        page.contains(&format!(
            r#"<a href="/depenses?vehicule_id={vehicule}">Toutes les dépenses (12)</a>"#
        )),
        "{page}"
    );
}

#[sqlx::test]
async fn l_api_lit_et_ecrit_les_depenses_et_le_lecteur_ne_fait_que_lire(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "DP-300-DP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let lecteur = se_connecter(&app, "lecture").await;

    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_depense":"carburant","date_depense":"2026-10-12",
            "montant":"68,40","carburant":"gazole","quantite":"45,2","kilometrage":"61200",
            "plein_complet":"true","libelle":"Station fictive"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/depenses",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let cree: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let id = cree["id"].as_i64().unwrap();
    assert_eq!(cree["type_depense"], "carburant");
    assert_eq!(cree["montant_centimes"], 6840);
    assert_eq!(cree["quantite_centiemes"], 4520);
    assert_eq!(cree["kilometrage"], 61200);
    assert_eq!(cree["plein_complet"], true);
    assert_eq!(cree["immatriculation"], "DP-300-DP");
    assert_eq!(cree["carburant"], "gazole");

    // Sans carburant, un plein est refusé ; un péage avec un carburant aussi, en français.
    for corps in [
        format!(
            r#"{{"vehicule_id":"{vehicule}","type_depense":"carburant","date_depense":"2026-10-12",
                "montant":"68,40","quantite":"45,2"}}"#
        ),
        format!(
            r#"{{"vehicule_id":"{vehicule}","type_depense":"peage","date_depense":"2026-10-13",
                "montant":"12,30","carburant":"gazole"}}"#
        ),
    ] {
        let reponse = requete_json(
            &app,
            "POST",
            "/api/v1/depenses",
            Some(&cookie),
            Some(&corps),
        )
        .await;
        assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
        let erreur: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
        assert!(
            erreur["erreur"].as_str().unwrap().contains("carburant"),
            "{erreur}"
        );
    }
    assert_eq!(kilometrage(&pool, vehicule).await, 61200);

    // Un péage avec une quantité est refusé, en français.
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_depense":"peage","date_depense":"2026-10-13",
            "montant":"12,30","quantite":"10"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/depenses",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    let erreur: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert!(
        erreur["erreur"].as_str().unwrap().contains("péage"),
        "{erreur}"
    );
    // Sans la quantité, il passe ; la case « plein complet » n'a pas d'objet et vaut faux.
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_depense":"peage","date_depense":"2026-10-13",
            "montant":"12,30","libelle":"Lyon – Grenoble","plein_complet":"oui"}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/depenses",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let peage: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(peage["plein_complet"], false);
    assert_eq!(peage["kilometrage"], serde_json::Value::Null);

    // Le lecteur lit, par l'API comme par les pages, et n'écrit rien.
    let reponse = requete_json(&app, "GET", "/api/v1/depenses", Some(&lecteur), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let lues: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(lues.as_array().unwrap().len(), 2);
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/depenses/{id}"),
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    for (methode, chemin, corps) in [
        ("POST", "/api/v1/depenses".to_string(), Some("{}")),
        ("PUT", format!("/api/v1/depenses/{id}"), Some("{}")),
        ("DELETE", format!("/api/v1/depenses/{id}"), None),
    ] {
        let reponse = requete_json(&app, methode, &chemin, Some(&lecteur), corps).await;
        assert_eq!(
            reponse.status(),
            StatusCode::FORBIDDEN,
            "{methode} {chemin}"
        );
    }
    assert_eq!(
        get(&app, "/depenses/nouvelle", Some(&lecteur))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_formulaire(&app, "/depenses", Some(&lecteur), "")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let page = texte(get(&app, "/depenses", Some(&lecteur)).await).await;
    assert!(page.contains("Lyon – Grenoble"), "{page}");
    assert!(
        !page.contains("Modifier") && !page.contains("Nouvelle dépense"),
        "{page}"
    );
    let page = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&lecteur)).await).await;
    assert!(!page.contains("Noter un plein"), "{page}");
    assert!(page.contains("Station fictive"), "{page}");

    // L'administrateur modifie, puis supprime ; un identifiant inconnu est introuvable.
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_depense":"carburant","date_depense":"2026-10-12",
            "montant":"70","carburant":"e85","quantite":"46","kilometrage":"62000","plein_complet":"non"}}"#
    );
    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/depenses/{id}"),
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let modifie: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(modifie["montant_centimes"], 7000);
    assert_eq!(modifie["plein_complet"], false);
    assert_eq!(modifie["carburant"], "e85");
    assert_eq!(kilometrage(&pool, vehicule).await, 62000);
    let reponse = requete_json(
        &app,
        "DELETE",
        &format!("/api/v1/depenses/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/depenses/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn les_deux_marques_menent_aux_depenses_et_le_csv_les_exporte(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "DP-400-DP").await;
    depenses::creer(
        &pool,
        &plein(vehicule, "2026-10-12", "68,40", "45,2", "61200"),
    )
    .await
    .unwrap();
    depenses::creer(
        &pool,
        &peage(vehicule, "2026-10-13", "12,30", "Lyon – Grenoble"),
    )
    .await
    .unwrap();

    // Autoclair Pro : une entrée de la barre latérale, allumée sur la liste.
    let pro = application(pool.clone());
    let cookie = se_connecter(&pro, "gestion").await;
    let page = texte(get(&pro, "/", Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<a href="/depenses" data-ico="carburant">Dépenses</a>"#),
        "{page}"
    );
    let page = texte(get(&pro, "/depenses", Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<a href="/depenses" data-ico="carburant" class="actif" aria-current="page">Dépenses</a>"#),
        "{page}"
    );
    assert_eq!(page.matches("aria-current").count(), 1, "{page}");

    // Autoclair : pas dans les quatre onglets, mais dans « Plus », et la liste allume « Plus ».
    let part = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie_part = se_connecter(&part, "gestion").await;
    let page = texte(get(&part, "/", Some(&cookie_part)).await).await;
    assert!(!page.contains(r#"href="/depenses""#), "{page}");
    let page = texte(get(&part, "/plus", Some(&cookie_part)).await).await;
    assert!(
        page.contains(r#"<a href="/depenses" data-ico="carburant">Dépenses</a>"#),
        "{page}"
    );
    let page = texte(get(&part, "/depenses", Some(&cookie_part)).await).await;
    assert!(
        page.contains(r#" class="actif" aria-current="page">Plus</a>"#),
        "{page}"
    );
    assert_eq!(page.matches("aria-current").count(), 1, "{page}");
    // Et pas de lien d'export chez un particulier.
    assert!(!page.contains("Exporter (CSV)"), "{page}");

    // Le CSV : quantité et unité à part, « plein complet » en oui/non, rien pour un péage.
    let reponse = get(&pro, "/depenses.csv", Some(&cookie)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let disposition = reponse.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.starts_with("attachment; filename=\"depenses-20"),
        "{disposition}"
    );
    let csv = texte(reponse).await;
    assert_eq!(
        csv,
        "\u{FEFF}Date;Véhicule;Type;Carburant;Quantité;Unité;Kilométrage (km);Consommation (aux 100 km);Plein complet;Montant (€);Station ou trajet;Notes\r\n\
         13/10/2026;DP-400-DP;Péage;;;;;;;12,30;Lyon – Grenoble;\r\n\
         12/10/2026;DP-400-DP;Carburant;Gazole (B7);45,2;L;61200;;oui;68,40;Station fictive;\r\n"
    );

    // L'icône et le script existent bien.
    let style = include_str!("../static/style.css");
    assert!(style.contains("--ico-carburant:") && style.contains(r#"[data-ico="carburant"]"#));
    let script = include_str!("../static/app.js");
    assert!(
        script.contains("select[name=type_depense]")
            && script.contains("select[name=carburant]")
            && script.contains("data-unite")
    );
    // L'étiquette d'unité se vise par sa balise (#131) : `[data-unite]` seul tomberait sur la
    // première option de la liste des carburants, qui porte le même attribut et vient avant.
    assert!(
        script.contains(r#"querySelector("span[data-unite]")"#)
            && !script.contains(r#"querySelector("[data-unite]")"#),
        "le script doit viser span[data-unite]"
    );
}

/// La migration `20261021160000` (#128) sur une base qui avait déjà des pleins : le bloc 1
/// appliqué, des pleins sans carburant insérés, puis la migration ; chaque plein reçoit le
/// carburant proposé d'après son véhicule, le péage rien, et la contrainte tient ensuite.
#[sqlx::test(migrations = false)]
async fn la_migration_donne_un_carburant_aux_pleins_deja_notes(pool: PgPool) {
    const CREATION_DES_DEPENSES: i64 = 20261021150000;
    let migrateur = sqlx::migrate!("./migrations");
    for m in migrateur
        .iter()
        .filter(|m| m.version <= CREATION_DES_DEPENSES)
    {
        sqlx::raw_sql(&m.sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("migration {} : {e}", m.version));
    }
    let vehicules: Vec<i64> = sqlx::query_scalar(
        "INSERT INTO vehicules
            (immatriculation, marque, modele, date_mise_en_service, carburant, kilometrage, statut)
         VALUES ('MI-100-DI', 'R', 'K', '2023-06-01', 'diesel', 1, 'en_service'),
                ('MI-101-ES', 'R', 'C', '2023-06-01', 'essence', 1, 'en_service'),
                ('MI-102-EL', 'R', 'Z', '2023-06-01', 'electrique', 1, 'en_service')
         RETURNING id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for (vehicule, type_depense) in [
        (vehicules[0], "carburant"),
        (vehicules[1], "carburant"),
        (vehicules[2], "carburant"),
        (vehicules[0], "peage"),
    ] {
        sqlx::query(
            "INSERT INTO depenses (vehicule_id, type_depense, date_depense, montant_centimes)
             VALUES ($1, $2, '2026-10-12', 100)",
        )
        .bind(vehicule)
        .bind(type_depense)
        .execute(&pool)
        .await
        .unwrap();
    }

    for m in migrateur
        .iter()
        .filter(|m| m.version > CREATION_DES_DEPENSES)
    {
        sqlx::raw_sql(&m.sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("migration {} : {e}", m.version));
    }
    let carburants: Vec<(String, String)> =
        sqlx::query_as("SELECT type_depense, carburant FROM depenses ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        carburants,
        [
            ("carburant".to_string(), "gazole".to_string()),
            ("carburant".to_string(), "sp95_e10".to_string()),
            ("carburant".to_string(), "electricite".to_string()),
            ("peage".to_string(), String::new()),
        ]
    );
    let erreur = sqlx::query(
        "INSERT INTO depenses (vehicule_id, type_depense, date_depense)
         VALUES ($1, 'carburant', '2026-10-13')",
    )
    .bind(vehicules[0])
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(a_le_code(&erreur, CODE_VIOLATION_CHECK), "{erreur}");
}

/// Le cookie du message d'action posé par une redirection (`fait=…`).
fn fait_pose(reponse: &axum::http::Response<axum::body::Body>, code: &str) -> bool {
    reponse
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|v| v.to_str().unwrap().starts_with(&format!("fait={code};")))
}

fn position(page: &str, texte: &str) -> usize {
    page.find(texte)
        .unwrap_or_else(|| panic!("« {texte} » absent de la page :\n{page}"))
}

/// Le ticket d'une dépense, joint depuis le formulaire (#126, bloc 2) : un document du véhicule
/// relié à la dépense, du type « ticket », rangé dans le dossier « Dépenses » de la fiche, qui
/// suit la dépense sur la fiche, dans la liste et dans l'API, et qui reste quand elle disparaît.
#[sqlx::test]
async fn le_ticket_se_joint_depuis_le_formulaire_et_suit_la_depense(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "TK-126-TK").await;
    let autre_vehicule = creer_vehicule(&pool, "TK-127-TK").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let vehicule_texte = vehicule.to_string();
    let champs = [
        ("vehicule_id", vehicule_texte.as_str()),
        ("type_depense", "carburant"),
        ("date_depense", "2026-10-12"),
        ("montant", "68,40"),
        ("carburant", "gazole"),
        ("quantite", "45,2"),
        ("kilometrage", "61200"),
        ("plein_complet", "oui"),
        ("libelle", "Station fictive"),
        ("notes", ""),
    ];

    // Un ticket vide est refusé avant d'écrire quoi que ce soit : le formulaire revient avec sa
    // saisie, dit que le ticket est à rechoisir, et aucune dépense n'existe.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/depenses",
        "ticket",
        &champs,
        &[("ticket-vide.jpg", "image/jpeg", b"")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(page.contains("Le fichier est vide."), "{page}");
    assert!(page.contains("choisissez-le de nouveau"), "{page}");
    assert!(page.contains(r#"value="Station fictive""#), "{page}");
    assert!(depenses::lister(&pool).await.unwrap().is_empty());

    // Création avec le ticket (une image fictive) : la dépense, le document relié, le message.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/depenses",
        "ticket",
        &champs,
        &[("ticket-1012.jpg", "image/jpeg", b"une image, pour le test")],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    assert!(
        fait_pose(&reponse, "plein-enregistre-ticket"),
        "{:?}",
        reponse.headers()
    );
    let id = depenses::lister(&pool).await.unwrap()[0].id;
    let creee = depenses::obtenir(&pool, id).await.unwrap();
    assert_eq!(creee.documents.len(), 1, "{creee:?}");
    let ticket = creee.documents[0].clone();
    assert_eq!(
        (
            ticket.type_document.as_str(),
            ticket.vehicule_id,
            ticket.depense_id,
            ticket.intervention_id,
            ticket.nom_original.as_str()
        ),
        ("ticket", Some(vehicule), Some(id), None, "ticket-1012.jpg")
    );

    // La fiche et la liste mènent au ticket ; la fiche le range dans le dossier « Dépenses » ;
    // sa page nomme le plein et y mène ; l'API le porte.
    let lien = format!(r#"<a href="/documents/{}">Ticket</a>"#, ticket.id);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(&lien), "{fiche}");
    assert!(
        position(&fiche, "<h3>Dépenses</h3>") < position(&fiche, "ticket-1012.jpg")
            && position(&fiche, "ticket-1012.jpg") < position(&fiche, "<h3>Autres</h3>"),
        "{fiche}"
    );
    let liste = texte(get(&app, "/depenses", Some(&cookie)).await).await;
    assert!(liste.contains(&lien), "{liste}");
    let page = texte(get(&app, &format!("/documents/{}", ticket.id), Some(&cookie)).await).await;
    assert!(
        page.contains(&format!(
            r#"<a href="/depenses/{id}/modifier">Plein du 12/10/2026</a>"#
        )),
        "{page}"
    );
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/depenses/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(json["documents"][0]["depense_id"], id, "{json}");

    // Le formulaire de modification rappelle le ticket joint et en accepte un second (un PDF).
    let formulaire =
        texte(get(&app, &format!("/depenses/{id}/modifier"), Some(&cookie)).await).await;
    assert!(
        formulaire.contains(r#"enctype="multipart/form-data""#)
            && formulaire.contains(&format!(r#"href="/documents/{}""#, ticket.id)),
        "{formulaire}"
    );
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        &format!("/depenses/{id}"),
        "ticket",
        &champs,
        &[("ticket.pdf", "application/pdf", b"%PDF-1.4 ticket fictif")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(fait_pose(&reponse, "depense-modifiee-ticket"));
    assert_eq!(
        depenses::obtenir(&pool, id).await.unwrap().documents.len(),
        2
    );

    // Sans ticket (le navigateur envoie le champ vide) : rien de plus, le message ordinaire.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        &format!("/depenses/{id}"),
        "ticket",
        &champs,
        &[("", "application/octet-stream", b"")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert!(fait_pose(&reponse, "depense-modifiee"));
    assert_eq!(
        depenses::obtenir(&pool, id).await.unwrap().documents.len(),
        2
    );

    // « Ajouter » depuis le dossier « Dépenses » ne propose que le ticket ; le dépôt depuis la
    // fiche avec `depense_id` relie le document, sauf si la dépense est celle d'un autre véhicule.
    let page = texte(
        get(
            &app,
            &format!("/documents/nouveau?vehicule_id={vehicule}&dossier=depenses"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(
            r#"<option value="ticket" selected>Ticket de carburant ou de péage</option>"#
        ),
        "{page}"
    );
    let id_texte = id.to_string();
    let reponse = post_multipart(
        &app,
        &cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[
            ("type_document", "ticket"),
            ("depense_id", id_texte.as_str()),
        ],
        &[("ticket-3.jpg", "image/jpeg", b"une autre image")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        depenses::obtenir(&pool, id).await.unwrap().documents.len(),
        3
    );
    let reponse = post_multipart(
        &app,
        &cookie,
        &format!("/vehicules/{autre_vehicule}/documents"),
        &[
            ("type_document", "ticket"),
            ("depense_id", id_texte.as_str()),
        ],
        &[("ticket-4.jpg", "image/jpeg", b"une image de trop")],
    )
    .await;
    let page = texte(reponse).await;
    assert!(
        page.contains("La dépense indiquée n&#39;est pas une dépense de ce véhicule."),
        "{page}"
    );
    assert_eq!(
        depenses::obtenir(&pool, id).await.unwrap().documents.len(),
        3
    );

    // Supprimer la dépense garde ses tickets, sans dépense ; leur page ne nomme plus rien.
    let reponse = post_formulaire(
        &app,
        &format!("/depenses/{id}/supprimer"),
        Some(&cookie),
        "",
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let restants: Vec<Option<i64>> =
        sqlx::query_scalar("SELECT depense_id FROM documents WHERE vehicule_id = $1 ORDER BY id")
            .bind(vehicule)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(restants, vec![None, None, None]);
    let page = texte(get(&app, &format!("/documents/{}", ticket.id), Some(&cookie)).await).await;
    assert!(!page.contains("Plein du"), "{page}");
}

// --- Bloc 3 de #126 : ce que ça coûte, et ce que ça consomme --------------------------------

/// Un plein partiel : la case « plein complet » décochée.
fn partiel(mut saisie: SaisieDepense) -> SaisieDepense {
    saisie.plein_complet = String::new();
    saisie
}

/// La consommation sur la ligne du plein, et le résumé de la fiche (totaux, consommation
/// moyenne, coût au kilomètre) : sur la page, pour un lecteur aussi, et par l'API. Des dates
/// relatives à aujourd'hui : les douze derniers mois ne dépendent pas de l'année du test.
#[sqlx::test]
async fn la_fiche_et_l_api_resument_ce_que_le_vehicule_coute_et_consomme(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "CO-300-CO").await;
    let sans_depense = creer_vehicule(&pool, "CO-301-CO").await;
    let aujourd_hui = chrono::Local::now().date_naive();
    let jour = |en_arriere: i64| (aujourd_hui - chrono::Duration::days(en_arriere)).to_string();

    // Saisis dans le désordre : le plus récent d'abord. 50 L à 10 000 km, 20 L (partiel) à
    // 10 300 km, 30 L à 10 650 km : 50 L sur 650 km = 7,7 L/100 km.
    depenses::creer(&pool, &plein(vehicule, &jour(1), "45,00", "30", "10650"))
        .await
        .unwrap();
    depenses::creer(&pool, &plein(vehicule, &jour(6), "70,00", "50", "10000"))
        .await
        .unwrap();
    depenses::creer(
        &pool,
        &partiel(plein(vehicule, &jour(4), "30,00", "20", "10300")),
    )
    .await
    .unwrap();
    depenses::creer(
        &pool,
        &peage(vehicule, &jour(4), "10,00", "Lyon – Grenoble"),
    )
    .await
    .unwrap();

    let pro = application(pool.clone());
    let admin = se_connecter(&pro, "gestion").await;
    let lecteur = se_connecter(&pro, "lecture").await;
    let fiche = format!("/vehicules/{vehicule}");
    for cookie in [&admin, &lecteur] {
        let page = texte(get(&pro, &fiche, Some(cookie)).await).await;
        // Sur la ligne du plein qui clôt l'intervalle, et en moyenne dans le résumé.
        assert_eq!(page.matches("7,7 L/100\u{202F}km").count(), 2, "{page}");
        assert!(page.contains("<dt>Consommation moyenne</dt>"), "{page}");
        assert!(
            page.contains("sur 650\u{202F}km, 1 plein complet mesuré"),
            "{page}"
        );
        // Le plein partiel et le premier plein n'ont pas de consommation à eux.
        assert!(!page.contains("10\u{202F}300 km · 7"), "{page}");
        // Les totaux : 70 + 30 + 45 = 145 € de carburant, 10 € de péages.
        assert!(
            page.contains(&format!(
                "<dt>Cette année ({})</dt>",
                aujourd_hui.format("%Y")
            )),
            "{page}"
        );
        assert!(page.contains("<dt>Douze derniers mois</dt>"), "{page}");
        assert!(
            page.contains(
                "Carburant 145,00\u{202F}€&nbsp;· Péages 10,00\u{202F}€&nbsp;· <strong>Total 155,00\u{202F}€</strong>"
            ),
            "{page}"
        );
        // Coût au kilomètre : 30 + 10 + 45 = 85 € sur 650 km (le plein du premier relevé,
        // qui remplit ce qui a été roulé avant, n'en est pas) = 0,13 €/km.
        assert!(page.contains("<dt>Coût au kilomètre</dt>"), "{page}");
        assert!(page.contains("0,13\u{202F}€/km"), "{page}");
    }
    // Un véhicule sans dépense n'a pas de résumé.
    let page = texte(get(&pro, &format!("/vehicules/{sans_depense}"), Some(&admin)).await).await;
    assert!(page.contains("Aucune dépense."), "{page}");
    assert!(!page.contains("Consommation moyenne"), "{page}");

    // L'API : le résumé, et la consommation de chaque plein dans la liste.
    let reponse = requete_json(
        &pro,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/depenses/resume"),
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let resume: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(resume["nombre_de_depenses"], 4, "{resume}");
    assert_eq!(resume["douze_derniers_mois"]["carburant_centimes"], 14_500);
    assert_eq!(resume["douze_derniers_mois"]["peage_centimes"], 1_000);
    assert_eq!(resume["consommations"][0]["unite"], "L");
    assert_eq!(resume["consommations"][0]["dixiemes_aux_100_km"], 77);
    assert_eq!(resume["cout_au_km"]["kilometres"], 650);
    assert_eq!(resume["cout_au_km"]["depenses_centimes"], 8_500);
    assert_eq!(resume["cout_au_km"]["centimes_aux_100_km"], 1_308);
    let reponse = requete_json(
        &pro,
        "GET",
        "/api/v1/vehicules/999999/depenses/resume",
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::NOT_FOUND);
    let reponse = requete_json(
        &pro,
        "GET",
        &format!("/api/v1/depenses?vehicule_id={vehicule}"),
        Some(&lecteur),
        None,
    )
    .await;
    let liste: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let liste = liste.as_array().unwrap();
    assert_eq!(liste.len(), 4);
    let par_km = |km: i64| liste.iter().find(|d| d["kilometrage"] == km).unwrap();
    assert_eq!(par_km(10_650)["consommation"]["dixiemes_aux_100_km"], 77);
    assert_eq!(par_km(10_650)["consommation"]["kilometres"], 650);
    assert!(par_km(10_000)["consommation"].is_null());
    assert!(par_km(10_300)["consommation"].is_null());
}

fn ligne_d_entretien(operation: &str, quantite: &str, prix: &str) -> SaisieLigne {
    SaisieLigne {
        operation: operation.to_string(),
        libelle: String::new(),
        reference: String::new(),
        quantite: quantite.to_string(),
        prix: prix.to_string(),
    }
}

fn entretien(
    vehicule: i64,
    date: &str,
    kilometrage: &str,
    cout: &str,
    lignes: Vec<SaisieLigne>,
) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".to_string(),
        date_intervention: date.to_string(),
        kilometrage: kilometrage.to_string(),
        cout: cout.to_string(),
        prestataire: "Garage fictif".to_string(),
        lignes,
        ..Default::default()
    }
}

/// Le bloc « Coût » de la fiche additionne l'entretien (dont les pièces), le carburant et les
/// péages, par la page comme par l'API (#94, bloc 6) : une facture dit le coût d'une intervention,
/// les prix de ses lignes disent les pièces, et un coût oublié ne fait pas disparaître des prix
/// saisis. Le coût au kilomètre prend l'entretien, sans celui du premier relevé. Un véhicule qui
/// n'a que des interventions a un coût, un autre dont les interventions sont gratuites n'en a pas.
#[sqlx::test]
async fn la_fiche_et_l_api_additionnent_l_entretien_les_pleins_et_les_peages(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "CO-400-CO").await;
    let seulement_entretien = creer_vehicule(&pool, "CO-401-CO").await;
    let gratuit = creer_vehicule(&pool, "CO-402-CO").await;
    let aujourd_hui = chrono::Local::now().date_naive();
    let jour = |en_arriere: i64| (aujourd_hui - chrono::Duration::days(en_arriere)).to_string();

    // Saisies dans le désordre. Le premier relevé est la révision d'il y a 40 jours : son coût
    // n'entre pas dans le coût au kilomètre.
    interventions::creer(
        &pool,
        &entretien(
            vehicule,
            &jour(20),
            "10500",
            "0",
            vec![ligne_d_entretien("pneus", "4", "300,00")],
        ),
    )
    .await
    .unwrap();
    interventions::creer(
        &pool,
        &entretien(
            vehicule,
            &jour(40),
            "10000",
            "180,00",
            vec![
                ligne_d_entretien("vidange", "5,4", "45,00"),
                ligne_d_entretien("filtre_huile", "1", "12,50"),
            ],
        ),
    )
    .await
    .unwrap();
    depenses::creer(&pool, &plein(vehicule, &jour(10), "80,00", "50", "11000"))
        .await
        .unwrap();
    depenses::creer(
        &pool,
        &peage(vehicule, &jour(15), "10,00", "Lyon – Grenoble"),
    )
    .await
    .unwrap();
    interventions::creer(
        &pool,
        &entretien(seulement_entretien, &jour(30), "5000", "100,00", Vec::new()),
    )
    .await
    .unwrap();
    interventions::creer(
        &pool,
        &entretien(gratuit, &jour(30), "5000", "0", Vec::new()),
    )
    .await
    .unwrap();

    let pro = application(pool.clone());
    let admin = se_connecter(&pro, "gestion").await;
    let lecteur = se_connecter(&pro, "lecture").await;
    for cookie in [&admin, &lecteur] {
        let page = texte(get(&pro, &format!("/vehicules/{vehicule}"), Some(cookie)).await).await;
        assert!(page.contains(r#"<h2 data-ico="euro">Coût</h2>"#), "{page}");
        // 180 € de facture + 300 € de pièces dont le coût a été oublié = 480 € d'entretien, dont
        // 57,50 + 300 = 357,50 € de pièces ; le reste, 122,50 €. Avec 80 € de carburant et
        // 10 € de péages : 570 €.
        assert!(
            page.contains(
                "Entretien 480,00\u{202F}€ (pièces 357,50\u{202F}€&nbsp;· reste 122,50\u{202F}€)&nbsp;· Carburant 80,00\u{202F}€&nbsp;· Péages 10,00\u{202F}€&nbsp;· <strong>Total 570,00\u{202F}€</strong>"
            ),
            "{page}"
        );
        // Coût au kilomètre : de 10 000 km (la révision, qui n'en est pas) à 11 000 km (le plein),
        // 300 + 10 + 80 = 390 € sur 1 000 km = 0,39 €/km.
        assert!(page.contains("<dt>Coût au kilomètre</dt>"), "{page}");
        assert!(page.contains("0,39\u{202F}€/km"), "{page}");
        assert!(page.contains("entretien compris"), "{page}");
        assert!(page.contains("« Pièces » : les prix saisis"), "{page}");
    }

    // Des interventions sans dépense : le coût est là, sans séparation pièces et reste (aucun prix
    // de pièce saisi), et la liste des dépenses dit qu'il n'y en a pas.
    let page = texte(
        get(
            &pro,
            &format!("/vehicules/{seulement_entretien}"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(page.contains(r#"<h2 data-ico="euro">Coût</h2>"#), "{page}");
    assert!(page.contains("Aucune dépense."), "{page}");
    assert!(
        page.contains(
            "Entretien 100,00\u{202F}€&nbsp;· Carburant 0,00\u{202F}€&nbsp;· Péages 0,00\u{202F}€"
        ),
        "{page}"
    );
    assert!(!page.contains("(pièces"), "{page}");
    // Des interventions qui n'ont rien coûté : pas de coût à montrer.
    let page = texte(get(&pro, &format!("/vehicules/{gratuit}"), Some(&admin)).await).await;
    assert!(!page.contains(r#"<h2 data-ico="euro">"#), "{page}");

    // L'API : le même résumé.
    let reponse = requete_json(
        &pro,
        "GET",
        &format!("/api/v1/vehicules/{vehicule}/depenses/resume"),
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let resume: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(resume["nombre_d_interventions"], 2, "{resume}");
    assert_eq!(resume["nombre_de_depenses"], 2, "{resume}");
    let douze = &resume["douze_derniers_mois"];
    assert_eq!(douze["entretien_centimes"], 48_000, "{resume}");
    assert_eq!(douze["pieces_centimes"], 35_750, "{resume}");
    assert_eq!(douze["carburant_centimes"], 8_000, "{resume}");
    assert_eq!(douze["peage_centimes"], 1_000, "{resume}");
    assert_eq!(resume["cout_au_km"]["kilometres"], 1_000, "{resume}");
    assert_eq!(
        resume["cout_au_km"]["entretien_centimes"], 30_000,
        "{resume}"
    );
    assert_eq!(resume["cout_au_km"]["depenses_centimes"], 9_000, "{resume}");
    assert_eq!(
        resume["cout_au_km"]["centimes_aux_100_km"], 3_900,
        "{resume}"
    );
}

/// La liste se filtre par véhicule et par période ; ses totaux sont ceux de ce qui est affiché ;
/// l'export est celui du filtre ; un filtre illisible ne casse pas la page et est refusé par
/// l'export ; la consommation d'un plein se mesure sur un plein d'avant la période.
#[sqlx::test]
async fn la_liste_se_filtre_par_vehicule_et_par_periode_et_additionne_ce_qui_est_affiche(
    pool: PgPool,
) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let a = creer_vehicule(&pool, "FI-100-FI").await;
    let b = creer_vehicule(&pool, "FI-200-FI").await;
    // Véhicule A : un plein en mars (la borne), un plein et un péage en avril. Véhicule B : un
    // plein en avril.
    depenses::creer(&pool, &plein(a, "2025-03-10", "70,00", "50", "10000"))
        .await
        .unwrap();
    depenses::creer(&pool, &plein(a, "2025-04-10", "60,00", "40", "10650"))
        .await
        .unwrap();
    depenses::creer(&pool, &peage(a, "2025-04-12", "12,30", "Lyon – Grenoble"))
        .await
        .unwrap();
    depenses::creer(&pool, &plein(b, "2025-04-20", "50,00", "30", "500"))
        .await
        .unwrap();

    let pro = application(pool.clone());
    let admin = se_connecter(&pro, "gestion").await;
    let lecteur = se_connecter(&pro, "lecture").await;
    let ligne_de = |page: &str, id: i64| page.contains(&format!(r#"<a href="/vehicules/{id}">"#));

    // Sans filtre : tout, et le pied de liste additionne tout. Les raccourcis sont là.
    let page = texte(get(&pro, "/depenses", Some(&admin)).await).await;
    assert!(ligne_de(&page, a) && ligne_de(&page, b), "{page}");
    assert!(
        page.contains(
            "4 dépenses · Carburant <strong>180,00\u{202F}€</strong> · Péages <strong>12,30\u{202F}€</strong> · Total <strong>192,30\u{202F}€</strong>"
        ),
        "{page}"
    );
    for raccourci in ["Ce mois", "Cette année", "Douze derniers mois", "Tout"] {
        assert!(page.contains(raccourci), "{raccourci}\n{page}");
    }
    let aujourd_hui = chrono::Local::now().date_naive();
    let (du_mois, au_mois) = gestion_parc_auto::metier::consommation::mois_en_cours(aujourd_hui);
    assert!(
        page.contains(&format!(
            r#"href="/depenses?du={du_mois}&#38;au={au_mois}""#
        )),
        "{page}"
    );
    assert!(page.contains(r#"href="/depenses.csv""#), "{page}");

    // Avril, tous véhicules : trois dépenses, le plein de mars n'y est pas, mais la consommation
    // du plein d'avril de A se mesure quand même (40 L sur 650 km = 6,2).
    let page = texte(
        get(
            &pro,
            "/depenses?du=2025-04-01&au=2025-04-30",
            Some(&lecteur),
        )
        .await,
    )
    .await;
    assert!(!page.contains("10/03/2025"), "{page}");
    assert!(
        page.contains("10/04/2025") && page.contains("20/04/2025"),
        "{page}"
    );
    assert!(page.contains("6,2 L/100\u{202F}km"), "{page}");
    assert!(
        page.contains(
            "3 dépenses · Carburant <strong>110,00\u{202F}€</strong> · Péages <strong>12,30\u{202F}€</strong> · Total <strong>122,30\u{202F}€</strong>"
        ),
        "{page}"
    );
    assert!(
        page.contains("Période : du 01/04/2025 au 30/04/2025."),
        "{page}"
    );
    assert!(
        page.contains(r#"href="/depenses.csv?du=2025-04-01&#38;au=2025-04-30""#),
        "{page}"
    );
    // Les champs du filtre rappellent ce qui est filtré.
    assert!(page.contains(r#"name="du" value="2025-04-01""#), "{page}");
    assert!(page.contains(r#"name="au" value="2025-04-30""#), "{page}");

    // Un véhicule, avril : deux dépenses, l'autre véhicule disparaît, le lien d'export suit.
    let page = texte(
        get(
            &pro,
            &format!("/depenses?vehicule_id={a}&du=2025-04-01&au=2025-04-30"),
            Some(&admin),
        )
        .await,
    )
    .await;
    assert!(ligne_de(&page, a) && !ligne_de(&page, b), "{page}");
    assert!(
        page.contains("2 dépenses · Carburant <strong>60,00\u{202F}€</strong>"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            r#"href="/depenses.csv?vehicule_id={a}&#38;du=2025-04-01&#38;au=2025-04-30""#
        )),
        "{page}"
    );
    assert!(
        page.contains(&format!(r#"<option value="{a}" selected>"#)),
        "{page}"
    );

    // Une seule borne suffit ; un filtre sans résultat garde le formulaire et le dit.
    let page = texte(get(&pro, "/depenses?du=2025-04-15", Some(&admin)).await).await;
    assert!(
        page.contains("depuis le 15/04/2025") && page.contains("20/04/2025"),
        "{page}"
    );
    assert!(!page.contains("12/04/2025"), "{page}");
    let page = texte(get(&pro, "/depenses?du=2030-01-01&au=2030-12-31", Some(&admin)).await).await;
    assert!(page.contains("Aucune dépense pour ce filtre."), "{page}");
    assert!(page.contains(r#"class="filtres""#), "{page}");
    assert!(
        page.contains(r#"<a href="/depenses">Effacer le filtre</a>"#),
        "{page}"
    );

    // Un filtre illisible ne casse pas la page : tout s'affiche, avec le motif.
    let reponse = get(&pro, "/depenses?du=hier", Some(&admin)).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"class="message-erreur""#) && page.contains("« du »"),
        "{page}"
    );
    assert!(ligne_de(&page, a) && ligne_de(&page, b), "{page}");
    let page = texte(get(&pro, "/depenses?du=2025-05-01&au=2025-04-01", Some(&admin)).await).await;
    assert!(page.contains("commence après sa fin"), "{page}");

    // L'export est celui du filtre : l'avril de A, avec sa consommation ; rien d'une autre période.
    let reponse = get(
        &pro,
        &format!("/depenses.csv?vehicule_id={a}&du=2025-04-01&au=2025-04-30"),
        Some(&admin),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let csv = texte(reponse).await;
    assert!(
        csv.contains(
            "10/04/2025;FI-100-FI;Carburant;Gazole (B7);40;L;10650;6,2;oui;60,00;Station fictive;"
        ),
        "{csv}"
    );
    assert!(csv.contains("12/04/2025;FI-100-FI;Péage"), "{csv}");
    assert!(
        !csv.contains("10/03/2025") && !csv.contains("FI-200-FI"),
        "{csv}"
    );
    assert_eq!(csv.lines().count(), 3, "l'en-tête et deux lignes : {csv}");
    // Sans filtre : tout. Avec un filtre illisible : refusé, pas ignoré.
    let csv = texte(get(&pro, "/depenses.csv", Some(&admin)).await).await;
    assert_eq!(csv.lines().count(), 5, "{csv}");
    let reponse = get(&pro, "/depenses.csv?du=hier", Some(&admin)).await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert!(texte(reponse).await.contains("« du »"));

    // Les raccourcis ne s'impriment pas (le filtre non plus) : la phrase « Période : … » suffit.
    let style = include_str!("../static/style.css");
    assert!(
        style.contains(".carte-actions, form, .aide, .raccourcis { display: none !important; }"),
        "la règle d'impression"
    );

    // L'API accepte le même filtre et le refuse de même.
    let reponse = requete_json(
        &pro,
        "GET",
        "/api/v1/depenses?du=2025-04-01&au=2025-04-30",
        Some(&lecteur),
        None,
    )
    .await;
    let liste: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(liste.as_array().unwrap().len(), 3);
    let reponse = requete_json(
        &pro,
        "GET",
        "/api/v1/depenses?au=31%2F10%2F2026",
        Some(&lecteur),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
}

/// Le chiffre « Dépenses du mois » du tableau de bord d'Autoclair Pro ; l'accueil d'Autoclair ne
/// change pas.
#[sqlx::test]
async fn le_tableau_de_bord_pro_montre_les_depenses_du_mois(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "TB-100-TB").await;
    let pro = application(pool.clone());
    let cookie = se_connecter(&pro, "gestion").await;
    let aujourd_hui = chrono::Local::now().date_naive();
    let (du, au) = gestion_parc_auto::metier::consommation::mois_en_cours(aujourd_hui);
    // L'adresse est écrite dans le gabarit : `&amp;`, pas l'échappement d'une valeur.
    let lien = format!(r#"<a href="/depenses?du={du}&amp;au={au}">"#);

    // Rien ce mois-ci : le chiffre dit 0 €.
    let page = texte(get(&pro, "/", Some(&cookie)).await).await;
    assert!(page.contains(&lien), "{page}");
    assert!(
        page.contains("<strong>0\u{202F}€</strong><span>Dépenses du mois</span>"),
        "{page}"
    );

    // Une dépense aujourd'hui, un péage aujourd'hui, une du mois dernier (hors du chiffre).
    depenses::creer(
        &pool,
        &plein(vehicule, &aujourd_hui.to_string(), "123,45", "40", ""),
    )
    .await
    .unwrap();
    depenses::creer(
        &pool,
        &peage(
            vehicule,
            &aujourd_hui.to_string(),
            "10,00",
            "Lyon – Grenoble",
        ),
    )
    .await
    .unwrap();
    let ancien = aujourd_hui - chrono::Duration::days(70);
    depenses::creer(
        &pool,
        &plein(vehicule, &ancien.to_string(), "500,00", "40", ""),
    )
    .await
    .unwrap();
    let page = texte(get(&pro, "/", Some(&cookie)).await).await;
    // 123,45 + 10,00 = 133,45 € : 133 € en euros ronds.
    assert!(
        page.contains("<strong>133\u{202F}€</strong><span>Dépenses du mois</span>"),
        "{page}"
    );

    // Autoclair : l'accueil ne change pas.
    let particulier = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&particulier, "gestion").await;
    let page = texte(get(&particulier, "/", Some(&cookie)).await).await;
    assert!(!page.contains("Dépenses du mois"), "{page}");
}
