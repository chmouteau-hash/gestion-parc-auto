//! Pages des affectations véhicule ↔ conducteur.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::donnees::affectations::Affectation;
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::affectations::{self, SaisieAffectation};
use crate::metier::{conducteurs, vehicules};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/affectations", get(liste).post(creer))
        .route("/affectations/nouvelle", get(nouvelle))
        .route("/affectations/{id}", post(modifier))
        .route("/affectations/{id}/modifier", get(editer))
        .route("/affectations/{id}/cloturer", post(cloturer))
        .route("/affectations/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "affectations/liste.html")]
struct PageListe {
    ctx: Contexte,
    affectations: Vec<Affectation>,
}

#[derive(Template)]
#[template(path = "affectations/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieAffectation,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    conducteurs: Vec<Conducteur>,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    saisie: SaisieAffectation,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle affectation".to_string(),
            Some(_) => "Modifier l'affectation".to_string(),
        },
        action: match id {
            None => "/affectations".to_string(),
            Some(id) => format!("/affectations/{id}"),
        },
        saisie,
        erreur,
        vehicules: vehicules::lister(&etat.pool).await?,
        conducteurs: conducteurs::lister(&etat.pool).await?,
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("affectations"),
        affectations: affectations::lister(&etat.pool).await?,
    })
}

async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieAffectation>,
) -> Resultat<Html<String>> {
    if saisie.date_debut.is_empty() {
        saisie.date_debut = chrono::Local::now().date_naive().to_string();
    }
    rendre(formulaire(&etat, admin.contexte("affectations"), None, saisie, None).await?)
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieAffectation>,
) -> Resultat<Response> {
    match affectations::creer(&etat.pool, &saisie).await {
        Ok(_) => Ok(Redirect::to("/affectations").into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("affectations"),
                None,
                saisie,
                Some(message),
            )
            .await?,
        )?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

async fn editer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Html<String>> {
    let affectation = affectations::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("affectations"),
            Some(id),
            SaisieAffectation::depuis(&affectation),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
    Form(saisie): Form<SaisieAffectation>,
) -> Resultat<Response> {
    match affectations::modifier(&etat.pool, id, &saisie).await {
        Ok(_) => Ok(Redirect::to("/affectations").into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("affectations"),
                Some(id),
                saisie,
                Some(message),
            )
            .await?,
        )?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

/// Clôture à la date du jour (bouton « Terminer » de la liste).
async fn cloturer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    affectations::cloturer(&etat.pool, id, chrono::Local::now().date_naive()).await?;
    Ok(Redirect::to("/affectations"))
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    affectations::supprimer(&etat.pool, id).await?;
    Ok(Redirect::to("/affectations"))
}
