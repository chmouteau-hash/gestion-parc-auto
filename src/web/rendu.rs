//! Rendu des templates Askama et contexte commun à toutes les pages.

use askama::Template;
use axum::response::Html;
use chrono::NaiveDate;

use crate::commun::types::Profil;
use crate::donnees::affectations::{Affectation, Etat};
use crate::erreurs::Resultat;

/// Ce que `templates/base.html` affiche sur chaque page : qui est connecté, et quel onglet du
/// menu est actif. Le profil de l'installation y voyage aussi : les gabarits lui demandent ce
/// qu'ils doivent montrer.
#[derive(Debug, Clone)]
pub struct Contexte {
    pub login: String,
    pub est_admin: bool,
    pub page: &'static str,
    pub profil: Profil,
}

impl Contexte {
    /// Qui conduit quoi, depuis quand : utile à une entreprise, sans objet pour un particulier.
    /// Le profil masque, il n'interdit pas : les routes des affectations répondent toujours.
    pub fn montre_affectations(&self) -> bool {
        self.profil == Profil::Entreprise
    }

    /// Le service auquel un conducteur est rattaché n'existe que dans une entreprise.
    pub fn montre_service(&self) -> bool {
        self.profil == Profil::Entreprise
    }
}

/// Une affectation prête à afficher dans un tableau.
///
/// Son état (en cours, à venir, terminée) dépend de la date du jour : il est calculé ici, une
/// fois, plutôt que dans le template.
pub struct LigneAffectation {
    pub a: Affectation,
    pub etat: Etat,
    /// Vrai si la période couvre aujourd'hui : la ligne est mise en avant, et le bouton
    /// « Terminer aujourd'hui » est proposé.
    pub en_cours: bool,
}

impl LigneAffectation {
    pub fn toutes(affectations: Vec<Affectation>, aujourd_hui: NaiveDate) -> Vec<LigneAffectation> {
        affectations
            .into_iter()
            .map(|a| LigneAffectation {
                etat: a.etat(aujourd_hui),
                en_cours: a.est_en_cours(aujourd_hui),
                a,
            })
            .collect()
    }
}

/// Rend un template en page HTML. Une erreur de rendu est technique (bogue), pas métier.
pub fn rendre<T: Template>(template: T) -> Resultat<Html<String>> {
    Ok(Html(template.render()?))
}
