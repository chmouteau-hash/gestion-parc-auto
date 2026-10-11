//! Type d'erreur unique de l'application et sa traduction en réponse HTTP.
//!
//! Une seule énumération traverse les trois couches. La couche `web` n'a pas à connaître le
//! détail des erreurs SQL : la couche `metier` les traduit en [`ErreurApp::Metier`] quand elles
//! ont un sens pour l'utilisateur (doublon, référence inconnue) et laisse le reste en
//! [`ErreurApp::Technique`].

use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};

#[derive(Debug, thiserror::Error)]
pub enum ErreurApp {
    /// Saisie invalide ou règle de gestion non respectée. Le message est destiné à l'écran.
    #[error("{0}")]
    Metier(String),
    /// La ressource demandée n'existe pas (ou plus).
    #[error("Ressource introuvable")]
    Introuvable,
    /// Aucun utilisateur connecté : la page redirige vers la connexion.
    #[error("Connexion requise")]
    NonConnecte,
    /// Utilisateur connecté mais sans le rôle nécessaire.
    #[error("Accès refusé : cette action est réservée aux administrateurs")]
    Interdit,
    /// Panne (base inaccessible, disque plein, bogue). Journalisée, jamais détaillée à l'écran.
    #[error(transparent)]
    Technique(#[from] anyhow::Error),
}

pub type Resultat<T> = Result<T, ErreurApp>;

impl ErreurApp {
    /// Raccourci pour une erreur métier.
    pub fn metier(message: impl Into<String>) -> ErreurApp {
        ErreurApp::Metier(message.into())
    }
}

impl From<sqlx::Error> for ErreurApp {
    fn from(erreur: sqlx::Error) -> ErreurApp {
        match erreur {
            sqlx::Error::RowNotFound => ErreurApp::Introuvable,
            autre => ErreurApp::Technique(anyhow::Error::new(autre).context("erreur PostgreSQL")),
        }
    }
}

impl From<askama::Error> for ErreurApp {
    fn from(erreur: askama::Error) -> ErreurApp {
        ErreurApp::Technique(anyhow::Error::new(erreur).context("rendu du template"))
    }
}

impl From<std::io::Error> for ErreurApp {
    fn from(erreur: std::io::Error) -> ErreurApp {
        ErreurApp::Technique(anyhow::Error::new(erreur).context("entrée/sortie"))
    }
}

/// Codes d'erreur PostgreSQL (classe 23 : violation d'intégrité).
pub const CODE_VIOLATION_UNICITE: &str = "23505";
pub const CODE_VIOLATION_CLE_ETRANGERE: &str = "23503";
pub const CODE_VIOLATION_CHECK: &str = "23514";

/// Nom de la contrainte PostgreSQL violée, s'il y en a une.
pub fn contrainte_violee(erreur: &sqlx::Error) -> Option<String> {
    match erreur {
        sqlx::Error::Database(db) => db.constraint().map(str::to_string),
        _ => None,
    }
}

/// `true` si l'erreur SQL porte ce code (ex. [`CODE_VIOLATION_UNICITE`]).
pub fn a_le_code(erreur: &sqlx::Error, code: &str) -> bool {
    match erreur {
        sqlx::Error::Database(db) => db.code().as_deref() == Some(code),
        _ => false,
    }
}

/// Page d'erreur autonome (sans menu : on ne sait pas toujours qui est connecté).
#[derive(askama::Template)]
#[template(path = "erreur.html")]
struct PageErreur<'a> {
    titre: &'a str,
    message: &'a str,
}

fn page_erreur(statut: StatusCode, titre: &str, message: &str) -> Response {
    let corps = PageErreur { titre, message }
        .render()
        .unwrap_or_else(|_| format!("{titre} : {message}"));
    (statut, Html(corps)).into_response()
}

impl IntoResponse for ErreurApp {
    fn into_response(self) -> Response {
        match self {
            ErreurApp::Metier(message) => {
                page_erreur(StatusCode::BAD_REQUEST, "Saisie refusée", &message)
            }
            ErreurApp::Introuvable => page_erreur(
                StatusCode::NOT_FOUND,
                "Introuvable",
                "Cette page ou cet élément n'existe pas.",
            ),
            ErreurApp::NonConnecte => Redirect::to("/connexion").into_response(),
            ErreurApp::Interdit => page_erreur(
                StatusCode::FORBIDDEN,
                "Accès refusé",
                "Cette action est réservée aux administrateurs.",
            ),
            ErreurApp::Technique(erreur) => {
                tracing::error!(erreur = ?erreur, "erreur technique");
                page_erreur(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erreur technique",
                    "Une erreur interne s'est produite. Elle a été journalisée.",
                )
            }
        }
    }
}
