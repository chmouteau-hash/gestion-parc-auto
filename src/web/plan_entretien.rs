//! Pages du plan d'entretien d'un véhicule (#94, bloc 2) : ajouter une opération suivie, la
//! modifier, ne plus la suivre. Le plan lui-même se lit sur la fiche du véhicule.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::Choix;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::plan_entretien::{self, SaisiePlan};
use crate::metier::validation::identifiant;
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::Administrateur;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/plan-entretien", post(creer))
        .route("/plan-entretien/nouveau", get(nouveau))
        .route("/plan-entretien/{id}", post(modifier))
        .route("/plan-entretien/{id}/modifier", get(editer))
        .route("/plan-entretien/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "plan_entretien/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisiePlan,
    erreur: Option<String>,
    vehicule: Vehicule,
    /// Les opérations proposées, en deux groupes, sans celles déjà suivies.
    groupes_operations: [(&'static str, Vec<Choix>); 2],
    /// La création demande aussi la dernière fois ; la modification, non : elle se change dans
    /// l'entretien.
    creation: bool,
}

/// Construit le formulaire, pour la création (`id` absent) ou la modification d'une ligne.
async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    vehicule: Vehicule,
    saisie: SaisiePlan,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let deja_suivies = plan_entretien::lister_par_vehicule(&etat.pool, vehicule.id).await?;
    let courante = id.map(|_| saisie.operation.clone());
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => format!("Suivre une opération · {}", vehicule.immatriculation),
            Some(_) => format!(
                "Modifier le plan d'entretien · {}",
                vehicule.immatriculation
            ),
        },
        action: match id {
            None => "/plan-entretien".to_string(),
            Some(id) => format!("/plan-entretien/{id}"),
        },
        groupes_operations: plan_entretien::operations_proposees(
            &deja_suivies,
            courante.as_deref(),
        ),
        creation: id.is_none(),
        saisie,
        erreur,
        vehicule,
    })
}

/// Le véhicule de la saisie : celui de l'adresse ou du formulaire. Introuvable s'il n'existe pas.
async fn vehicule_de(etat: &Etat, saisie: &SaisiePlan) -> Resultat<Vehicule> {
    let id = identifiant(&saisie.vehicule_id, "véhicule").map_err(|_| ErreurApp::Introuvable)?;
    vehicules::obtenir(&etat.pool, id).await
}

/// `?vehicule_id=…` : le formulaire part toujours de la fiche d'un véhicule.
async fn nouveau(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(saisie): Query<SaisiePlan>,
) -> Resultat<Html<String>> {
    let vehicule = vehicule_de(&etat, &saisie).await?;
    let saisie = SaisiePlan {
        vehicule_id: vehicule.id.to_string(),
        ..saisie
    };
    rendre(
        formulaire(
            &etat,
            admin.contexte("vehicules"),
            None,
            vehicule,
            saisie,
            None,
        )
        .await?,
    )
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisiePlan>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    match plan_entretien::creer(&etat.pool, &saisie, aujourd_hui).await {
        Ok((ligne, intervention)) => Ok(fait::rediriger(
            &format!("/vehicules/{}", ligne.vehicule_id),
            if intervention.is_some() {
                Fait::OperationSuivieAvecDerniereFois
            } else {
                Fait::OperationSuivie
            },
        )),
        Err(ErreurApp::Metier(message)) => {
            let vehicule = vehicule_de(&etat, &saisie).await?;
            let page = formulaire(
                &etat,
                admin.contexte("vehicules"),
                None,
                vehicule,
                saisie,
                Some(message),
            )
            .await?;
            Ok(rendre(page)?.into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn editer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let ligne = plan_entretien::obtenir(&etat.pool, id).await?;
    let vehicule = vehicules::obtenir(&etat.pool, ligne.vehicule_id).await?;
    let saisie = SaisiePlan::depuis(&ligne);
    rendre(
        formulaire(
            &etat,
            admin.contexte("vehicules"),
            Some(id),
            vehicule,
            saisie,
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisiePlan>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    match plan_entretien::modifier(&etat.pool, id, &saisie, aujourd_hui).await {
        Ok(ligne) => Ok(fait::rediriger(
            &format!("/vehicules/{}", ligne.vehicule_id),
            Fait::PlanModifie,
        )),
        Err(ErreurApp::Metier(message)) => {
            let ligne = plan_entretien::obtenir(&etat.pool, id).await?;
            let vehicule = vehicules::obtenir(&etat.pool, ligne.vehicule_id).await?;
            let saisie = SaisiePlan {
                vehicule_id: vehicule.id.to_string(),
                ..saisie
            };
            let page = formulaire(
                &etat,
                admin.contexte("vehicules"),
                Some(id),
                vehicule,
                saisie,
                Some(message),
            )
            .await?;
            Ok(rendre(page)?.into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let ligne = plan_entretien::obtenir(&etat.pool, id).await?;
    plan_entretien::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger(
        &format!("/vehicules/{}", ligne.vehicule_id),
        Fait::OperationPlusSuivie,
    ))
}
