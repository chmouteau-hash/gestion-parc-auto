//! La racine du site : l'accueil du profil. Une installation particulier reçoit ses véhicules
//! en cartes ; une entreprise, son tableau de bord.

use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;

use crate::commun::format::euros_ronds_fr;
use crate::commun::types::Profil;
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::metier::accueil::{self, Chiffres, ResumeVehicule};
use crate::metier::alertes::Alerte;
use crate::metier::consommation::mois_en_cours;
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::UtilisateurConnecte;

pub fn routes() -> Router<Etat> {
    Router::new().route("/", get(page))
}

#[derive(Template)]
#[template(path = "accueil/particulier.html")]
struct PageParticulier {
    ctx: Contexte,
    resumes: Vec<ResumeVehicule>,
    alertes: Vec<Alerte>,
    nb_reformes: usize,
}

#[derive(Template)]
#[template(path = "accueil/entreprise.html")]
struct PageEntreprise {
    ctx: Contexte,
    chiffres: Chiffres,
    alertes: Vec<Alerte>,
    sans_conducteur: Vec<Vehicule>,
    interventions: Vec<Intervention>,
    horizon_jours: i64,
    /// « 1 279 € » : les dépenses du mois, en euros ronds (le chiffre doit tenir dans sa case).
    depenses_du_mois: String,
    /// Le premier et le dernier jour du mois (`AAAA-MM-JJ`) : le lien du chiffre ouvre la liste
    /// des dépenses de ce mois.
    mois_du: String,
    mois_au: String,
}

async fn page(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let ctx = utilisateur.contexte("accueil");
    let aujourd_hui = chrono::Local::now().date_naive();
    let horizon_jours = etat.config.alertes_horizon_jours;
    match ctx.profil {
        Profil::Particulier => {
            let accueil = accueil::particulier(&etat.pool, aujourd_hui, horizon_jours).await?;
            rendre(PageParticulier {
                ctx,
                resumes: accueil.resumes,
                alertes: accueil.alertes,
                nb_reformes: accueil.nb_reformes,
            })
        }
        Profil::Entreprise => {
            let accueil = accueil::entreprise(&etat.pool, aujourd_hui, horizon_jours).await?;
            let (mois_du, mois_au) = mois_en_cours(aujourd_hui);
            rendre(PageEntreprise {
                ctx,
                chiffres: accueil.chiffres,
                alertes: accueil.alertes,
                sans_conducteur: accueil.sans_conducteur,
                interventions: accueil.interventions,
                horizon_jours,
                depenses_du_mois: euros_ronds_fr(accueil.depenses_du_mois_centimes),
                mois_du: mois_du.to_string(),
                mois_au: mois_au.to_string(),
            })
        }
    }
}
