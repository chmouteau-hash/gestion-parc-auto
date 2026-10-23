//! API JSON sous `/api/v1` : les mêmes opérations que les pages, pour les scripts.
//!
//! Même session (cookie) et mêmes règles que les pages : les fonctions appellent la couche
//! métier à l'identique. Seule la forme de la réponse change (JSON, codes HTTP). Les saisies
//! sont les mêmes structures que les formulaires : tous les champs sont des chaînes, y compris
//! les nombres et les dates (`"2026-10-11"`), afin qu'il n'y ait qu'une validation.
//!
//! Les documents (fichiers) ne sont pas exposés ici en v1.

use axum::extract::{FromRequestParts, Query, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::donnees::affectations::Affectation;
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::depenses::Depense;
use crate::donnees::echeances::Echeance;
use crate::donnees::interventions::Intervention;
use crate::donnees::plan_entretien::LignePlan;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::ErreurApp;
use crate::etat::Etat;
use crate::metier::affectations::{self, SaisieAffectation};
use crate::metier::alertes::{self, Alerte};
use crate::metier::conducteurs::{self, SaisieConducteur};
use crate::metier::consommation::Resume;
use crate::metier::depenses::{self, SaisieDepense, SaisieFiltre};
use crate::metier::echeances::{self, SaisieEcheance};
use crate::metier::interventions::{self, SaisieIntervention};
use crate::metier::plan_entretien::{self, SaisiePlan, Suivi};
use crate::metier::vehicules::{self, SaisieVehicule};
use crate::web::chemin::Identifiant;
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/alertes", get(alertes_lister))
        .route("/vehicules", get(vehicules_lister).post(vehicules_creer))
        .route(
            "/vehicules/{id}",
            get(vehicules_obtenir)
                .put(vehicules_modifier)
                .delete(vehicules_supprimer),
        )
        .route(
            "/conducteurs",
            get(conducteurs_lister).post(conducteurs_creer),
        )
        .route(
            "/conducteurs/{id}",
            get(conducteurs_obtenir)
                .put(conducteurs_modifier)
                .delete(conducteurs_supprimer),
        )
        .route(
            "/affectations",
            get(affectations_lister).post(affectations_creer),
        )
        .route(
            "/affectations/{id}",
            get(affectations_obtenir)
                .put(affectations_modifier)
                .delete(affectations_supprimer),
        )
        .route(
            "/interventions",
            get(interventions_lister).post(interventions_creer),
        )
        .route(
            "/interventions/{id}",
            get(interventions_obtenir)
                .put(interventions_modifier)
                .delete(interventions_supprimer),
        )
        .route("/echeances", get(echeances_lister).post(echeances_creer))
        .route(
            "/echeances/{id}",
            get(echeances_obtenir)
                .put(echeances_modifier)
                .delete(echeances_supprimer),
        )
        .route("/vehicules/{id}/plan-entretien", get(plan_lister))
        .route("/plan-entretien", post(plan_creer))
        .route(
            "/plan-entretien/{id}",
            get(plan_obtenir).put(plan_modifier).delete(plan_supprimer),
        )
        .route("/depenses", get(depenses_lister).post(depenses_creer))
        .route("/vehicules/{id}/depenses/resume", get(depenses_resume))
        .route(
            "/depenses/{id}",
            get(depenses_obtenir)
                .put(depenses_modifier)
                .delete(depenses_supprimer),
        )
}

/// Erreur rendue en JSON (`{"erreur": "..."}`) avec le code HTTP adapté, au lieu d'une page.
pub struct ErreurApi(pub ErreurApp);

impl From<ErreurApp> for ErreurApi {
    fn from(erreur: ErreurApp) -> ErreurApi {
        ErreurApi(erreur)
    }
}

impl IntoResponse for ErreurApi {
    fn into_response(self) -> Response {
        let (statut, message) = match self.0 {
            ErreurApp::Metier(message) => (StatusCode::BAD_REQUEST, message),
            ErreurApp::Introuvable => (StatusCode::NOT_FOUND, "Ressource introuvable".to_string()),
            ErreurApp::NonConnecte => (StatusCode::UNAUTHORIZED, "Connexion requise".to_string()),
            ErreurApp::Interdit => (
                StatusCode::FORBIDDEN,
                "Action réservée aux administrateurs".to_string(),
            ),
            ErreurApp::Technique(erreur) => {
                tracing::error!(erreur = ?erreur, "erreur technique (API)");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erreur interne".to_string(),
                )
            }
        };
        (statut, Json(json!({ "erreur": message }))).into_response()
    }
}

