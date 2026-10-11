//! Rendu des templates Askama et contexte commun à toutes les pages.

use askama::Template;
use axum::response::Html;

use crate::erreurs::Resultat;

/// Ce que `templates/base.html` affiche sur chaque page : qui est connecté, et quel onglet du
/// menu est actif.
#[derive(Debug, Clone)]
pub struct Contexte {
    pub login: String,
    pub est_admin: bool,
    pub page: &'static str,
}

/// Rend un template en page HTML. Une erreur de rendu est technique (bogue), pas métier.
pub fn rendre<T: Template>(template: T) -> Resultat<Html<String>> {
    Ok(Html(template.render()?))
}
