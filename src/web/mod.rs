//! Couche HTTP : routeur, intergiciels, et une page par entité.
//!
//! Garanties de cette couche :
//! - aucune requête SQL : tout passe par `metier` ;
//! - toute route qui écrit (`POST`, `PUT`, `DELETE`) prend un [`session::Administrateur`] en
//!   paramètre ; toute route qui lit prend un [`session::UtilisateurConnecte`]. Les seules routes
//!   anonymes sont la page de connexion et les fichiers statiques ;
//! - les en-têtes de sécurité sont posés sur toutes les réponses par [`entetes_securite`].

pub mod api;
pub mod rendu;
pub mod session;

mod affectations;
mod alertes;
mod conducteurs;
mod connexion;
mod documents;
mod echeances;
mod interventions;
mod vehicules;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::erreurs::ErreurApp;
use crate::etat::Etat;
use crate::metier::documents::TAILLE_MAX_OCTETS;

/// Feuille de style et script, embarqués dans l'exécutable : pas de dossier à déployer à côté.
const STYLE: &str = include_str!("../../static/style.css");
const SCRIPT: &str = include_str!("../../static/app.js");

pub fn routeur(etat: Etat) -> Router {
    Router::new()
        .route("/", get(accueil))
        .route("/static/style.css", get(style))
        .route("/static/app.js", get(script))
        .merge(connexion::routes())
        .merge(alertes::routes())
        .merge(vehicules::routes())
        .merge(conducteurs::routes())
        .merge(affectations::routes())
        .merge(interventions::routes())
        .merge(echeances::routes())
        .merge(documents::routes())
        .nest("/api/v1", api::routes())
        .fallback(introuvable)
        .layer(middleware::from_fn(entetes_securite))
        // Un peu plus que la taille maximale d'un document : l'enveloppe multipart compte aussi.
        .layer(DefaultBodyLimit::max(TAILLE_MAX_OCTETS + 64 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(etat)
}

async fn accueil() -> Redirect {
    Redirect::to("/alertes")
}

async fn introuvable() -> ErreurApp {
    ErreurApp::Introuvable
}

async fn style() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE,
    )
        .into_response()
}

async fn script() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        SCRIPT,
    )
        .into_response()
}

/// En-têtes de sécurité standards, sur toutes les réponses.
///
/// - `Content-Security-Policy` : le navigateur ne charge style et script que depuis ce site, et
///   refuse tout script en ligne (ce qui neutralise la plupart des injections HTML) ;
/// - `X-Content-Type-Options: nosniff` : un fichier déposé n'est jamais « deviné » comme HTML ;
/// - `X-Frame-Options: DENY` : la page ne peut pas être incluse dans un site tiers (détournement
///   de clic) ;
/// - `Referrer-Policy` : l'adresse des pages internes ne fuit pas vers d'autres sites.
async fn entetes_securite(requete: Request, suite: Next) -> Response {
    let mut reponse = suite.run(requete).await;
    let entetes = reponse.headers_mut();
    entetes.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; frame-ancestors 'none'; form-action 'self'",
        ),
    );
    entetes.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    entetes.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    entetes.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    reponse
}
