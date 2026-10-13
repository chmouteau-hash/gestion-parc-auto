//! Pages des conducteurs : liste, fiche, création, modification, suppression.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Choix, StatutConducteur};
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::documents::Document;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::conducteurs::{self, SaisieConducteur};
use crate::metier::{affectations, documents};
use crate::web::chemin::Identifiant;
use crate::web::documents::FormulaireDepot;
use crate::web::rendu::{rendre, Contexte, LigneAffectation, LigneConducteur};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/conducteurs", get(liste).post(creer))
        .route("/conducteurs/nouveau", get(nouveau))
        .route("/conducteurs/{id}", get(fiche).post(modifier))
        .route("/conducteurs/{id}/modifier", get(editer))
        .route("/conducteurs/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "conducteurs/liste.html")]
struct PageListe {
    ctx: Contexte,
    conducteurs: Vec<LigneConducteur>,
}

#[derive(Template)]
#[template(path = "conducteurs/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieConducteur,
    erreur: Option<String>,
    statuts: Vec<Choix>,
}

impl PageFormulaire {
    fn nouveau(ctx: Contexte, saisie: SaisieConducteur, erreur: Option<String>) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: "Nouveau conducteur".to_string(),
            action: "/conducteurs".to_string(),
            saisie,
            erreur,
            statuts: StatutConducteur::choix(),
        }
    }

    fn modification(
        ctx: Contexte,
        id: i64,
        saisie: SaisieConducteur,
        erreur: Option<String>,
    ) -> PageFormulaire {
        PageFormulaire {
            ctx,
            titre: format!("Modifier {} {}", saisie.prenom, saisie.nom),
            action: format!("/conducteurs/{id}"),
            saisie,
            erreur,
            statuts: StatutConducteur::choix(),
        }
    }
}

#[derive(Template)]
#[template(path = "conducteurs/fiche.html")]
struct PageFiche {
    ctx: Contexte,
    conducteur: Conducteur,
    /// Faux pour un conducteur inactif ou au permis expiré : le bouton « Affecter » disparaît.
    affectable: bool,
    /// Vrai le lendemain de la date d'expiration : la fiche l'écrit à côté de la date.
    permis_expire: bool,
    /// Vrai si le permis expire dans l'horizon des alertes sans l'être encore : la fiche le dit.
    permis_a_venir: bool,
    affectations: Vec<LigneAffectation>,
    documents: Vec<Document>,
    depot: FormulaireDepot,
    /// Motif du refus quand un dépôt de document vient d'échouer.
    erreur_depot: Option<String>,
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    rendre(PageListe {
        ctx: utilisateur.contexte("conducteurs"),
        conducteurs: LigneConducteur::tous(
            conducteurs::lister(&etat.pool).await?,
            aujourd_hui,
            etat.config.alertes_horizon_jours,
        ),
    })
}

async fn fiche(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    page_fiche(&etat, utilisateur.contexte("conducteurs"), id, None).await
}

/// Construit la fiche. `erreur_depot` porte le motif d'un dépôt de document refusé :
/// `web::documents` ré-affiche alors la fiche d'où venait le fichier, avec ce motif en tête.
pub(super) async fn page_fiche(
    etat: &Etat,
    ctx: Contexte,
    id: i64,
    erreur_depot: Option<String>,
) -> Resultat<Html<String>> {
    let conducteur = conducteurs::obtenir(&etat.pool, id).await?;
    let aujourd_hui = chrono::Local::now().date_naive();
    rendre(PageFiche {
        ctx,
        affectable: affectations::verifier_conducteur_affectable(&conducteur, aujourd_hui).is_ok(),
        permis_expire: conducteur.permis_expire(aujourd_hui),
        permis_a_venir: crate::metier::alertes::permis_a_venir(
            conducteur.date_expiration_permis,
            aujourd_hui,
            etat.config.alertes_horizon_jours,
        ),
        conducteur,
        affectations: LigneAffectation::toutes(
            affectations::lister_par_conducteur(&etat.pool, id).await?,
            aujourd_hui,
        ),
        documents: documents::lister_par_conducteur(&etat.pool, id).await?,
        depot: FormulaireDepot::pour_conducteur(id),
        erreur_depot,
    })
}

async fn nouveau(
    admin: Administrateur,
    Query(mut saisie): Query<SaisieConducteur>,
) -> Resultat<Html<String>> {
    if saisie.statut.is_empty() {
        saisie.statut = StatutConducteur::Actif.code().to_string();
    }
    rendre(PageFormulaire::nouveau(
        admin.contexte("conducteurs"),
        saisie,
        None,
    ))
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieConducteur>,
) -> Resultat<Response> {
    match conducteurs::creer(&etat.pool, &saisie).await {
        Ok(conducteur) => {
            Ok(Redirect::to(&format!("/conducteurs/{}", conducteur.id)).into_response())
        }
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::nouveau(
            admin.contexte("conducteurs"),
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
    let conducteur = conducteurs::obtenir(&etat.pool, id).await?;
    rendre(PageFormulaire::modification(
        admin.contexte("conducteurs"),
        id,
        SaisieConducteur::depuis(&conducteur),
        None,
    ))
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieConducteur>,
) -> Resultat<Response> {
    let aujourd_hui = chrono::Local::now().date_naive();
    match conducteurs::modifier(&etat.pool, id, &saisie, aujourd_hui).await {
        Ok(_) => Ok(Redirect::to(&format!("/conducteurs/{id}")).into_response()),
        Err(ErreurApp::Metier(message)) => Ok(rendre(PageFormulaire::modification(
            admin.contexte("conducteurs"),
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
    conducteurs::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok(Redirect::to("/conducteurs"))
}
