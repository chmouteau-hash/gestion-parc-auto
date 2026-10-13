//! Rendu des templates Askama et contexte commun à toutes les pages.

use askama::Template;
use axum::response::Html;
use chrono::NaiveDate;

use crate::commun::types::Profil;
use crate::donnees::affectations::{Affectation, Etat};
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::echeances::Echeance;
use crate::erreurs::Resultat;
use crate::metier::echeances::EtatEcheance;

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

/// Une échéance prête à afficher dans un tableau.
///
/// Son état (dépassée, à venir, à traiter, traitée) dépend lui aussi de la date du jour : c'est lui que la
/// pastille écrit.
pub struct LigneEcheance {
    pub e: Echeance,
    pub etat: EtatEcheance,
}

impl LigneEcheance {
    pub fn toutes(
        echeances: Vec<Echeance>,
        aujourd_hui: NaiveDate,
        horizon_jours: i64,
    ) -> Vec<LigneEcheance> {
        echeances
            .into_iter()
            .map(|e| LigneEcheance {
                etat: crate::metier::echeances::etat(&e, aujourd_hui, horizon_jours),
                e,
            })
            .collect()
    }
}

/// « À venir » à côté d'une date de permis : un avertissement, donc réservé aux conducteurs actifs,
/// comme la page Alertes. « Expiré », lui, est un fait : il s'écrit pour tous.
pub fn permis_a_venir(c: &Conducteur, aujourd_hui: NaiveDate, horizon_jours: i64) -> bool {
    c.est_actif()
        && crate::metier::alertes::permis_a_venir(
            c.date_expiration_permis,
            aujourd_hui,
            horizon_jours,
        )
}

/// Un conducteur prêt à afficher dans la liste : un permis expiré, ou qui expire dans l'horizon des
/// alertes, se dit à côté de sa date.
pub struct LigneConducteur {
    pub c: Conducteur,
    pub permis_expire: bool,
    pub permis_a_venir: bool,
}

impl LigneConducteur {
    pub fn tous(
        conducteurs: Vec<Conducteur>,
        aujourd_hui: NaiveDate,
        horizon_jours: i64,
    ) -> Vec<LigneConducteur> {
        conducteurs
            .into_iter()
            .map(|c| LigneConducteur {
                permis_expire: c.permis_expire(aujourd_hui),
                permis_a_venir: permis_a_venir(&c, aujourd_hui, horizon_jours),
                c,
            })
            .collect()
    }
}

/// Rend un template en page HTML. Une erreur de rendu est technique (bogue), pas métier.
pub fn rendre<T: Template>(template: T) -> Resultat<Html<String>> {
    Ok(Html(template.render()?))
}
