//! Pages des véhicules : liste, fiche, création, modification, suppression.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

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
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte, LigneAffectation, LigneEcheance};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/vehicules", get(liste).post(creer))
        .route("/vehicules/nouveau", get(nouveau))
        .route("/vehicules/{id}", get(fiche).post(modifier))
        .route("/vehicules/{id}/modifier", get(editer))
        .route("/vehicules/{id}/kilometrage", post(relever))
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
    /// La photo du véhicule la plus récente, pour la vignette en tête de fiche.
    photo: Option<Document>,
    depot: FormulaireDepot,
    /// Motif du refus quand un dépôt de document vient d'échouer.
    erreur_depot: Option<String>,
    /// Motif du refus d'une autre action faite depuis la fiche (relevé du kilométrage).
    erreur: Option<String>,
}

/// Le relevé du kilométrage, depuis la carte d'accueil (`?retour=accueil`) ou la fiche.
#[derive(Debug, Default, Deserialize)]
struct Releve {
    #[serde(default)]
    kilometrage: String,
}

#[derive(Debug, Default, Deserialize)]
struct OrigineReleve {
    #[serde(default)]
    retour: String,
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
    page_fiche(&etat, utilisateur.contexte("vehicules"), id, None, None).await
}

/// Construit la fiche. `erreur_depot` porte le motif d'un dépôt de document refusé :
/// `web::documents` ré-affiche alors la fiche d'où venait le fichier, avec ce motif en tête.
/// `erreur` porte celui d'un relevé de kilométrage refusé.
pub(super) async fn page_fiche(
    etat: &Etat,
    ctx: Contexte,
    id: i64,
    erreur_depot: Option<String>,
    erreur: Option<String>,
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
            etat.config.alertes_horizon_jours,
        ),
        documents: documents::lister_par_vehicule(&etat.pool, id).await?,
        photo: documents::derniere_photo(&etat.pool, id).await?,
        depot: FormulaireDepot::pour_vehicule(id),
        vehicule,
        erreur_depot,
        erreur,
    })
}

/// Le relevé en un geste. Accepté : retour à la page d'où il vient, avec le message. Refusé :
/// la fiche, avec le motif.
async fn relever(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Query(origine): Query<OrigineReleve>,
    Form(releve): Form<Releve>,
) -> Resultat<Response> {
    match vehicules::relever_kilometrage(&etat.pool, id, &releve.kilometrage).await {
        Ok(_) => {
            let vers = match origine.retour.as_str() {
                "accueil" => "/".to_string(),
                _ => format!("/vehicules/{id}"),
            };
            Ok(fait::rediriger(&vers, Fait::KilometrageReleve))
        }
        Err(ErreurApp::Metier(message)) => {
            Ok(
                page_fiche(&etat, admin.contexte("vehicules"), id, None, Some(message))
                    .await?
                    .into_response(),
            )
        }
        Err(autre) => Err(autre),
    }
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
        Ok(vehicule) => Ok(fait::rediriger(
            &format!("/vehicules/{}", vehicule.id),
            Fait::VehiculeCree,
        )),
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
        Ok(_) => Ok(fait::rediriger(
            &format!("/vehicules/{id}"),
            Fait::VehiculeModifie,
        )),
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
) -> Resultat<Response> {
    vehicules::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(fait::rediriger("/vehicules", Fait::VehiculeSupprime))
}
