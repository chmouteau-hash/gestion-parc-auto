//! Ce qui a été fait pendant une intervention (#94, bloc 1) : les lignes d'opérations, de la
//! saisie à la fiche, par le formulaire HTML et par l'API JSON. Et la facture jointe depuis le
//! même formulaire (#100).

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::erreurs::ErreurApp;
use gestion_parc_auto::metier::interventions::{self, SaisieIntervention, SaisieLigne};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, creer_vehicule, get, post_formulaire, post_multipart,
    post_multipart_nomme, requete_json, se_connecter, texte,
};

fn ligne(operation: &str, reference: &str, quantite: &str, prix: &str) -> SaisieLigne {
    SaisieLigne {
        operation: operation.to_string(),
        libelle: String::new(),
        reference: reference.to_string(),
        quantite: quantite.to_string(),
        prix: prix.to_string(),
    }
}

fn saisie(
    vehicule: i64,
    date: &str,
    kilometrage: &str,
    lignes: Vec<SaisieLigne>,
) -> SaisieIntervention {
    SaisieIntervention {
        vehicule_id: vehicule.to_string(),
        type_intervention: "revision".to_string(),
        date_intervention: date.to_string(),
        kilometrage: kilometrage.to_string(),
        cout: "189,90".to_string(),
        prestataire: "Garage fictif".to_string(),
        notes: String::new(),
        lignes,
    }
}

async fn nombre_de_lignes(pool: &PgPool, intervention: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM lignes_intervention WHERE intervention_id = $1")
        .bind(intervention)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn les_lignes_se_lisent_dans_l_ordre_du_vocabulaire_se_remplacent_et_partent_avec_l_intervention(
    pool: PgPool,
) {
    let vehicule = creer_vehicule(&pool, "OP-100-OP").await;
    let mut autre = ligne("autre", "", "", "49,90");
    autre.libelle = "Joint de culasse".to_string();
    let creee = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![
                autre,
                ligne("filtre_huile", "W712", "2", "12,50"),
                ligne("vidange", "5W30", "", ""),
            ],
        ),
    )
    .await
    .unwrap();

    // Relue dans l'ordre du vocabulaire, « autre » en dernier, quel que soit l'ordre de saisie.
    let codes: Vec<&str> = creee.lignes.iter().map(|l| l.operation.as_str()).collect();
    assert_eq!(codes, ["vidange", "filtre_huile", "autre"]);
    assert_eq!(
        creee.operations_fr(),
        "Vidange (réf. 5W30) · Filtre à huile (réf. W712, × 2, 12,50 €) · Joint de culasse (49,90 €)"
    );
    // Même chose par les listes.
    let par_vehicule = interventions::lister_par_vehicule(&pool, vehicule)
        .await
        .unwrap();
    assert_eq!(par_vehicule[0].lignes.len(), 3);
    assert_eq!(
        interventions::lister(&pool).await.unwrap()[0].lignes.len(),
        3
    );

    // Modifier remplace l'ensemble des lignes.
    let modifiee = interventions::modifier(
        &pool,
        creee.id,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![ligne("pneus", "", "4", "412")],
        ),
    )
    .await
    .unwrap();
    assert_eq!(modifiee.operations_fr(), "Pneus (× 4, 412,00 €)");
    assert_eq!(nombre_de_lignes(&pool, creee.id).await, 1);

    // Et une intervention sans ligne n'a rien à dire.
    let vide = interventions::modifier(
        &pool,
        creee.id,
        &saisie(vehicule, "2026-05-05", "20000", vec![]),
    )
    .await
    .unwrap();
    assert_eq!(vide.operations_fr(), "");
    assert_eq!(nombre_de_lignes(&pool, creee.id).await, 0);

    // Supprimer l'intervention emporte ses lignes (cascade).
    let avec = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-06-06",
            "21000",
            vec![ligne("batterie", "", "", "")],
        ),
    )
    .await
    .unwrap();
    assert_eq!(nombre_de_lignes(&pool, avec.id).await, 1);
    interventions::supprimer(&pool, avec.id).await.unwrap();
    assert_eq!(nombre_de_lignes(&pool, avec.id).await, 0);
}

