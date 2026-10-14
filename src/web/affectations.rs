//! Pages des affectations véhicule ↔ conducteur.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use chrono::NaiveDate;

use crate::donnees::conducteurs::Conducteur;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::affectations::{self, SaisieAffectation};
use crate::metier::{conducteurs, vehicules};
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte, LigneAffectation};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/affectations", get(liste).post(creer))
        .route("/affectations/nouvelle", get(nouvelle))
        .route("/affectations/{id}", post(modifier))
        .route("/affectations/{id}/modifier", get(editer))
        .route("/affectations/{id}/cloturer", post(cloturer))
        .route("/affectations/{id}/supprimer", post(supprimer))
}

fn aujourd_hui() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[derive(Template)]
#[template(path = "affectations/liste.html")]
struct PageListe {
    ctx: Contexte,
    affectations: Vec<LigneAffectation>,
}

/// Une ligne d'une liste déroulante du formulaire.
///
/// Un véhicule réformé, un conducteur inactif ou au permis expiré ne peut pas recevoir
/// d'affectation : sa ligne est grisée et dit pourquoi. Exception : la ligne déjà sélectionnée
/// reste active, sinon le navigateur ne l'enverrait pas et une affectation existante ne pourrait
/// plus être corrigée ni clôturée. La règle elle-même est tenue par `metier::affectations`.
///
/// Un conducteur se lit « Prénom Nom, Service » : deux personnes de même nom se distinguent par
/// leur service. Le service n'existe que dans le profil entreprise (`Contexte::montre_service`) ;
/// un conducteur sans service garde son nom seul.
struct OptionListe {
    valeur: String,
    libelle: String,
    selectionnee: bool,
    desactivee: bool,
}

impl OptionListe {
    fn nouvelle(id: i64, nom: String, empechement: Option<&str>, selection: &str) -> OptionListe {
        let valeur = id.to_string();
        let selectionnee = valeur == selection;
        OptionListe {
            libelle: match empechement {
                Some(raison) => format!("{nom} ({raison})"),
                None => nom,
            },
            desactivee: empechement.is_some() && !selectionnee,
            selectionnee,
            valeur,
        }
    }

    fn vehicule(v: &Vehicule, selection: &str) -> OptionListe {
        let empechement = v.est_reforme().then_some("réformé");
        OptionListe::nouvelle(v.id, v.designation(), empechement, selection)
    }

    fn conducteur(
        c: &Conducteur,
        selection: &str,
        aujourd_hui: NaiveDate,
        avec_service: bool,
    ) -> OptionListe {
        let empechement = if !c.est_actif() {
            Some("inactif")
        } else if c.permis_expire(aujourd_hui) {
            Some("permis expiré")
        } else {
            None
        };
        // Le service est enregistré sans blancs autour : vide veut dire « non renseigné ».
        let nom = if avec_service && !c.service.is_empty() {
            format!("{}, {}", c.nom_complet(), c.service)
        } else {
            c.nom_complet()
        };
        OptionListe::nouvelle(c.id, nom, empechement, selection)
    }
}

#[derive(Template)]
#[template(path = "affectations/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieAffectation,
    erreur: Option<String>,
    vehicules: Vec<OptionListe>,
    conducteurs: Vec<OptionListe>,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    saisie: SaisieAffectation,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let aujourd_hui = aujourd_hui();
    let avec_service = ctx.montre_service();
    let vehicules = vehicules::lister(&etat.pool)
        .await?
        .iter()
        .map(|v| OptionListe::vehicule(v, &saisie.vehicule_id))
        .collect();
    let conducteurs = conducteurs::lister(&etat.pool)
        .await?
        .iter()
        .map(|c| OptionListe::conducteur(c, &saisie.conducteur_id, aujourd_hui, avec_service))
        .collect();
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle affectation".to_string(),
            Some(_) => "Modifier l'affectation".to_string(),
        },
        action: match id {
            None => "/affectations".to_string(),
            Some(id) => format!("/affectations/{id}"),
        },
        saisie,
        erreur,
        vehicules,
        conducteurs,
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let aujourd_hui = aujourd_hui();
    let affectations = affectations::lister(&etat.pool, aujourd_hui).await?;
    rendre(PageListe {
        ctx: utilisateur.contexte("affectations"),
        affectations: LigneAffectation::toutes(affectations, aujourd_hui),
    })
}

async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieAffectation>,
) -> Resultat<Html<String>> {
    if saisie.date_debut.is_empty() {
        saisie.date_debut = aujourd_hui().to_string();
    }
    rendre(formulaire(&etat, admin.contexte("affectations"), None, saisie, None).await?)
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieAffectation>,
) -> Resultat<Response> {
    match affectations::creer(&etat.pool, &saisie, aujourd_hui()).await {
        Ok(_) => Ok(fait::rediriger("/affectations", Fait::AffectationCreee)),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("affectations"),
                None,
                saisie,
                Some(message),
            )
            .await?,
        )?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

async fn editer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let affectation = affectations::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("affectations"),
            Some(id),
            SaisieAffectation::depuis(&affectation),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieAffectation>,
) -> Resultat<Response> {
    match affectations::modifier(&etat.pool, id, &saisie, aujourd_hui()).await {
        Ok(_) => Ok(fait::rediriger("/affectations", Fait::AffectationModifiee)),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("affectations"),
                Some(id),
                saisie,
                Some(message),
            )
            .await?,
        )?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

/// Clôture à la date du jour (bouton « Terminer » de la liste).
async fn cloturer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    affectations::cloturer(&etat.pool, id, aujourd_hui()).await?;
    Ok(fait::rediriger("/affectations", Fait::AffectationCloturee))
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    affectations::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger("/affectations", Fait::AffectationSupprimee))
}
