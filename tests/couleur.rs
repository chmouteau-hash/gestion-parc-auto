//! La couleur de repère du véhicule, sur base réelle : le nuancier du formulaire, la plaque et la
//! silhouette teintées partout, la fiche, l'API, l'export. Sans couleur, rien ne change.

mod commun;

use axum::http::{header, StatusCode};
use gestion_parc_auto::commun::types::Profil;
use gestion_parc_auto::metier::echeances::{self, SaisieEcheance};
use gestion_parc_auto::metier::vehicules;
use sqlx::PgPool;

use commun::{
    application, application_avec_profil, creer_utilisateur, get, post_formulaire, requete_json,
    se_connecter, texte,
};

const FORMULAIRE: &str = "immatriculation=CO-100-UL&marque=Renault&modele=Clio&date_mise_en_service=2022-03-15&carburant=essence&kilometrage=48200&statut=en_service";

#[sqlx::test]
async fn la_couleur_se_choisit_dans_un_nuancier_et_teinte_la_plaque_partout(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le formulaire propose le nuancier, « Aucune » cochée d'avance.
    let page = texte(get(&app, "/vehicules/nouveau", Some(&cookie)).await).await;
    assert!(
        page.contains(
            r#"<label class="nuance"><input type="radio" name="couleur" value="" checked>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<label class="nuance couleur-rouge"><input type="radio" name="couleur" value="rouge">"#
        ),
        "{page}"
    );

    // Sans couleur : la plaque reste bleue (aucune classe de plus), la fiche le dit.
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), FORMULAIRE).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<span class="plaque">CO-100-UL</span>"#),
        "{page}"
    );
    assert!(
        page.contains("<dt>Couleur</dt><dd><em>non renseignée</em></dd>"),
        "{page}"
    );

    // Avec une couleur : la plaque la porte en tête de fiche, la silhouette aussi, la fiche écrit
    // son nom avec son échantillon, et le formulaire de modification la coche.
    let corps = format!("{FORMULAIRE}&couleur=rouge");
    let reponse = post_formulaire(&app, &fiche, Some(&cookie), &corps).await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<span class="plaque couleur-rouge">CO-100-UL</span>"#),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<dt>Couleur</dt><dd><span class="couleur-rouge"><span class="echantillon" aria-hidden="true"></span> Rouge</span></dd>"#
        ),
        "{page}"
    );
    assert!(
        page.contains(r#"<span class="silhouette couleur-rouge" aria-hidden="true"></span>"#),
        "{page}"
    );
    let page = texte(get(&app, &format!("{fiche}/modifier"), Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<input type="radio" name="couleur" value="rouge" checked>"#),
        "{page}"
    );

    // Dans les listes et la recherche, la plaque porte la couleur.
    let id: i64 = fiche.rsplit('/').next().unwrap().parse().unwrap();
    echeances::creer(
        &pool,
        &SaisieEcheance {
            vehicule_id: id.to_string(),
            type_echeance: "assurance".to_string(),
            date_limite: "2027-01-01".to_string(),
            statut: String::new(),
            ..Default::default()
        },
    )
    .await
    .expect("échéance acceptée");
    for chemin in ["/vehicules", "/echeances", "/recherche?q=CO-100"] {
        let page = texte(get(&app, chemin, Some(&cookie)).await).await;
        assert!(
            page.contains(r#"<span class="plaque couleur-rouge">CO-100-UL</span>"#),
            "{chemin}\n{page}"
        );
    }

    // L'API et l'export CSV la donnent aussi.
    let reponse = requete_json(&app, "GET", &format!("/api/v1{fiche}"), Some(&cookie), None).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(texte(reponse).await.contains(r#""couleur":"rouge""#));
    let csv = texte(get(&app, "/vehicules.csv", Some(&cookie)).await).await;
    assert!(csv.contains("CO-100-UL;;Renault;Clio;Rouge;"), "{csv}");
}

#[sqlx::test]
async fn sur_l_accueil_la_silhouette_prend_la_couleur(pool: PgPool) {
    creer_utilisateur(&pool, "christophe", "administrateur").await;
    let app = application_avec_profil(pool.clone(), Profil::Particulier);
    let cookie = se_connecter(&app, "christophe").await;
    let corps = format!("{FORMULAIRE}&couleur=bleu");
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), &corps).await;
    assert_eq!(
        reponse.status(),
        StatusCode::SEE_OTHER,
        "{}",
        texte(reponse).await
    );
    let page = texte(get(&app, "/", Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<span class="plaque couleur-bleu">CO-100-UL</span>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<span class="silhouette couleur-bleu"></span>"#),
        "{page}"
    );
}

#[sqlx::test]
async fn les_couleurs_courantes_ont_des_niveaux_et_il_y_a_un_beige(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;

    // Le nuancier propose les trois niveaux de chacune, du plus clair au plus foncé.
    let page = texte(get(&app, "/vehicules/nouveau", Some(&cookie)).await).await;
    for famille in ["gris", "rouge", "vert", "bleu"] {
        let places: Vec<usize> = [
            format!("{famille}_clair"),
            famille.to_string(),
            format!("{famille}_fonce"),
        ]
        .iter()
        .map(|code| {
            page.find(&format!(r#"name="couleur" value="{code}""#))
                .unwrap_or_else(|| panic!("niveau {code} absent du nuancier\n{page}"))
        })
        .collect();
        assert!(places.is_sorted(), "{famille} : {places:?}");
        // Les trois niveaux sont dans la même rangée : aucune autre ne s'ouvre entre le clair et
        // le foncé.
        assert!(
            !page[places[0]..places[2]].contains("rang-nuances"),
            "{famille} : ses niveaux sont répartis sur deux rangées"
        );
    }
    assert!(
        page.contains(
            r#"<label class="nuance couleur-gris_fonce"><input type="radio" name="couleur" value="gris_fonce"><span class="echantillon" aria-hidden="true"></span>Gris foncé</label>"#
        ),
        "{page}"
    );
    // Le marron a deux niveaux, dans une rangée à lui ; le beige est une couleur à part, dans la
    // première rangée avec celles qui n'ont qu'un niveau.
    let clair = page.find(r#"value="marron_clair""#).expect("marron clair");
    let normal = page.find(r#"value="marron""#).expect("marron");
    assert!(clair < normal, "marron clair avant marron");
    assert!(
        !page[clair..normal].contains("rang-nuances"),
        "les deux niveaux du marron sont répartis sur deux rangées"
    );
    let beige = page.find(r#"value="beige""#).expect("beige");
    let premiere_rangee = page.find("rang-nuances").expect("une rangée");
    let seconde_rangee = page[premiere_rangee + 1..]
        .find("rang-nuances")
        .map(|i| premiere_rangee + 1 + i)
        .expect("une seconde rangée");
    assert!(
        premiere_rangee < beige && beige < seconde_rangee,
        "le beige est dans la première rangée"
    );

    // Un niveau se choisit, s'écrit et teinte la plaque comme n'importe quelle couleur.
    let corps = format!("{FORMULAIRE}&couleur=gris_fonce");
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);
    let fiche = reponse.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let page = texte(get(&app, &fiche, Some(&cookie)).await).await;
    assert!(
        page.contains(r#"<span class="plaque couleur-gris_fonce">CO-100-UL</span>"#),
        "{page}"
    );
    assert!(
        page.contains(
            r#"<dt>Couleur</dt><dd><span class="couleur-gris_fonce"><span class="echantillon" aria-hidden="true"></span> Gris foncé</span></dd>"#
        ),
        "{page}"
    );
    let csv = texte(get(&app, "/vehicules.csv", Some(&cookie)).await).await;
    assert!(csv.contains("CO-100-UL;;Renault;Clio;Gris foncé;"), "{csv}");
}

#[sqlx::test]
async fn une_couleur_hors_du_nuancier_est_refusee(pool: PgPool) {
    creer_utilisateur(&pool, "gestion", "administrateur").await;
    let app = application(pool.clone());
    let cookie = se_connecter(&app, "gestion").await;
    let corps = format!("{FORMULAIRE}&couleur=turquoise");
    let reponse = post_formulaire(&app, "/vehicules", Some(&cookie), &corps).await;
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = texte(reponse).await;
    // L'apostrophe du message est échappée dans la page : on vise la fin de la phrase.
    assert!(
        page.contains("« turquoise » n&#39;est pas acceptée pour « couleur »"),
        "{page}"
    );
    // Rien n'a été enregistré.
    assert!(vehicules::lister(&pool).await.unwrap().is_empty());
}
