//! Dépôt, téléchargement et suppression des documents rattachés à un véhicule ou un conducteur.
//!
//! Le téléchargement passe par une route authentifiée et renvoie le fichier en pièce jointe
//! (`Content-Disposition: attachment`) : un fichier HTML déposé n'est jamais exécuté par le
//! navigateur dans le contexte du site.

use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;

use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::documents::{self, nom_telechargement, Rattachement};
use crate::metier::validation::identifiant;
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/documents", post(deposer))
        .route("/documents/{id}/telecharger", get(telecharger))
        .route("/documents/{id}/supprimer", post(supprimer))
}

/// Formulaire `multipart/form-data` avec un champ `vehicule_id` **ou** `conducteur_id`, puis un
/// champ `fichier`. Les champs sont lus dans l'ordre d'envoi.
async fn deposer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    mut multipart: Multipart,
) -> Resultat<Redirect> {
    let mut rattachement: Option<Rattachement> = None;
    let mut fichier: Option<(String, String, Vec<u8>)> = None;

    while let Some(champ) = multipart
        .next_field()
        .await
        .map_err(|e| ErreurApp::metier(format!("Envoi du formulaire invalide : {e}")))?
    {
        match champ.name().unwrap_or("") {
            "vehicule_id" => {
                let valeur = champ.text().await.map_err(|e| anyhow::anyhow!(e))?;
                rattachement = Some(Rattachement::Vehicule(identifiant(&valeur, "véhicule")?));
            }
            "conducteur_id" => {
                let valeur = champ.text().await.map_err(|e| anyhow::anyhow!(e))?;
                rattachement = Some(Rattachement::Conducteur(identifiant(
                    &valeur,
                    "conducteur",
                )?));
            }
            "fichier" => {
                let nom = champ.file_name().unwrap_or("").to_string();
                let type_mime = champ.content_type().unwrap_or("").to_string();
                let octets = champ.bytes().await.map_err(|e| {
                    ErreurApp::metier(format!("Fichier non reçu (trop volumineux ?) : {e}"))
                })?;
                fichier = Some((nom, type_mime, octets.to_vec()));
            }
            _ => {}
        }
    }

    let rattachement = rattachement.ok_or_else(|| {
        ErreurApp::metier("Le document doit être rattaché à un véhicule ou à un conducteur.")
    })?;
    let (nom, type_mime, octets) =
        fichier.ok_or_else(|| ErreurApp::metier("Aucun fichier n'a été choisi."))?;

    documents::deposer(
        &etat.pool,
        &etat.config.documents_dir,
        rattachement,
        &nom,
        &type_mime,
        &octets,
    )
    .await?;

    Ok(Redirect::to(&page_de_retour(rattachement)))
}

async fn telecharger(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Response> {
    let (document, contenu) = documents::lire(&etat.pool, &etat.config.documents_dir, id).await?;
    let type_mime = HeaderValue::from_str(&document.type_mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let disposition = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        nom_telechargement(&document.nom_original)
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, type_mime),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        contenu,
    )
        .into_response())
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    let document = documents::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    let retour = match (document.vehicule_id, document.conducteur_id) {
        (Some(id), _) => page_de_retour(Rattachement::Vehicule(id)),
        (_, Some(id)) => page_de_retour(Rattachement::Conducteur(id)),
        _ => "/".to_string(),
    };
    Ok(Redirect::to(&retour))
}

fn page_de_retour(rattachement: Rattachement) -> String {
    match rattachement {
        Rattachement::Vehicule(id) => format!("/vehicules/{id}"),
        Rattachement::Conducteur(id) => format!("/conducteurs/{id}"),
    }
}
