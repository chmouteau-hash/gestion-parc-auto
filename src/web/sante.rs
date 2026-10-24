//! `GET /health` : la sonde de l'hébergement (#63, bloc 2).
//!
//! Anonyme, comme la page de connexion : une sonde n'a pas de compte. `200 ok` quand le serveur
//! tourne et que la base répond, `503` sinon ; le texte ne dit rien de plus (ni version, ni
//! chiffres) pour ne rien apprendre à qui n'est pas connecté. Jamais mise en cache : une sonde
//! veut l'état de l'instant.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::etat::Etat;
use crate::metier::sante::{self, Sante};

pub fn routes() -> Router<Etat> {
    Router::new().route("/health", get(health))
}

async fn health(State(etat): State<Etat>) -> Response {
    let (statut, corps) = match sante::verifier(&etat.pool).await {
        Sante::Ok => (StatusCode::OK, "ok"),
        Sante::BaseInjoignable => (StatusCode::SERVICE_UNAVAILABLE, "base injoignable"),
    };
    (
        statut,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        corps,
    )
        .into_response()
}
