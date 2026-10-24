//! La vignette Crit'Air du véhicule (#165), sur base réelle : le choix dans le formulaire, la
//! règle que le formulaire reçoit pour proposer une classe, la fiche (vignette choisie, ou
//! proposée), la liste, l'API, l'export, et le certificat rangé dans le dossier « Administratif ».

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::metier::vehicules::{self, SaisieVehicule};
use sqlx::PgPool;

use commun::{
    application, creer_utilisateur, get, post_formulaire, post_multipart, requete_json,
    saisie_vehicule, se_connecter, texte,
};

/// Un diesel mis en service le 15/03/2008 : l'application lui propose Crit'Air 3.
const FORMULAIRE: &str = "immatriculation=CR-100-AI&marque=Peugeot&modele=Partner&date_mise_en_service=2008-03-15&carburant=diesel&kilometrage=180000&statut=en_service";

/// La position d'un texte dans la page, pour vérifier l'ordre d'affichage.
fn position(page: &str, texte: &str) -> usize {
    page.find(texte)
        .unwrap_or_else(|| panic!("« {texte} » absent\n{page}"))
}

#[sqlx::test]
async fn la_vignette_se_choisit_et_se_montre_sur_la_fiche_et_dans_la_liste(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire : « Non renseignée » cochée d'avance, une case par classe avec sa pastille,
    // et la règle de la proposition pour le script, telle que `CritAir::tranches` l'écrit.
    let page = texte(get(&app, "/vehicules/nouveau", Some(&cookie)).await).await;
    assert!(
        page.contains(
            r#"<label class="nuance"><input type="radio" name="crit_air" value="" checked>Non renseignée</label>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<label class="nuance"><input type="radio" name="crit_air" value="3"><span class="crit-air crit-air-3" aria-hidden="true">3</span>Crit&#39;Air 3</label>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<label class="nuance"><input type="radio" name="crit_air" value="non_classe">Non classé</label>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(
            "&#34;diesel&#34;:[[2011,&#34;2&#34;],[2006,&#34;3&#34;],[2001,&#34;4&#34;],[1997,&#34;5&#34;],[-2147483648,&#34;non_classe&#34;]]"
        ),
        "{page}"
    );
    assert!(
        page.contains("&#34;electrique&#34;:[[-2147483648,&#34;e&#34;]]"),
        "{page}"
    );
    assert!(page.contains("&#34;autre&#34;:[]"), "{page}");

    // Sans vignette : la fiche le dit, et donne la proposition en texte seul ; la liste n'a pas
    // de pastille.
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), FORMULAIRE).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(
            "<dt>Vignette Crit'Air</dt><dd><em>non renseignée</em> · d'après le carburant et la mise en service : Crit&#39;Air 3, à vérifier sur le certificat</dd>"
        ),
        "{page}"
    );
    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(liste.contains("<th>Crit'Air</th>"), "{liste}");
    assert!(!liste.contains("crit-air"), "{liste}");

    // Avec une vignette : la fiche montre la pastille et le libellé, plus de proposition ; le
    // formulaire de modification la coche ; la liste montre la pastille, nommée.
    let corps = format!("{FORMULAIRE}&crit_air=2");
    let reponse = post_formulaire(&app, &fiche, Some(&cookie), &corps).await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(
            r#"<dt>Vignette Crit'Air</dt><dd><span class="crit-air crit-air-2" aria-hidden="true">2</span> Crit&#39;Air 2</dd>"#
        ),
        "{page}"
    );
    assert!(!page.contains("à vérifier sur le certificat"), "{page}");
    let page = texte(get(&app, &format!("{fiche}/modifier"), Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<input type="radio" name="crit_air" value="2" checked>"#),
        "{page}"
    );
    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(
        liste.contains(
            r#"<td><span class="crit-air crit-air-2" role="img" aria-label="Crit&#39;Air 2" title="Crit&#39;Air 2">2</span></td>"#
        ),
        "{liste}"
    );

    // L'API et l'export CSV la donnent aussi.
    let reponse = requete_json(&app, "GET", &format!("/api/v1{fiche}"), Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse).await.contains(r#""crit_air":"2""#));
    let csv = texte(get(&app, "/vehicules.csv", Some(&cookie)).await).await;
    assert!(csv.contains(";Diesel;Crit'Air 2;180000;"), "{csv}");

    // « Non classé » n'a pas de vignette : écrit en toutes lettres, sans pastille.
    let corps = format!("{FORMULAIRE}&crit_air=non_classe");
    let reponse = post_formulaire(&app, &fiche, Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains("<dt>Vignette Crit'Air</dt><dd> Non classé</dd>"),
        "{page}"
    );
    let liste = texte(get(&app, "/vehicules", Some(&cookie)).await).await;
    assert!(liste.contains("<td>Non classé</td>"), "{liste}");

    // Une classe qui n'existe pas est refusée, avec le nom du champ.
    let corps = format!("{FORMULAIRE}&crit_air=6");
    let reponse = post_formulaire(&app, &fiche, Some(&cookie), &corps).await;
    let page = texte(reponse).await;
    assert!(
        page.contains("La valeur « 6 » n&#39;est pas acceptée pour « vignette Crit&#39;Air »."),
        "{page}"
    );
}

#[sqlx::test]
async fn la_fiche_propose_la_classe_du_carburant_et_de_la_date(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    for (immatriculation, carburant, date, attendu) in [
        (
            "CR-201-EL",
            "electrique",
            "2024-06-20",
            " · d'après le carburant et la mise en service : Crit&#39;Air E, à vérifier sur le certificat",
        ),
        (
            "CR-202-GP",
            "gpl",
            "2009-05-01",
            " · d'après le carburant et la mise en service : Crit&#39;Air 1, à vérifier sur le certificat",
        ),
        (
            "CR-203-ES",
            "essence",
            "1995-04-01",
            " · d'après le carburant et la mise en service : Non classé, à vérifier sur le certificat",
        ),
        ("CR-204-AU", "autre", "2020-01-01", ""),
    ] {
        let id = vehicules::creer(
            &pool,
            &SaisieVehicule {
                carburant: carburant.to_string(),
                date_mise_en_service: date.to_string(),
                ..saisie_vehicule(immatriculation)
            },
        )
        .await
        .unwrap()
        .id;
        let page = texte(get(&app, &format!("/vehicules/{id}"), Some(&cookie)).await).await;
        assert!(
            page.contains(&format!(
                "<dt>Vignette Crit'Air</dt><dd><em>non renseignée</em>{attendu}</dd>"
            )),
            "{carburant}\n{page}"
        );
    }
}

#[sqlx::test]
async fn le_certificat_qualite_de_l_air_se_range_dans_administratif(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let vehicule = commun::creer_vehicule(&pool, "CR-300-CE").await;

    // Le dépôt depuis « Administratif » le propose, à côté de la carte grise et de l'assurance.
    let page = texte(
        get(
            &app,
            &format!("/documents/nouveau?vehicule_id={vehicule}&dossier=administratif"),
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert!(
        page.contains(
            r#"<option value="certificat_qualite_air">Certificat qualité de l&#39;air (Crit&#39;Air)</option>"#
        ),
        "{page}"
    );

    let reponse = post_multipart(
        &app,
        &cookie,
        &format!("/vehicules/{vehicule}/documents"),
        &[("type_document", "certificat_qualite_air")],
        &[(
            "certificat-crit-air.pdf",
            "application/pdf",
            b"%PDF-1.4 cqa",
        )],
    )
    .await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let page = texte(get(&app, &format!("/vehicules/{vehicule}"), Some(&cookie)).await).await;
    let administratif = position(&page, "<h3>Administratif</h3>");
    let certificat = position(&page, "certificat-crit-air.pdf");
    let entretien = position(&page, "<h3>Entretien</h3>");
    assert!(
        administratif < certificat && certificat < entretien,
        "{page}"
    );
}
