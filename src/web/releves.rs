//! Corriger un relevé du compteur (#94, bloc 4 ; retour #4) : le modifier ou le supprimer depuis
//! l'historique du compteur. Le compteur du véhicule suit. Les interventions et les pleins de
//! l'historique se corrigent dans leurs propres formulaires.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use chrono::NaiveDate;

use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::releves::{self, SaisieReleve};
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::Administrateur;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/releves/{id}", post(modifier))
        .route("/releves/{id}/modifier", get(editer))
        .route("/releves/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "releves/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    id: i64,
    vehicule: Vehicule,
    saisie: SaisieReleve,
    erreur: Option<String>,
    /// Borne du champ date : un relevé n'est pas dans le futur.
    aujourd_hui: NaiveDate,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: i64,
    vehicule_id: i64,
    saisie: SaisieReleve,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let vehicule = vehicules::obtenir(&etat.pool, vehicule_id).await?;
    Ok(PageFormulaire {
        ctx,
        titre: format!("Corriger un relevé · {}", vehicule.immatriculation),
        id,
        vehicule,
        saisie,
        erreur,
        aujourd_hui: chrono::Local::now().date_naive(),
    })
}

async fn editer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let releve = releves::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("vehicules"),
            id,
            releve.vehicule_id,
            SaisieReleve::depuis(&releve),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieReleve>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    match releves::modifier(&etat.pool, id, &saisie, aujourd_hui).await {
        Ok(releve) => Ok(fait::rediriger(
            &format!("/vehicules/{}/compteur", releve.vehicule_id),
            Fait::ReleveModifie,
        )),
        Err(ErreurApp::Metier(message)) => {
            let releve = releves::obtenir(&etat.pool, id).await?;
            let page = formulaire(
                &etat,
                admin.contexte("vehicules"),
                id,
                releve.vehicule_id,
                saisie,
                Some(message),
            )
            .await?;
            Ok(rendre(page)?.into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let vehicule_id = releves::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger(
        &format!("/vehicules/{vehicule_id}/compteur"),
        Fait::ReleveSupprime,
    ))
}
