//! Les dépenses pour rouler (#126, bloc 1) : les pleins de carburant et les tickets de péage, de
//! la saisie à la fiche, par le formulaire HTML et par l'API JSON ; le compteur du véhicule qui
//! suit un plein ; l'export CSV ; la navigation des deux marques. Données fictives.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::erreurs::{a_le_code, ErreurApp, CODE_VIOLATION_CHECK};
use gestion_parc_auto::metier::depenses::{self, SaisieDepense};
use gestion_parc_auto::metier::vehicules::{self, SaisieVehicule};
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, creer_vehicule, get, post_formulaire,
    requete_json, saisie_vehicule, se_connecter, texte,
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
        carburant: String::new(),
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
    // Le carburant du plein est proposé d'après celui du véhicule (diesel → gazole).
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
            r#"<option value="{vehicule}" data-carburant="gazole" selected>"#
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
            r#"<option value="{electrique}" data-carburant="electricite" selected>"#
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
         &quantite=46&kilometrage=62000&libelle=&notes="
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
    assert_eq!(
        modifie.carburant, "gazole",
        "sans carburant envoyé : celui du véhicule"
    );
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
            "montant":"68,40","quantite":"45,2","kilometrage":"61200","plein_complet":"true",
            "libelle":"Station fictive"}}"#
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
    assert_eq!(cree["carburant"], "gazole", "proposé d'après le véhicule");
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
        "\u{FEFF}Date;Véhicule;Type;Carburant;Quantité;Unité;Kilométrage (km);Plein complet;Montant (€);Station ou trajet;Notes\r\n\
         13/10/2026;DP-400-DP;Péage;;;;;;12,30;Lyon – Grenoble;\r\n\
         12/10/2026;DP-400-DP;Carburant;Gazole (B7);45,2;L;61200;oui;68,40;Station fictive;\r\n"
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
}
