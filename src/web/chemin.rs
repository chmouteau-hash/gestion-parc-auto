//! Lecture de l'identifiant porté par l'adresse (`/vehicules/{id}`).
//!
//! L'extracteur `Path<i64>` d'Axum répond lui-même, en anglais et sans mise en page, quand
//! l'adresse ne contient pas un nombre (`/vehicules/abc`). [`Identifiant`] le remplace dans
//! toutes les routes : une adresse mal formée donne la même page « Introuvable » qu'un
//! identifiant qui n'existe pas.

use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;

use crate::erreurs::ErreurApp;
use crate::etat::Etat;

/// L'identifiant numérique d'une route `…/{id}`.
#[derive(Debug, Clone, Copy)]
pub struct Identifiant(pub i64);

impl FromRequestParts<Etat> for Identifiant {
    type Rejection = ErreurApp;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApp> {
        match Path::<i64>::from_request_parts(parts, etat).await {
            Ok(Path(id)) => Ok(Identifiant(id)),
            Err(rejet) => Err(traduire(rejet)),
        }
    }
}

/// Une adresse illisible est « introuvable ». Tout autre rejet signale une route mal déclarée
/// (pas de `{id}`, ou plusieurs) : c'est un bogue, journalisé comme tel.
fn traduire(rejet: PathRejection) -> ErreurApp {
    if rejet.status().is_client_error() {
        ErreurApp::Introuvable
    } else {
        ErreurApp::Technique(
            anyhow::anyhow!(rejet.body_text()).context("lecture de l'identifiant dans l'adresse"),
        )
    }
}
