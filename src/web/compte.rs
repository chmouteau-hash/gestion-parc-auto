//! « Mon compte » : la page de son propre compte et le changement de son mot de passe (#172).
//!
//! C'est la seule route d'écriture ouverte à un lecteur : il n'écrit que son propre mot de
//! passe, après avoir donné l'actuel. La règle « toute route qui écrit prend un
//! `Administrateur` » (`docs/architecture.md`, § 5) a ici son unique exception, visible dans la
//! signature : `UtilisateurConnecte`.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::utilisateurs::{self, SaisieMotDePasse};
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/compte", get(page))
        .route("/compte/mot-de-passe", post(changer_mot_de_passe))
}

#[derive(Template)]
#[template(path = "compte.html")]
struct PageCompte {
    ctx: Contexte,
    role: &'static str,
    erreur: Option<String>,
}

fn page_compte(utilisateur: &UtilisateurConnecte, erreur: Option<String>) -> PageCompte {
    PageCompte {
        ctx: utilisateur.contexte("compte"),
        role: if utilisateur.est_admin() {
            "administrateur"
        } else {
            "lecteur (consultation seule)"
        },
        erreur,
    }
}

async fn page(utilisateur: UtilisateurConnecte) -> Resultat<Html<String>> {
    rendre(page_compte(&utilisateur, None))
}

async fn changer_mot_de_passe(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieMotDePasse>,
) -> Resultat<Response> {
    match utilisateurs::changer_son_mot_de_passe(&etat.pool, &utilisateur.utilisateur, &saisie)
        .await
    {
        Ok(()) => {
            tracing::info!(login = %utilisateur.utilisateur.login, "mot de passe changé");
            Ok(fait::rediriger("/compte", Fait::MotDePasseChange))
        }
        Err(ErreurApp::Metier(message)) => {
            Ok(rendre(page_compte(&utilisateur, Some(message)))?.into_response())
        }
        Err(autre) => Err(autre),
    }
}
