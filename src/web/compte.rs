//! « Mon compte » : la page de son propre compte, le changement de son mot de passe (#172) et
//! son adresse de courriel avec le choix du rappel du matin (#182).
//!
//! Ce sont les deux seules routes d'écriture ouvertes à un lecteur : il n'écrit que sur son
//! propre compte, le mot de passe après avoir donné l'actuel. La règle « toute route qui écrit
//! prend un `Administrateur` » (`docs/architecture.md`, § 5) a ici ses deux exceptions, visibles
//! dans la signature : `UtilisateurConnecte`.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::utilisateurs::{self, SaisieCourriel, SaisieMotDePasse};
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/compte", get(page))
        .route("/compte/mot-de-passe", post(changer_mot_de_passe))
        .route("/compte/courriel", post(changer_courriel))
}

#[derive(Template)]
#[template(path = "compte.html")]
struct PageCompte {
    ctx: Contexte,
    role: &'static str,
    /// L'adresse et la case du rappel, telles qu'enregistrées ou telles que saisies si refusées.
    saisie: SaisieCourriel,
    erreur: Option<String>,
}

fn page_compte(
    utilisateur: &UtilisateurConnecte,
    saisie: SaisieCourriel,
    erreur: Option<String>,
) -> PageCompte {
    PageCompte {
        ctx: utilisateur.contexte("compte"),
        role: if utilisateur.est_admin() {
            "administrateur"
        } else {
            "lecteur (consultation seule)"
        },
        saisie,
        erreur,
    }
}

async fn page(utilisateur: UtilisateurConnecte) -> Resultat<Html<String>> {
    let saisie = SaisieCourriel::depuis(&utilisateur.utilisateur);
    rendre(page_compte(&utilisateur, saisie, None))
}

async fn changer_courriel(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieCourriel>,
) -> Resultat<Response> {
    match utilisateurs::definir_courriel(&etat.pool, utilisateur.utilisateur.id, &saisie).await {
        Ok(compte) => {
            // L'adresse elle-même ne va pas dans le journal : une donnée personnelle (#60).
            tracing::info!(login = %compte.login, rappels = compte.rappels, "courriel changé");
            Ok(fait::rediriger("/compte", Fait::CourrielChange))
        }
        Err(ErreurApp::Metier(message)) => {
            Ok(rendre(page_compte(&utilisateur, saisie, Some(message)))?.into_response())
        }
        Err(autre) => Err(autre),
    }
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
            let saisie = SaisieCourriel::depuis(&utilisateur.utilisateur);
            Ok(rendre(page_compte(&utilisateur, saisie, Some(message)))?.into_response())
        }
        Err(autre) => Err(autre),
    }
}
