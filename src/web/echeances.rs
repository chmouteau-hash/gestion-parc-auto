//! Pages des échéances réglementaires et d'entretien.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Choix, StatutEcheance, TypeEcheance};
use crate::donnees::echeances::Echeance;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::echeances::{self, SaisieEcheance};
use crate::metier::vehicules;
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/echeances", get(liste).post(creer))
        .route("/echeances/nouvelle", get(nouvelle))
        .route("/echeances/{id}", post(modifier))
        .route("/echeances/{id}/modifier", get(editer))
        .route("/echeances/{id}/traiter", post(traiter))
        .route("/echeances/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "echeances/liste.html")]
struct PageListe {
    ctx: Contexte,
    echeances: Vec<Echeance>,
}

#[derive(Template)]
#[template(path = "echeances/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieEcheance,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    types: Vec<Choix>,
    statuts: Vec<Choix>,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    mut saisie: SaisieEcheance,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    if saisie.statut.is_empty() {
        saisie.statut = StatutEcheance::Ouverte.code().to_string();
    }
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle échéance".to_string(),
            Some(_) => "Modifier l'échéance".to_string(),
        },
        action: match id {
            None => "/echeances".to_string(),
            Some(id) => format!("/echeances/{id}"),
        },
        saisie,
        erreur,
        vehicules: vehicules::lister(&etat.pool).await?,
        types: TypeEcheance::choix(),
        statuts: StatutEcheance::choix(),
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("echeances"),
        echeances: echeances::lister(&etat.pool).await?,
    })
}

async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(saisie): Query<SaisieEcheance>,
) -> Resultat<Html<String>> {
    rendre(formulaire(&etat, admin.contexte("echeances"), None, saisie, None).await?)
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieEcheance>,
) -> Resultat<Response> {
    match echeances::creer(&etat.pool, &saisie).await {
        Ok(e) => Ok(Redirect::to(&format!("/vehicules/{}", e.vehicule_id)).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("echeances"),
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
    let echeance = echeances::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("echeances"),
            Some(id),
            SaisieEcheance::depuis(&echeance),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
    Form(saisie): Form<SaisieEcheance>,
) -> Resultat<Response> {
    match echeances::modifier(&etat.pool, id, &saisie).await {
        Ok(e) => Ok(Redirect::to(&format!("/vehicules/{}", e.vehicule_id)).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("echeances"),
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

/// Bouton « Traitée » de la vue alertes et de la liste.
async fn traiter(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    echeances::traiter(&etat.pool, id).await?;
    Ok(Redirect::to("/alertes"))
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    echeances::supprimer(&etat.pool, id).await?;
    Ok(Redirect::to("/echeances"))
}