type ResultatApi<T> = Result<T, ErreurApi>;

/// Les mêmes extracteurs que les pages, mais dont le refus est en JSON.
pub struct Lecteur(pub UtilisateurConnecte);

impl FromRequestParts<Etat> for Lecteur {
    type Rejection = ErreurApi;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApi> {
        UtilisateurConnecte::from_request_parts(parts, etat)
            .await
            .map(Lecteur)
            .map_err(ErreurApi)
    }
}

pub struct Admin(pub Administrateur);

impl FromRequestParts<Etat> for Admin {
    type Rejection = ErreurApi;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApi> {
        Administrateur::from_request_parts(parts, etat)
            .await
            .map(Admin)
            .map_err(ErreurApi)
    }
}

/// L'identifiant de l'adresse. Une valeur illisible (`/vehicules/abc`) donne un 404 en JSON.
pub struct Id(pub i64);

impl FromRequestParts<Etat> for Id {
    type Rejection = ErreurApi;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApi> {
        Identifiant::from_request_parts(parts, etat)
            .await
            .map(|Identifiant(id)| Id(id))
            .map_err(ErreurApi)
    }
}

fn cree<T: serde::Serialize>(valeur: T) -> Response {
    (StatusCode::CREATED, Json(valeur)).into_response()
}

// --- Alertes ---------------------------------------------------------------------------------

async fn alertes_lister(_l: Lecteur, State(etat): State<Etat>) -> ResultatApi<Json<Vec<Alerte>>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        alertes::calculer(&etat.pool, aujourd_hui, etat.config.alertes_horizon_jours).await?,
    ))
}

// --- Véhicules -------------------------------------------------------------------------------

async fn vehicules_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
) -> ResultatApi<Json<Vec<Vehicule>>> {
    Ok(Json(vehicules::lister(&etat.pool).await?))
}

async fn vehicules_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Vehicule>> {
    Ok(Json(vehicules::obtenir(&etat.pool, id).await?))
}

async fn vehicules_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieVehicule>,
) -> ResultatApi<Response> {
    Ok(cree(vehicules::creer(&etat.pool, &saisie).await?))
}

async fn vehicules_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieVehicule>,
) -> ResultatApi<Json<Vehicule>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        vehicules::modifier(&etat.pool, id, &saisie, aujourd_hui).await?,
    ))
}

async fn vehicules_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    vehicules::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Conducteurs -----------------------------------------------------------------------------

async fn conducteurs_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
) -> ResultatApi<Json<Vec<Conducteur>>> {
    Ok(Json(conducteurs::lister(&etat.pool).await?))
}

async fn conducteurs_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Conducteur>> {
    Ok(Json(conducteurs::obtenir(&etat.pool, id).await?))
}

async fn conducteurs_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieConducteur>,
) -> ResultatApi<Response> {
    Ok(cree(conducteurs::creer(&etat.pool, &saisie).await?))
}

async fn conducteurs_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieConducteur>,
) -> ResultatApi<Json<Conducteur>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        conducteurs::modifier(&etat.pool, id, &saisie, aujourd_hui).await?,
    ))
}

async fn conducteurs_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    conducteurs::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Affectations ----------------------------------------------------------------------------

async fn affectations_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
) -> ResultatApi<Json<Vec<Affectation>>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(affectations::lister(&etat.pool, aujourd_hui).await?))
}

async fn affectations_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Affectation>> {
    Ok(Json(affectations::obtenir(&etat.pool, id).await?))
}

async fn affectations_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieAffectation>,
) -> ResultatApi<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(cree(
        affectations::creer(&etat.pool, &saisie, aujourd_hui).await?,
    ))
}

async fn affectations_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieAffectation>,
) -> ResultatApi<Json<Affectation>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        affectations::modifier(&etat.pool, id, &saisie, aujourd_hui).await?,
    ))
}

async fn affectations_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    affectations::supprimer(&etat.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Interventions ---------------------------------------------------------------------------

async fn interventions_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
) -> ResultatApi<Json<Vec<Intervention>>> {
    Ok(Json(interventions::lister(&etat.pool).await?))
}

