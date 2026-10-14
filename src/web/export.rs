//! Export CSV des listes (`/vehicules.csv`, `/conducteurs.csv`, `/affectations.csv`,
//! `/interventions.csv`, `/echeances.csv`), pour un tableur. Les mêmes colonnes que les listes
//! à l'écran, les mêmes libellés ; les nombres sans espace de milliers, pour que le tableur les
//! lise comme des nombres. Les pages ne proposent le lien qu'à une entreprise (Autoclair Pro),
//! mais tout compte connecté peut appeler l'adresse.

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::commun::csv;
use crate::commun::format::date_fr;
use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::metier::{affectations, conducteurs, echeances, interventions, vehicules};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/vehicules.csv", get(exporter_vehicules))
        .route("/conducteurs.csv", get(exporter_conducteurs))
        .route("/affectations.csv", get(exporter_affectations))
        .route("/interventions.csv", get(exporter_interventions))
        .route("/echeances.csv", get(exporter_echeances))
}

/// Le fichier, à télécharger, nommé d'après la liste et le jour.
fn fichier(nom: &str, contenu: String) -> Response {
    let jour = chrono::Local::now().date_naive();
    let disposition = format!("attachment; filename=\"{nom}-{jour}.csv\"");
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&disposition)
                    .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
            ),
        ],
        contenu,
    )
        .into_response()
}

/// Un montant en centimes, écrit `149,90`.
fn euros(centimes: i64) -> String {
    format!("{},{:02}", centimes / 100, centimes % 100)
}

async fn exporter_vehicules(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Response> {
    let lignes = vehicules::lister(&etat.pool)
        .await?
        .into_iter()
        .map(|v| {
            vec![
                v.immatriculation.clone(),
                v.vin.clone(),
                v.marque.clone(),
                v.modele.clone(),
                v.date_mise_en_service_fr(),
                v.carburant_libelle().to_string(),
                v.kilometrage.to_string(),
                v.statut_libelle().to_string(),
            ]
        })
        .collect::<Vec<_>>();
    Ok(fichier(
        "vehicules",
        csv::document(
            &[
                "Immatriculation",
                "VIN",
                "Marque",
                "Modèle",
                "Mise en service",
                "Carburant",
                "Kilométrage (km)",
                "Statut",
            ],
            &lignes,
        ),
    ))
}

async fn exporter_conducteurs(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Response> {
    let lignes = conducteurs::lister(&etat.pool)
        .await?
        .into_iter()
        .map(|c| {
            vec![
                c.nom.clone(),
                c.prenom.clone(),
                c.service.clone(),
                c.date_expiration_permis_fr(),
                c.statut_libelle().to_string(),
            ]
        })
        .collect::<Vec<_>>();
    Ok(fichier(
        "conducteurs",
        csv::document(
            &["Nom", "Prénom", "Service", "Expiration du permis", "Statut"],
            &lignes,
        ),
    ))
}

async fn exporter_affectations(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    let lignes = affectations::lister(&etat.pool, aujourd_hui)
        .await?
        .into_iter()
        .map(|a| {
            vec![
                a.immatriculation.clone(),
                a.conducteur_nom_complet(),
                a.date_debut_fr(),
                a.date_fin.map(date_fr).unwrap_or_default(),
                a.etat(aujourd_hui).libelle().to_string(),
            ]
        })
        .collect::<Vec<_>>();
    Ok(fichier(
        "affectations",
        csv::document(&["Véhicule", "Conducteur", "Début", "Fin", "État"], &lignes),
    ))
}

async fn exporter_interventions(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Response> {
    let lignes = interventions::lister(&etat.pool)
        .await?
        .into_iter()
        .map(|i| {
            vec![
                i.date_intervention_fr(),
                i.immatriculation.clone(),
                i.type_libelle().to_string(),
                i.kilometrage.to_string(),
                euros(i.cout_centimes),
                i.prestataire.clone(),
                i.notes.clone(),
            ]
        })
        .collect::<Vec<_>>();
    Ok(fichier(
        "interventions",
        csv::document(
            &[
                "Date",
                "Véhicule",
                "Type",
                "Kilométrage (km)",
                "Coût (€)",
                "Prestataire",
                "Notes",
            ],
            &lignes,
        ),
    ))
}

async fn exporter_echeances(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Response> {
    let lignes = echeances::lister(&etat.pool)
        .await?
        .into_iter()
        .map(|e| {
            vec![
                e.immatriculation.clone(),
                e.type_libelle().to_string(),
                e.date_limite_fr(),
                e.statut_libelle().to_string(),
            ]
        })
        .collect::<Vec<_>>();
    Ok(fichier(
        "echeances",
        csv::document(&["Véhicule", "Type", "Date limite", "Statut"], &lignes),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_euros_s_ecrivent_avec_une_virgule() {
        assert_eq!(euros(14990), "149,90");
        assert_eq!(euros(5), "0,05");
        assert_eq!(euros(0), "0,00");
    }
}
