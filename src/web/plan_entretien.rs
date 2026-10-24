//! Pages du plan d'entretien d'un véhicule (#94, bloc 2) : ajouter une opération suivie, la
//! modifier, ne plus la suivre ; partir d'un plan proposé selon le carburant (bloc 5). Le plan
//! lui-même se lit sur la fiche du véhicule.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Carburant, Choix};
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
        .route("/plan-entretien/depart", get(depart).post(creer_depart))
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

/// Une opération de la page « Plan de départ » : la proposition, ou ce qu'on en a fait avant un
/// refus.
struct LigneDepart {
    code: &'static str,
    libelle: &'static str,
    cochee: bool,
    km: String,
    mois: String,
    /// Pourquoi elle n'est pas cochée d'office.
    note: Option<&'static str>,
}

#[derive(Template)]
#[template(path = "plan_entretien/depart.html")]
struct PageDepart {
    ctx: Contexte,
    titre: String,
    vehicule: Vehicule,
    /// « Essence », « Diesel »… ; vide si le véhicule n'en a pas de connu.
    carburant: &'static str,
    lignes: Vec<LigneDepart>,
    erreur: Option<String>,
}

/// La page part des propositions pour le carburant du véhicule (#94, bloc 5), sans les
/// opérations qu'il suit déjà. Après un refus (`envoi`), elle garde ce qui a été coché et saisi.
async fn page_depart(
    etat: &Etat,
    ctx: Contexte,
    vehicule: Vehicule,
    envoi: Option<&[(String, String)]>,
    erreur: Option<String>,
) -> Resultat<PageDepart> {
    let deja_suivies = plan_entretien::lister_par_vehicule(&etat.pool, vehicule.id).await?;
    let carburant = Carburant::depuis_code(&vehicule.carburant);
    let champ = |nom: String| {
        envoi.and_then(|paires| {
            paires
                .iter()
                .find(|(n, _)| *n == nom)
                .map(|(_, v)| v.clone())
        })
    };
    let lignes = plan_entretien::plan_de_depart(carburant)
        .into_iter()
        .filter(|p| {
            !deja_suivies
                .iter()
                .any(|l| l.operation == p.operation.code())
        })
        .map(|p| {
            let code = p.operation.code();
            LigneDepart {
                code,
                libelle: p.operation.libelle(),
                cochee: match envoi {
                    Some(paires) => paires.iter().any(|(n, v)| n == "operation" && v == code),
                    None => p.cochee,
                },
                km: champ(format!("km_{code}"))
                    .unwrap_or_else(|| p.periodicite_km.map(|k| k.to_string()).unwrap_or_default()),
                mois: champ(format!("mois_{code}")).unwrap_or_else(|| {
                    p.periodicite_mois
                        .map(|m| m.to_string())
                        .unwrap_or_default()
                }),
                note: p.note,
            }
        })
        .collect();
    Ok(PageDepart {
        ctx,
        titre: format!("Plan de départ · {}", vehicule.immatriculation),
        carburant: carburant.map(Carburant::libelle).unwrap_or(""),
        vehicule,
        lignes,
        erreur,
    })
}

/// `?vehicule_id=…` : « Proposer un plan », depuis la fiche d'un véhicule sans plan.
async fn depart(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(saisie): Query<SaisiePlan>,
) -> Resultat<Html<String>> {
    let vehicule = vehicule_de(&etat, &saisie).await?;
    rendre(page_depart(&etat, admin.contexte("vehicules"), vehicule, None, None).await?)
}

/// Les cases cochées arrivent en paires `operation=…` répétées, chacune avec ses `km_…` et
/// `mois_…` : le serveur ne lit que les périodicités des cases cochées.
async fn creer_depart(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(paires): Form<Vec<(String, String)>>,
) -> Resultat<Response> {
    let champ = |nom: &str| {
        paires
            .iter()
            .find(|(n, _)| n == nom)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let vehicule = vehicule_de(
        &etat,
        &SaisiePlan {
            vehicule_id: champ("vehicule_id"),
            ..Default::default()
        },
    )
    .await?;
    let saisies: Vec<SaisiePlan> = paires
        .iter()
        .filter(|(n, _)| n == "operation")
        .map(|(_, code)| SaisiePlan {
            vehicule_id: vehicule.id.to_string(),
            operation: code.clone(),
            periodicite_km: champ(&format!("km_{code}")),
            periodicite_mois: champ(&format!("mois_{code}")),
            ..Default::default()
        })
        .collect();
    let aujourd_hui = crate::commun::dates::aujourd_hui();
    match plan_entretien::creer_plan_de_depart(&etat.pool, &saisies, aujourd_hui).await {
        Ok(_) => Ok(fait::rediriger(
            &format!("/vehicules/{}", vehicule.id),
            Fait::PlanDeDepart,
        )),
        Err(ErreurApp::Metier(message)) => {
            let page = page_depart(
                &etat,
                admin.contexte("vehicules"),
                vehicule,
                Some(&paires),
                Some(message),
            )
            .await?;
            Ok(rendre(page)?.into_response())
        }
        Err(autre) => Err(autre),
    }
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
    let aujourd_hui = crate::commun::dates::aujourd_hui();
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
    let aujourd_hui = crate::commun::dates::aujourd_hui();
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