#[sqlx::test]
async fn une_ligne_refusee_n_enregistre_rien_et_la_base_tient_ses_regles(pool: PgPool) {
    let vehicule = creer_vehicule(&pool, "OP-200-OP").await;

    // « Autre » sans libellé : refus lisible, et ni l'intervention ni le compteur ne bougent.
    let refus = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "99000",
            vec![ligne("autre", "", "", "")],
        ),
    )
    .await;
    assert!(matches!(refus, Err(ErreurApp::Metier(m)) if m.contains("Autre")));
    assert!(interventions::lister(&pool).await.unwrap().is_empty());
    assert_eq!(
        vehicules::obtenir(&pool, vehicule)
            .await
            .unwrap()
            .kilometrage,
        12000
    );

    // La base elle-même refuse un doublon et un « autre » sans libellé, même en la contournant.
    let creee = interventions::creer(
        &pool,
        &saisie(
            vehicule,
            "2026-05-05",
            "20000",
            vec![ligne("vidange", "", "", "")],
        ),
    )
    .await
    .unwrap();
    let doublon = sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation) VALUES ($1, 'vidange')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(gestion_parc_auto::erreurs::a_le_code(
        &doublon,
        gestion_parc_auto::erreurs::CODE_VIOLATION_UNICITE
    ));
    let sans_libelle = sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation) VALUES ($1, 'autre')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(gestion_parc_auto::erreurs::a_le_code(
        &sans_libelle,
        gestion_parc_auto::erreurs::CODE_VIOLATION_CHECK
    ));
    // Deux « autre » distincts, en revanche, passent.
    sqlx::query(
        "INSERT INTO lignes_intervention (intervention_id, operation, libelle)
         VALUES ($1, 'autre', 'Joint'), ($1, 'autre', 'Durite')",
    )
    .bind(creee.id)
    .execute(&pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn le_formulaire_enregistre_les_cases_cochees_et_les_remontre(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "OP-300-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire vierge propose toutes les opérations, aucune cochée.
    let page = texte(get(&app, "/interventions/nouvelle", Some(&cookie)).await).await;
    assert!(page.contains("Ce qui a été fait"), "{page}");
    for libelle in [
        "Vidange",
        "Filtre à huile",
        "Courroie de distribution",
        "Autre",
    ] {
        assert!(page.contains(libelle), "{libelle} absent\n{page}");
    }
    assert!(!page.contains(" checked"), "{page}");

    // Deux cases cochées ; les champs d'une case non cochée (filtre à air) voyagent aussi et
    // sont ignorés.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&cout=149%2C90&prestataire=Garage&notes=\
         &operation=vidange&reference_vidange=5W30&quantite_vidange=5%2C4&prix_vidange=45\
         &reference_filtre_air=oubli%C3%A9&quantite_filtre_air=3\
         &operation=autre&libelle_autre=Joint&reference_autre=&quantite_autre=&prix_autre="
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        reponse.headers()[header::LOCATION],
        format!("/vehicules/{vehicule}")
    );

    let attendu = "Vidange (réf. 5W30, × 5,4, 45,00 €) · Joint";
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(attendu), "{fiche}");
    assert!(!fiche.contains("oublié"), "{fiche}");
    let liste = texte(get(&app, "/interventions", Some(&cookie)).await).await;
    assert!(liste.contains(attendu), "{liste}");

    // « Modifier » remontre les cases cochées avec leurs valeurs, les autres vides.
    let id = interventions::lister(&pool).await.unwrap()[0].id;
    let formulaire = texte(
        get(
            &app,
            &format!("/interventions/{id}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        formulaire.contains(r#"value="vidange" checked"#)
            && formulaire.contains(r#"name="quantite_vidange" value="5,4""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"name="reference_vidange" value="5W30""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"name="prix_vidange" value="45.00""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"value="autre" checked"#)
            && formulaire.contains(r#"name="libelle_autre" value="Joint""#),
        "{formulaire}"
    );
    assert!(
        formulaire.contains(r#"value="filtre_air"> Filtre à air"#),
        "{formulaire}"
    );

    // Tout décocher vide les lignes.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&cout=149%2C90&prestataire=Garage&notes=&reference_vidange=5W30"
    );
    let reponse =
        post_formulaire(&app, &format!("/interventions/{id}"), Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(!fiche.contains("5W30"), "{fiche}");

    // Un refus réaffiche le formulaire avec le message et la saisie.
    let corps = format!(
        "vehicule_id={vehicule}&type_intervention=revision&date_intervention=2026-05-10\
         &kilometrage=13000&operation=autre&libelle_autre=&operation=pneus&quantite_pneus=4"
    );
    let reponse = post_formulaire(&app, "/interventions", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains(r#"role="alert""#) && page.contains("Autre"),
        "{page}"
    );
    assert!(
        page.contains(r#"value="pneus" checked"#)
            && page.contains(r#"name="quantite_pneus" value="4""#),
        "{page}"
    );
    assert_eq!(interventions::lister(&pool).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn l_api_json_porte_les_lignes(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    creer_utilisateur(&pool, "lecture", "lecteur").await;
    let vehicule = creer_vehicule(&pool, "OP-400-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"revision","date_intervention":"2026-05-10",
            "kilometrage":"13000","cout":"149,90",
            "lignes":[{{"operation":"vidange","prix":"45"}},
                      {{"operation":"filtre_huile","reference":"W712","quantite":"1","prix":"12,50"}}]}}"#
    );
    let reponse = requete_json(
        &app,
        "POST",
        "/api/v1/interventions",
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::CREATED);
    let cree: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    let id = cree["id"].as_i64().unwrap();
    assert_eq!(cree["lignes"].as_array().unwrap().len(), 2);
    assert_eq!(cree["lignes"][0]["operation"], "vidange");
    assert_eq!(cree["lignes"][0]["prix_centimes"], 4500);
    assert_eq!(cree["lignes"][1]["reference_piece"], "W712");

    // Un lecteur les lit aussi.
    let lecture = se_connecter(&app, "lecture").await;
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/interventions/{id}"),
        Some(&lecture),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let lue: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(lue["lignes"].as_array().unwrap().len(), 2);

    // Une ligne fautive est refusée comme toute saisie.
    let corps = format!(
        r#"{{"vehicule_id":"{vehicule}","type_intervention":"revision","date_intervention":"2026-05-10",
            "kilometrage":"13000","lignes":[{{"operation":"vidange_moteur"}}]}}"#
    );
    let reponse = requete_json(
        &app,
        "PUT",
        &format!("/api/v1/interventions/{id}"),
        Some(&cookie),
        Some(&corps),
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        interventions::obtenir(&pool, id)
            .await
            .unwrap()
            .lignes
            .len(),
        2,
        "le refus n'a rien changé"
    );
}

/// La facture d'une intervention, jointe depuis le formulaire (#100) : un document du véhicule
/// relié à l'intervention, qui la suit sur la fiche, dans la liste et dans l'API, et qui reste
/// quand l'intervention disparaît.
#[sqlx::test]
async fn la_facture_se_joint_depuis_le_formulaire_et_suit_l_intervention(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "OP-500-OP").await;
    let autre_vehicule = creer_vehicule(&pool, "OP-501-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let vehicule_texte = vehicule.to_string();
    let champs = [
        ("vehicule_id", vehicule_texte.as_str()),
        ("type_intervention", "revision"),
        ("date_intervention", "2026-05-10"),
        ("kilometrage", "13000"),
        ("cout", "138"),
        ("prestataire", "Garage fictif"),
        ("notes", ""),
        ("operation", "vidange"),
        ("reference_vidange", "5W40"),
        ("quantite_vidange", "5,4"),
        ("prix_vidange", "67,50"),
    ];

    // Création avec la facture (un PDF fictif) : l'intervention, ses lignes, et le document relié.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/interventions",
        "facture",
        &champs,
        &[(
            "facture-0512.pdf",
            "application/pdf",
            b"%PDF-1.4 facture fictive",
        )],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let id = interventions::lister(&pool).await.unwrap()[0].id;
    let creee = interventions::obtenir(&pool, id).await.unwrap();
    assert_eq!(creee.lignes.len(), 1);
    assert_eq!(creee.documents.len(), 1, "{creee:?}");
    let facture = creee.documents[0].clone();
    assert_eq!(
        (
            facture.type_document.as_str(),
            facture.vehicule_id,
            facture.intervention_id,
            facture.nom_original.as_str()
        ),
        ("facture", Some(vehicule), Some(id), "facture-0512.pdf")
    );

    // La fiche et la liste mènent à la facture ; sa page nomme l'intervention.
    let lien = format!(r#"<a href="/documents/{}">Facture</a>"#, facture.id);
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(fiche.contains(&lien), "{fiche}");
    let liste = texte(get(&app, "/interventions", Some(&cookie)).await).await;
    assert!(liste.contains(&lien), "{liste}");
    let page = texte(get(&app, &format!("/documents/{}", facture.id), Some(&cookie)).await).await;
    assert!(
        page.contains("Document de l'intervention du 10/05/2026 (Révision, Garage fictif)."),
        "{page}"
    );

    // Le formulaire de modification rappelle la facture jointe et en accepte une seconde.
    let formulaire = texte(
        get(
            &app,
            &format!("/interventions/{id}/modifier"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        formulaire.contains(r#"enctype="multipart/form-data""#)
            && formulaire.contains(&format!(r#"href="/documents/{}""#, facture.id)),
        "{formulaire}"
    );
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        &format!("/interventions/{id}"),
        "facture",
        &champs,
        &[("complement.jpg", "image/jpeg", b"une image, pour le test")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        interventions::obtenir(&pool, id)
            .await
            .unwrap()
            .documents
            .len(),
        2
    );

    // Sans facture (le navigateur envoie le champ vide) : rien de plus.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        &format!("/interventions/{id}"),
        "facture",
        &champs,
        &[("", "application/octet-stream", b"")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        interventions::obtenir(&pool, id)
            .await
            .unwrap()
            .documents
            .len(),
        2
    );

    // Une facture vide est refusée avant d'écrire quoi que ce soit : le formulaire revient avec
    // la saisie et dit de la rechoisir ; aucune intervention de plus.
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/interventions",
        "facture",
        &champs,
        &[("vide.pdf", "application/pdf", b"")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("Le fichier est vide.")
            && page.contains(r#"name="reference_vidange" value="5W40""#)
            && page.contains("choisissez-le de nouveau"),
        "{page}"
    );
    assert_eq!(interventions::lister(&pool).await.unwrap().len(), 1);

    // Depuis une fiche : un dépôt relié à une intervention d'un autre véhicule est refusé, relié
    // à la bonne, accepté.
    let id_texte = id.to_string();
    let reponse = post_multipart(
        &app,
        &cookie,
        &format!("/vehicules/{autre_vehicule}/documents"),
        &[("type_document", "facture"), ("intervention_id", &id_texte)],
        &[("f.pdf", "application/pdf", b"%PDF")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    // L'apostrophe du message est échappée dans la page (&#x27;) : on cherche la suite.
    assert!(texte(reponse)
        .await
        .contains("pas une intervention de ce véhicule"));
    let reponse = post_multipart(
        &app,
        &cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[("type_document", "facture"), ("intervention_id", &id_texte)],
        &[("f.pdf", "application/pdf", b"%PDF")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    // L'API lit les documents de l'intervention, sans les noms de fichiers internes.
    let reponse = requete_json(
        &app,
        "GET",
        &format!("/api/v1/interventions/{id}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let lue: serde_json::Value = serde_json::from_str(&texte(reponse).await).unwrap();
    assert_eq!(lue["documents"].as_array().unwrap().len(), 3);
    assert_eq!(lue["documents"][0]["intervention_id"], id);
    assert!(lue["documents"][0].get("chemin").is_none(), "{lue}");

    // Supprimer l'intervention garde les documents, sans intervention.
    interventions::supprimer(&pool, id).await.unwrap();
    let restants: Vec<Option<i64>> =
        sqlx::query_scalar("SELECT intervention_id FROM documents WHERE vehicule_id = $1")
            .bind(vehicule)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(restants, vec![None, None, None]);
}

/// Le type du document joint (#104) : proposé d'office selon l'intervention, choisi dans une
/// liste réduite, refusé hors de cette liste.
#[sqlx::test]
async fn le_document_joint_porte_le_type_choisi_ou_celui_de_l_intervention(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let vehicule = creer_vehicule(&pool, "OP-600-OP").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let vehicule_texte = vehicule.to_string();
    let choix_du_type = |page: &str| -> String {
        let debut = page
            .find(r#"name="type_document""#)
            .expect("liste du type du document");
        let bloc = &page[debut..];
        let fin = bloc.find("</select>").unwrap();
        bloc[..fin]
            .lines()
            .find(|l| l.contains(" selected"))
            .map(|l| l.trim().to_string())
            .unwrap_or_default()
    };

    // À l'ouverture : « Facture » d'office, « Contrôle technique » pour un contrôle technique.
    let page = texte(
        get(
            &app,
            &format!("/interventions/nouvelle?vehicule_id={vehicule}"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        choix_du_type(&page).starts_with(r#"<option value="facture" selected>"#),
        "{}",
        choix_du_type(&page)
    );
    assert!(
        page.contains("Facture ou procès-verbal")
            && page.contains("Prendre ou choisir le document"),
        "{page}"
    );
    let page = texte(
        get(
            &app,
            &format!(
                "/interventions/nouvelle?vehicule_id={vehicule}&type_intervention=controle_technique"
            ),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        choix_du_type(&page).starts_with(r#"<option value="controle_technique" selected>"#),
        "{}",
        choix_du_type(&page)
    );
    // Les types sans objet pour une intervention n'y sont pas.
    let debut = page.find(r#"name="type_document""#).unwrap();
    let liste = &page[debut..debut + page[debut..].find("</select>").unwrap()];
    assert!(
        !liste.contains("photo_vehicule")
            && !liste.contains("carte_grise")
            && liste.contains("autre"),
        "{liste}"
    );

    // Un contrôle technique avec son procès-verbal : le document est un « Contrôle technique ».
    let champs = [
        ("vehicule_id", vehicule_texte.as_str()),
        ("type_intervention", "controle_technique"),
        ("date_intervention", "2026-01-20"),
        ("kilometrage", "70445"),
        ("cout", "65"),
        ("prestataire", "Centre fictif"),
        ("notes", ""),
        ("type_document", "controle_technique"),
    ];
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/interventions",
        "facture",
        &champs,
        &[(
            "pv.pdf",
            "application/pdf",
            b"%PDF-1.4 proces-verbal fictif",
        )],
    )
    .await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let creee = interventions::lister(&pool).await.unwrap().remove(0);
    assert_eq!(creee.documents.len(), 1);
    assert_eq!(creee.documents[0].type_document, "controle_technique");
    let fiche = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    assert!(
        fiche.contains(&format!(
            r#"<a href="/documents/{}">Contrôle technique</a>"#,
            creee.documents[0].id
        )),
        "{fiche}"
    );

    // Sans choix envoyé, le type suit l'intervention ; une révision donne une facture.
    let mut revision = champs.to_vec();
    revision.retain(|(n, _)| *n != "type_document");
    revision[1] = ("type_intervention", "revision");
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/interventions",
        "facture",
        &revision,
        &[(
            "facture.pdf",
            "application/pdf",
            b"%PDF-1.4 facture fictive",
        )],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let types: Vec<String> = sqlx::query_scalar(
        "SELECT type_document FROM documents WHERE vehicule_id = $1 ORDER BY id",
    )
    .bind(vehicule)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(types, ["controle_technique", "facture"]);

    // Un type hors de la liste est refusé, rien n'est écrit, et le choix fautif reste affiché.
    let mut photo = champs.to_vec();
    photo[7] = ("type_document", "photo_vehicule");
    let reponse = post_multipart_nomme(
        &app,
        &cookie,
        "/interventions",
        "facture",
        &photo,
        &[("photo.png", "image/png", b"pas une photo")],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    assert!(
        page.contains("pas « Photo du véhicule »") && page.contains("choisissez-le de nouveau"),
        "{page}"
    );
    assert_eq!(interventions::lister(&pool).await.unwrap().len(), 2);
}
