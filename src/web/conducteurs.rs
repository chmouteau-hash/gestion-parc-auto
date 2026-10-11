//! Pages des conducteurs : liste, fiche, création, modification, suppression.

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Choix, StatutConducteur};
use crate::donnees::affectations::Affectation;
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::documents::Document;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::conducteurs::{self, SaisieConducteur};
use crate::metier::{affectations, documents};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/conducteurs", get(liste).post(creer))
        .route("/conducteurs/nouveau", get(nouveau))
        .route("/conducteurs/{id}", get(fiche).post(modifier))
        .route("/conducteurs/{id}/modifier", get(editer))
        .route("/conducteurs/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "conducteurs/liste.html")]
struct PageListe {
    ctx: Contexte,
    conducteurs: Vec<Conducteur>,
}

#[derive(Template)]
#[template(path = "conducteurs/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieConducteur,
    erreur: Option<String>,
    statuts: Vec<Choix>,
}

impl PageFormulaire {
    fn nouveau(ctx: Contexte, saisie: SaisieConducteur, erreur: Option<String>) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: "Nouveau conducteur".to_string(),
            action: "/conducteurs".to_string(),
            saisie,
            erreur,
            statuts: StatutConducteur::choix(),
        }
    }

    fn modification(
        ctx: Contexte,
        id: i64,
        saisie: SaisieConducteur,
        erreur: Option<String>,
    ) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: format!("Modifier {} {}", saisie.prenom, saisie.nom),
            action: format!("/conducteurs/{id}"),
            saisie,
            erreur,
            statuts: StatutConducteur::choix(),
        }
    }
}

#[derive(Template)]
#[template(path = "conducteurs/fiche.html")]
struct PageFiche {
    ctx: Contexte,
    conducteur: Conducteur,
    affectations: Vec<Affectation>,
    documents: Vec<Document>,
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("conducteurs"),
        conducteurs: conducteurs::lister(&etat.pool).await?,
    })
}

async fn fiche(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Html<String>> {
    rendre(PageFiche {
        ctx: utilisateur.contexte("conducteurs"),
        conducteur: conducteurs::obtenir(&etat.pool, id).await?,
        affectations: affectations::lister_par_conducteur(&etat.pool, id).await?,
        documents: documents::lister_par_conducteur(&etat.pool, id).await?,
    })
}

async fn nouveau(
    admin: Administrateur,
    Query(mut saisie): Query<SaisieConducteur>,
) -> Resultat<Html<String>> {
    if saisie.statut.is_empty() {
        saisie.statut = StatutConducteur::Actif.code().to_string();
    }
    rendre(PageFormulaire::nouveau(
        admin.contexte("conducteurs"),
        saisie,
        None,
    ))
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieConducteur>,
) -> Resultat<Response> {
    match conducteurs::creer(&etat.pool, &saisie).await {
        Ok(conducteur) => {
            Ok(Redirect::to(&format!("/conducteurs/{}", conducteur.id)).into_response())
        }
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::nouveau(
            admin.contexte("conducteurs"),
            saisie,
            Some(message),
        ))?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

async fn editer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Html<String>> {
    let conducteur = conducteurs::obtenir(&etat.pool, id).await?;
    rendre(PageFormulaire::modification(
        admin.contexte("conducteurs"),
        id,
        SaisieConducteur::depuis(&conducteur),
        None,
    ))
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
    Form(saisie): Form<SaisieConducteur>,
) -> Resultat<Response> {
    match conducteurs::modifier(&etat.pool, id, &saisie).await {
        Ok(_) => Ok(Redirect::to(&format!("/conducteurs/{id}")).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::modification(
            admin.contexte("conducteurs"),
            id,
            saisie,
            Some(message),
        ))?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Path(id): Path<i64>,
) -> Resultat<Redirect> {
    conducteurs::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(Redirect::to("/conducteurs"))
}
