//! Vue alertes : échéances dépassées ou proches.

use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;

use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::metier::alertes::{self, Alerte, Niveau};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new().route("/alertes", get(page))
}

#[derive(Template)]
#[template(path = "alertes.html")]
struct PageAlertes {
    ctx: Contexte,
    alertes: Vec<Alerte>,
    horizon_jours: i64,
    nb_depassees: usize,
    nb_a_venir: usize,
}

async fn page(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    let horizon_jours = etat.config.alertes_horizon_jours;
    let alertes = alertes::calculer(&etat.pool, aujourd_hui, horizon_jours).await?;
    let nb_depassees = alertes
        .iter()
        .filter(|a| a.niveau == Niveau::Depassee)
        .count();
    rendre(PageAlertes {
        ctx: utilisateur.contexte("alertes"),
        nb_a_venir: alertes.len() - nb_depassees,
        nb_depassees,
        alertes,
        horizon_jours,
    })
}
