//! Dépôt, téléchargement et suppression des documents rattachés à un véhicule ou un conducteur.
//!
//! Le téléchargement passe par une route authentifiée et renvoie le fichier en pièce jointe
//! (`Content-Disposition: attachment`) : un fichier HTML déposé n'est jamais exécuté par le
//! navigateur dans le contexte du site.

use axum::extract::multipart::MultipartError;
use axum::extract::{Multipart, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;

use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::documents::{self, nom_telechargement, Rattachement};
use crate::web::chemin::Identifiant;
use crate::web::session::{Administrateur, UtilisateurConnecte};
use crate::web::{conducteurs, vehicules};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/vehicules/{id}/documents", post(deposer_pour_vehicule))
        .route("/conducteurs/{id}/documents", post(deposer_pour_conducteur))
        .route("/documents/{id}/telecharger", get(telecharger))
        .route("/documents/{id}/supprimer", post(supprimer))
}

async fn deposer_pour_vehicule(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    multipart: Multipart,
) -> Resultat<Response> {
    deposer(&admin, &etat, Rattachement::Vehicule(id), multipart).await
}

async fn deposer_pour_conducteur(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    multipart: Multipart,
) -> Resultat<Response> {
    deposer(&admin, &etat, Rattachement::Conducteur(id), multipart).await
}

/// Reçoit le formulaire de dépôt (`multipart/form-data`, champ `fichier`).
///
/// C'est l'adresse qui dit à quoi rattacher le fichier. Elle est connue avant toute lecture de
/// l'envoi : un dépôt refusé (fichier trop gros, vide…) ré-affiche donc toujours la fiche d'où
/// il vient, avec le motif.
async fn deposer(
    admin: &Administrateur,
    etat: &Etat,
    rattachement: Rattachement,
    mut multipart: Multipart,
) -> Resultat<Response> {
    match recevoir(etat, rattachement, &mut multipart).await {
        Ok(()) => Ok(Redirect::to(&page_de_retour(rattachement)).into_response()),
        Err(ErreurApp::Metier(message)) => {
            let page = match rattachement {
                Rattachement::Vehicule(id) => {
                    let ctx = admin.contexte("vehicules");
                    vehicules::page_fiche(etat, ctx, id, Some(message)).await?
                }
                Rattachement::Conducteur(id) => {
                    let ctx = admin.contexte("conducteurs");
                    conducteurs::page_fiche(etat, ctx, id, Some(message)).await?
                }
            };
            Ok(page.into_response())
        }
        Err(autre) => Err(autre),
    }
}

/// Lit le fichier du formulaire et l'enregistre.
async fn recevoir(
    etat: &Etat,
    rattachement: Rattachement,
    multipart: &mut Multipart,
) -> Resultat<()> {
    let mut fichier: Option<(String, String, Vec<u8>)> = None;

    while let Some(champ) = multipart.next_field().await.map_err(refus_d_envoi)? {
        if champ.name() == Some("fichier") {
            let nom = champ.file_name().unwrap_or("").to_string();
            let type_mime = champ.content_type().unwrap_or("").to_string();
            let octets = champ.bytes().await.map_err(refus_d_envoi)?;
            fichier = Some((nom, type_mime, octets.to_vec()));
        }
    }

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

    Ok(())
}

/// Traduit un échec de lecture du formulaire. Le serveur cesse de lire dès que l'envoi dépasse
/// la taille permise : c'est le cas du fichier trop gros. Pour tout autre échec, le détail
/// technique (en anglais) va au journal, pas à l'écran.
fn refus_d_envoi(erreur: MultipartError) -> ErreurApp {
    if erreur.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ErreurApp::metier(documents::message_trop_volumineux())
    } else {
        tracing::warn!(%erreur, "formulaire de dépôt illisible");
        ErreurApp::metier("L'envoi du fichier a échoué. Réessayez.")
    }
}

async fn telecharger(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
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
    Identifiant(id): Identifiant,
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
