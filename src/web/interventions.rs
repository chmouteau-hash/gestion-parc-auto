//! Pages des interventions d'entretien.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Choix, TypeIntervention};
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::interventions::{self, SaisieIntervention};
use crate::metier::vehicules;
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/interventions", get(liste).post(creer))
        .route("/interventions/nouvelle", get(nouvelle))
        .route("/interventions/{id}", post(modifier))
        .route("/interventions/{id}/modifier", get(editer))
        .route("/interventions/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "interventions/liste.html")]
struct PageListe {
    ctx: Contexte,
    interventions: Vec<Intervention>,
}

#[derive(Template)]
#[template(path = "interventions/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieIntervention,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    types: Vec<Choix>,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    saisie: SaisieIntervention,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle intervention".to_string(),
            Some(_) => "Modifier l'intervention".to_string(),
        },
        action: match id {
            None => "/interventions".to_string(),
            Some(id) => format!("/interventions/{id}"),
        },
        saisie,
        erreur,
        vehicules: vehicules::lister(&etat.pool).await?,
        types: TypeIntervention::choix(),
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("interventions"),
        interventions: interventions::lister(&etat.pool).await?,
    })
}

async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieIntervention>,
) -> Resultat<Html<String>> {
    if saisie.date_intervention.is_empty() {
        saisie.date_intervention = chrono::Local::now().date_naive().to_string();
    }
    rendre(formulaire(&etat, admin.contexte("interventions"), None, saisie, None).await?)
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieIntervention>,
) -> Resultat<Response> {
    match interventions::creer(&etat.pool, &saisie).await {
        Ok(i) => Ok(Redirect::to(&format!("/vehicules/{}", i.vehicule_id)).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("interventions"),
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
    let intervention = interventions::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("interventions"),
            Some(id),
            SaisieIntervention::depuis(&intervention),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
    Form(saisie): Form<SaisieIntervention>,
) -> Resultat<Response> {
    match interventions::modifier(&etat.pool, id, &saisie).await {
        Ok(i) => Ok(Redirect::to(&format!("/vehicules/{}", i.vehicule_id)).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("interventions"),
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

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    interventions::supprimer(&etat.pool, id).await?;
    Ok(Redirect::to("/interventions"))
}
