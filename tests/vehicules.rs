//! Véhicules : cycle complet et contraintes de la base.

mod commun;

use gestion_parc_auto::commun::types::{
    Carburant, StatutConducteur, StatutEcheance, StatutVehicule, TypeEcheance, TypeIntervention,
};
use gestion_parc_auto::erreurs::ErreurApp;
use gestion_parc_auto::metier::interventions::SaisieIntervention;
use gestion_parc_auto::metier::{interventions, vehicules};
use sqlx::PgPool;

use commun::{creer_vehicule, saisie_vehicule};

#[sqlx::test]
async fn cycle_creer_lire_modifier_supprimer(pool: PgPool) {
    let cree = vehicules::creer(&pool, &saisie_vehicule("ab-123-cd"))
        .await
        .unwrap();
    assert_eq!(
        cree.immatriculation, "AB-123-CD",
        "normalisée en majuscules"
    );
    assert_eq!(cree.kilometrage, 12000);

    let lu = vehicules::obtenir(&pool, cree.id).await.unwrap();
    assert_eq!(lu.marque, "Renault");

    let mut saisie = saisie_vehicule("AB-123-CD");
    saisie.statut = "en_maintenance".to_string();
    saisie.kilometrage = "15000".to_string();
    let modifie = vehicules::modifier(&pool, cree.id, &saisie).await.unwrap();
    assert_eq!(modifie.statut, "en_maintenance");
    assert_eq!(modifie.kilometrage, 15000);
    assert!(modifie.modifie_le >= cree.modifie_le);

    assert_eq!(vehicules::lister(&pool).await.unwrap().len(), 1);

    vehicules::supprimer(&pool, &commun::dossier_documents_de_test(), cree.id)
        .await
        .unwrap();
    assert!(matches!(
        vehicules::obtenir(&pool, cree.id).await,
        Err(ErreurApp::Introuvable)
    ));
}

#[sqlx::test]
async fn immatriculation_unique(pool: PgPool) {
    creer_vehicule(&pool, "ZZ-999-ZZ").await;
    let erreur = vehicules::creer(&pool, &saisie_vehicule("zz-999-zz"))
        .await
        .expect_err("doublon refusé, même avec une casse différente");
    match erreur {
        ErreurApp::Metier(message) => assert!(message.contains("ZZ-999-ZZ"), "{message}"),
        autre => panic!("erreur inattendue : {autre:?}"),
    }
}

#[sqlx::test]
async fn saisie_invalide_ne_touche_pas_la_base(pool: PgPool) {
    let mut saisie = saisie_vehicule("AA-000-AA");
    saisie.date_mise_en_service = "01/06/2023".to_string();
    assert!(vehicules::creer(&pool, &saisie).await.is_err());
    assert!(vehicules::lister(&pool).await.unwrap().is_empty());
}

#[sqlx::test]
async fn une_intervention_releve_le_kilometrage(pool: PgPool) {
    let id = creer_vehicule(&pool, "KM-100-KM").await;
    let saisie = SaisieIntervention {
        vehicule_id: id.to_string(),
        type_intervention: "revision".to_string(),
        date_intervention: "2026-05-05".to_string(),
        kilometrage: "20000".to_string(),
        cout: "189,90".to_string(),
        prestataire: "Garage du centre".to_string(),
        notes: String::new(),
    };
    let intervention = interventions::creer(&pool, &saisie).await.unwrap();
    assert_eq!(intervention.cout_centimes, 18990);
    assert_eq!(intervention.cout_fr(), "189,90 €");
    assert_eq!(
        vehicules::obtenir(&pool, id).await.unwrap().kilometrage,
        20000
    );

    // Une intervention plus ancienne, avec moins de kilomètres, ne fait pas reculer le compteur.
    let mut ancienne = saisie.clone();
    ancienne.kilometrage = "8000".to_string();
    interventions::creer(&pool, &ancienne).await.unwrap();
    assert_eq!(
        vehicules::obtenir(&pool, id).await.unwrap().kilometrage,
        20000
    );
}

/// Les vocabulaires Rust et les contraintes CHECK de la migration doivent accepter exactement
/// les mêmes codes : on insère chaque code et on vérifie que la base l'accepte.
#[sqlx::test]
async fn codes_acceptes_par_la_base(pool: PgPool) {
    for statut in StatutVehicule::TOUS {
        for carburant in Carburant::TOUS {
            let mut saisie = saisie_vehicule(&format!("{}-{}", statut.code(), carburant.code()));
            saisie.statut = statut.code().to_string();
            saisie.carburant = carburant.code().to_string();
            vehicules::creer(&pool, &saisie)
                .await
                .unwrap_or_else(|e| panic!("{statut:?}/{carburant:?} refusé : {e}"));
        }
    }

    let vehicule = creer_vehicule(&pool, "VOCAB-1").await;
    for type_echeance in TypeEcheance::TOUS {
        for statut in StatutEcheance::TOUS {
            sqlx::query(
                "INSERT INTO echeances (vehicule_id, type_echeance, date_limite, statut)
                 VALUES ($1, $2, '2027-01-01', $3)",
            )
            .bind(vehicule)
            .bind(type_echeance.code())
            .bind(statut.code())
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("{type_echeance:?}/{statut:?} refusé : {e}"));
        }
    }
    for type_intervention in TypeIntervention::TOUS {
        sqlx::query(
            "INSERT INTO interventions (vehicule_id, type_intervention, date_intervention, kilometrage)
             VALUES ($1, $2, '2026-01-01', 1)",
        )
        .bind(vehicule)
        .bind(type_intervention.code())
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("{type_intervention:?} refusé : {e}"));
    }
    for statut in StatutConducteur::TOUS {
        sqlx::query(
            "INSERT INTO conducteurs (nom, prenom, date_expiration_permis, statut)
             VALUES ('Test', 'Test', '2030-01-01', $1)",
        )
        .bind(statut.code())
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("{statut:?} refusé : {e}"));
    }

    // Et un code hors vocabulaire est bien refusé par la base elle-même.
    let erreur = sqlx::query("UPDATE vehicules SET statut = 'vendu' WHERE id = $1")
        .bind(vehicule)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(gestion_parc_auto::erreurs::a_le_code(
        &erreur,
        gestion_parc_auto::erreurs::CODE_VIOLATION_CHECK
    ));
}
