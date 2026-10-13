//! Pages des véhicules : liste, fiche, création, modification, suppression.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Carburant, Choix, StatutVehicule};
use crate::donnees::affectations::Affectation;
use crate::donnees::documents::Document;
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::vehicules::{self, SaisieVehicule};
use crate::metier::{affectations, documents, echeances, interventions};
use crate::web::chemin::Identifiant;
use crate::web::documents::FormulaireDepot;
use crate::web::rendu::{rendre, Contexte, LigneAffectation, LigneEcheance};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/vehicules", get(liste).post(creer))
        .route("/vehicules/nouveau", get(nouveau))
        .route("/vehicules/{id}", get(fiche).post(modifier))
        .route("/vehicules/{id}/modifier", get(editer))
        .route("/vehicules/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "vehicules/liste.html")]
struct PageListe {
    ctx: Contexte,
    vehicules: Vec<Vehicule>,
}

#[derive(Template)]
#[template(path = "vehicules/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieVehicule,
    erreur: Option<String>,
    carburants: Vec<Choix>,
    statuts: Vec<Choix>,
}

impl PageFormulaire {
    fn nouveau(ctx: Contexte, saisie: SaisieVehicule, erreur: Option<String>) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: "Nouveau véhicule".to_string(),
            action: "/vehicules".to_string(),
            saisie,
            erreur,
            carburants: Carburant::choix(),
            statuts: StatutVehicule::choix(),
        }
    }

    fn modification(
        ctx: Contexte,
        id: i64,
        saisie: SaisieVehicule,
        erreur: Option<String>,
    ) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: format!("Modifier le véhicule {}", saisie.immatriculation),
            action: format!("/vehicules/{id}"),
            saisie,
            erreur,
            carburants: Carburant::choix(),
            statuts: StatutVehicule::choix(),
        }
    }
}

#[derive(Template)]
#[template(path = "vehicules/fiche.html")]
struct PageFiche {
    ctx: Contexte,
    vehicule: Vehicule,
    /// Affectations dont la période couvre aujourd'hui : un véhicule peut avoir plusieurs
    /// conducteurs à la fois.
    affectations_actives: Vec<Affectation>,
    affectations: Vec<LigneAffectation>,
    interventions: Vec<Intervention>,
    echeances: Vec<LigneEcheance>,
    documents: Vec<Document>,
    depot: FormulaireDepot,
    /// Motif du refus quand un dépôt de document vient d'échouer.
    erreur_depot: Option<String>,
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("vehicules"),
        vehicules: vehicules::lister(&etat.pool).await?,
    })
}

async fn fiche(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    page_fiche(&etat, utilisateur.contexte("vehicules"), id, None).await
}

/// Construit la fiche. `erreur_depot` porte le motif d'un dépôt de document refusé :
/// `web::documents` ré-affiche alors la fiche d'où venait le fichier, avec ce motif en tête.
pub(super) async fn page_fiche(
    etat: &Etat,
    ctx: Contexte,
    id: i64,
    erreur_depot: Option<String>,
) -> Resultat<Html<String>> {
    let vehicule = vehicules::obtenir(&etat.pool, id).await?;
    let affectations = affectations::lister_par_vehicule(&etat.pool, id).await?;
    let aujourd_hui = chrono::Local::now().date_naive();
    rendre(PageFiche {
        ctx,
        affectations_actives: affectations
            .iter()
            .filter(|a| a.est_en_cours(aujourd_hui))
            .cloned()
            .collect(),
        affectations: LigneAffectation::toutes(affectations, aujourd_hui),
        interventions: interventions::lister_par_vehicule(&etat.pool, id).await?,
        echeances: LigneEcheance::toutes(
            echeances::lister_par_vehicule(&etat.pool, id).await?,
            aujourd_hui,
        ),
        documents: documents::lister_par_vehicule(&etat.pool, id).await?,
        depot: FormulaireDepot::pour_vehicule(id),
        vehicule,
        erreur_depot,
    })
}

/// Le formulaire vierge accepte des valeurs dans l'URL (`?statut=en_service`) pour pré-remplir.
async fn nouveau(
    admin: Administrateur,
    Query(mut saisie): Query<SaisieVehicule>,
) -> Resultat<Html<String>> {
    if saisie.statut.is_empty() {
        saisie.statut = StatutVehicule::EnService.code().to_string();
    }
    rendre(PageFormulaire::nouveau(
        admin.contexte("vehicules"),
        saisie,
        None,
    ))
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieVehicule>,
) -> Resultat<Response> {
    match vehicules::creer(&etat.pool, &saisie).await {
        Ok(vehicule) => Ok(Redirect::to(&format!("/vehicules/{}", vehicule.id)).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::nouveau(
            admin.contexte("vehicules"),
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
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let vehicule = vehicules::obtenir(&etat.pool, id).await?;
    rendre(PageFormulaire::modification(
        admin.contexte("vehicules"),
        id,
        SaisieVehicule::depuis(&vehicule),
        None,
    ))
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieVehicule>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    match vehicules::modifier(&etat.pool, id, &saisie, aujourd_hui).await {
        Ok(_) => Ok(Redirect::to(&format!("/vehicules/{id}")).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::modification(
            admin.contexte("vehicules"),
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
    Identifiant(id): Identifiant,
) -> Resultat<Redirect> {
    vehicules::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(Redirect::to("/vehicules"))
}
