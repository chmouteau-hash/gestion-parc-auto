//! Vue alertes : échéances dépassées ou proches.

use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;

use crate::commun::format::nombre_fr;
use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::metier::alertes::{self, Alerte, Niveau};
use crate::metier::releves::{self, ARelever};
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
    /// « 1 000 km » : l'horizon des opérations du plan d'entretien.
    horizon_km: String,
    nb_depassees: usize,
    nb_a_venir: usize,
    /// Les compteurs à relever (#94, bloc 4) : à part des alertes, ils ne comptent ni dans les
    /// dépassées ni dans les à venir.
    a_relever: Vec<ARelever>,
}

async fn page(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let aujourd_hui = crate::commun::dates::aujourd_hui();
    let horizon = etat.config.horizon();
    let alertes = alertes::calculer(&etat.pool, aujourd_hui, horizon).await?;
    let a_relever = releves::a_relever(&etat.pool, aujourd_hui).await?;
    let nb_depassees = alertes
        .iter()
        .filter(|a| a.niveau == Niveau::Depassee)
        .count();
    rendre(PageAlertes {
        ctx: utilisateur.contexte("alertes"),
        nb_a_venir: alertes.len() - nb_depassees,
        nb_depassees,
        alertes,
        horizon_jours: horizon.jours,
        horizon_km: format!("{} km", nombre_fr(horizon.km)),
        a_relever,
    })
}
