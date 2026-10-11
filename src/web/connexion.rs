//! Pages de connexion et de déconnexion : les seules routes accessibles sans session.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::Deserialize;

use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::authentification;
use crate::web::rendu::rendre;
use crate::web::session::{cookie_session, cookie_suppression, valeur_cookie};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/connexion", get(formulaire).post(connecter))
        .route("/deconnexion", post(deconnecter))
}

#[derive(Template)]
#[template(path = "connexion.html")]
struct PageConnexion {
    erreur: Option<String>,
    login: String,
}

#[derive(Debug, Deserialize)]
struct SaisieConnexion {
    #[serde(default)]
    login: String,
    #[serde(default)]
    mot_de_passe: String,
}

async fn formulaire() -> Resultat<Html<String>> {
    rendre(PageConnexion {
        erreur: None,
        login: String::new(),
    })
}

async fn connecter(
    State(etat): State<Etat>,
    jar: PrivateCookieJar,
    Form(saisie): Form<SaisieConnexion>,
) -> Resultat<Response> {
    match authentification::connecter(&etat.pool, &saisie.login, &saisie.mot_de_passe).await {
        Ok(utilisateur) => {
            let valeur = valeur_cookie(utilisateur.id, chrono::Utc::now().timestamp());
            let jar = jar.add(cookie_session(valeur, etat.config.cookies_securises));
            tracing::info!(login = %utilisateur.login, "connexion");
            Ok((jar, Redirect::to("/")).into_response())
        }
        Err(ErreurApp::Metier(message)) => {
            tracing::warn!(login = %saisie.login, "connexion refusée");
            Ok(rendre(PageConnexion {
                erreur: Some(message),
                login: saisie.login,
            })?
            .into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn deconnecter(jar: PrivateCookieJar) -> Response {
    (jar.remove(cookie_suppression()), Redirect::to("/connexion")).into_response()
}
