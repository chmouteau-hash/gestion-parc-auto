//! Pages de connexion et de déconnexion : les seules routes accessibles sans session.

use std::time::Instant;

use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::Deserialize;

use crate::commun::types::Profil;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::{authentification, frein};
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
    profil: Profil,
}

#[derive(Debug, Deserialize)]
struct SaisieConnexion {
    #[serde(default)]
    login: String,
    #[serde(default)]
    mot_de_passe: String,
}

async fn formulaire(State(etat): State<Etat>) -> Resultat<Html<String>> {
    rendre(PageConnexion {
        erreur: None,
        login: String::new(),
        profil: etat.config.profil,
    })
}

async fn connecter(
    State(etat): State<Etat>,
    jar: PrivateCookieJar,
    Form(saisie): Form<SaisieConnexion>,
) -> Resultat<Response> {
    // Identifiant bloqué par le frein (#170) : on ne vérifie même pas le mot de passe.
    let maintenant = Instant::now();
    if let Some(reste) = etat.frein.attente(&saisie.login, maintenant) {
        tracing::warn!(login = %saisie.login, "connexion refusée : identifiant bloqué");
        let message = format!(
            "Trop d'essais pour cet identifiant : réessayez dans {}.",
            frein::libelle_attente(reste)
        );
        return refus(&etat, saisie.login, message, StatusCode::TOO_MANY_REQUESTS);
    }
    match authentification::connecter(&etat.pool, &saisie.login, &saisie.mot_de_passe).await {
        Ok(utilisateur) => {
            etat.frein.reussite(&saisie.login);
            let valeur = valeur_cookie(utilisateur.id, chrono::Utc::now().timestamp());
            let jar = jar.add(cookie_session(valeur, etat.config.cookies_securises));
            tracing::info!(login = %utilisateur.login, "connexion");
            Ok((jar, Redirect::to("/")).into_response())
        }
        Err(ErreurApp::Metier(message)) => match etat.frein.echec(&saisie.login, maintenant) {
            Some(blocage) => {
                let attente = frein::libelle_attente(blocage);
                tracing::warn!(login = %saisie.login, "connexion refusée : identifiant bloqué {attente}");
                let message = format!("{message} Trop d'essais : réessayez dans {attente}.");
                refus(&etat, saisie.login, message, StatusCode::TOO_MANY_REQUESTS)
            }
            None => {
                tracing::warn!(login = %saisie.login, "connexion refusée");
                refus(&etat, saisie.login, message, StatusCode::OK)
            }
        },
        Err(autre) => Err(autre),
    }
}

/// Le formulaire ré-affiché avec son message, et le code HTTP qui va avec.
fn refus(etat: &Etat, login: String, erreur: String, statut: StatusCode) -> Resultat<Response> {
    let page = rendre(PageConnexion {
        erreur: Some(erreur),
        login,
        profil: etat.config.profil,
    })?;
    Ok((statut, page).into_response())
}

async fn deconnecter(jar: PrivateCookieJar) -> Response {
    (jar.remove(cookie_suppression()), Redirect::to("/connexion")).into_response()
}
