//! Les pages de navigation : « Plus » (le reste du menu d'Autoclair, derrière le quatrième
//! onglet) et la recherche (le champ de la barre latérale d'Autoclair Pro, aussi sur « Plus »).

use askama::Template;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use crate::donnees::conducteurs::Conducteur;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::metier::recherche;
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/plus", get(plus))
        .route("/recherche", get(rechercher))
}

#[derive(Template)]
#[template(path = "plus.html")]
struct PagePlus {
    ctx: Contexte,
}

async fn plus(utilisateur: UtilisateurConnecte) -> Resultat<Html<String>> {
    rendre(PagePlus {
        ctx: utilisateur.contexte("plus"),
    })
}

#[derive(Debug, Default, Deserialize)]
struct Question {
    #[serde(default)]
    q: String,
}

#[derive(Template)]
#[template(path = "recherche.html")]
struct PageRecherche {
    ctx: Contexte,
    question: String,
    vehicules: Vec<Vehicule>,
    conducteurs: Vec<Conducteur>,
}

async fn rechercher(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Query(question): Query<Question>,
) -> Resultat<Html<String>> {
    let resultats = recherche::chercher(&etat.pool, &question.q).await?;
    rendre(PageRecherche {
        ctx: utilisateur.contexte("recherche"),
        question: resultats.question,
        vehicules: resultats.vehicules,
        conducteurs: resultats.conducteurs,
    })
}