async fn interventions_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Intervention>> {
    Ok(Json(interventions::obtenir(&etat.pool, id).await?))
}

async fn interventions_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieIntervention>,
) -> ResultatApi<Response> {
    Ok(cree(interventions::creer(&etat.pool, &saisie).await?))
}

async fn interventions_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieIntervention>,
) -> ResultatApi<Json<Intervention>> {
    Ok(Json(
        interventions::modifier(&etat.pool, id, &saisie).await?,
    ))
}

async fn interventions_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    interventions::supprimer(&etat.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Échéances -------------------------------------------------------------------------------

async fn echeances_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
) -> ResultatApi<Json<Vec<Echeance>>> {
    Ok(Json(echeances::lister(&etat.pool).await?))
}

async fn echeances_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Echeance>> {
    Ok(Json(echeances::obtenir(&etat.pool, id).await?))
}

async fn echeances_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieEcheance>,
) -> ResultatApi<Response> {
    Ok(cree(echeances::creer(&etat.pool, &saisie).await?))
}

async fn echeances_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieEcheance>,
) -> ResultatApi<Json<Echeance>> {
    Ok(Json(echeances::modifier(&etat.pool, id, &saisie).await?))
}

async fn echeances_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    echeances::supprimer(&etat.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Plan d'entretien ------------------------------------------------------------------------

/// Le plan d'entretien du véhicule (#94, bloc 2) : chaque opération suivie avec sa dernière fois,
/// sa prochaine borne et son état, calculés à la date du jour.
async fn plan_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Vec<Suivi>>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        plan_entretien::plan_du_vehicule(&etat.pool, id, aujourd_hui).await?,
    ))
}

async fn plan_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<LignePlan>> {
    Ok(Json(plan_entretien::obtenir(&etat.pool, id).await?))
}

/// Ajoute une opération au plan. `derniere_date` et `derniere_kilometrage`, facultatifs, notent
/// la dernière fois comme une intervention du véhicule.
async fn plan_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisiePlan>,
) -> ResultatApi<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    let (ligne, _) = plan_entretien::creer(&etat.pool, &saisie, aujourd_hui).await?;
    Ok(cree(ligne))
}

async fn plan_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisiePlan>,
) -> ResultatApi<Json<LignePlan>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        plan_entretien::modifier(&etat.pool, id, &saisie, aujourd_hui).await?,
    ))
}

async fn plan_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    plan_entretien::supprimer(&etat.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Dépenses --------------------------------------------------------------------------------

/// Les dépenses, les plus récentes en premier ; `?vehicule_id=…&du=…&au=…` les filtre, comme la
/// liste de l'écran. Chaque plein complet mesuré porte sa `consommation`.
async fn depenses_lister(
    _l: Lecteur,
    State(etat): State<Etat>,
    Query(saisie): Query<SaisieFiltre>,
) -> ResultatApi<Json<Vec<Depense>>> {
    let filtre = saisie.valider()?;
    Ok(Json(depenses::lister_filtre(&etat.pool, &filtre).await?))
}

/// Ce que le bloc « Coût » de la fiche d'un véhicule résume : l'entretien (dont les pièces), le
/// carburant et les péages de l'année et des douze derniers mois, la consommation moyenne, le coût
/// au kilomètre (entretien compris).
async fn depenses_resume(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Resume>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(Json(
        depenses::resume_du_vehicule(&etat.pool, id, aujourd_hui).await?,
    ))
}

async fn depenses_obtenir(
    _l: Lecteur,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<Json<Depense>> {
    Ok(Json(depenses::obtenir(&etat.pool, id).await?))
}

async fn depenses_creer(
    _a: Admin,
    State(etat): State<Etat>,
    Json(saisie): Json<SaisieDepense>,
) -> ResultatApi<Response> {
    Ok(cree(depenses::creer(&etat.pool, &saisie).await?))
}

async fn depenses_modifier(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
    Json(saisie): Json<SaisieDepense>,
) -> ResultatApi<Json<Depense>> {
    Ok(Json(depenses::modifier(&etat.pool, id, &saisie).await?))
}

async fn depenses_supprimer(
    _a: Admin,
    State(etat): State<Etat>,
    Id(id): Id,
) -> ResultatApi<StatusCode> {
    depenses::supprimer(&etat.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
