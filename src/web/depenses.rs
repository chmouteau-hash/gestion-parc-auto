//! Pages des dépenses : les pleins de carburant et les tickets de péage (#126).

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

use crate::commun::types::{Carburant, Choix, TypeDepense};
use crate::donnees::depenses::Depense;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::depenses::{self, SaisieDepense};
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/depenses", get(liste).post(creer))
        .route("/depenses/nouvelle", get(nouvelle))
        .route("/depenses/{id}", post(modifier))
        .route("/depenses/{id}/modifier", get(editer))
        .route("/depenses/{id}/supprimer", post(supprimer))
}

/// La liste se restreint à un véhicule (`/depenses?vehicule_id=3`) : c'est le lien « Toutes les
/// dépenses » de sa fiche.
#[derive(Debug, Default, Deserialize)]
struct Filtre {
    #[serde(default)]
    vehicule_id: String,
}

#[derive(Template)]
#[template(path = "depenses/liste.html")]
struct PageListe {
    ctx: Contexte,
    depenses: Vec<Depense>,
    /// Le véhicule sur lequel la liste est restreinte, s'il y en a un.
    vehicule: Option<Vehicule>,
}

#[derive(Template)]
#[template(path = "depenses/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieDepense,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    types: Vec<Choix>,
    /// L'unité de la quantité pour le véhicule déjà choisi (« L », ou « kWh ») ; `app.js` la
    /// change quand le véhicule change.
    unite: &'static str,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    saisie: SaisieDepense,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let vehicules = vehicules::lister(&etat.pool).await?;
    let unite = vehicules
        .iter()
        .find(|v| v.id.to_string() == saisie.vehicule_id)
        .and_then(|v| Carburant::depuis_code(&v.carburant))
        .map(Carburant::unite_de_plein)
        .unwrap_or("L");
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle dépense".to_string(),
            Some(_) => "Modifier la dépense".to_string(),
        },
        action: match id {
            None => "/depenses".to_string(),
            Some(id) => format!("/depenses/{id}"),
        },
        saisie,
        erreur,
        vehicules,
        types: TypeDepense::choix(),
        unite,
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Query(filtre): Query<Filtre>,
) -> Resultat<Html<String>> {
    let ctx = utilisateur.contexte("depenses");
    let (vehicule, depenses) = match filtre.vehicule_id.trim().parse::<i64>() {
        Ok(id) => (
            Some(vehicules::obtenir(&etat.pool, id).await?),
            depenses::lister_par_vehicule(&etat.pool, id).await?,
        ),
        Err(_) => (None, depenses::lister(&etat.pool).await?),
    };
    rendre(PageListe {
        ctx,
        depenses,
        vehicule,
    })
}

/// Le formulaire vierge accepte des valeurs dans l'adresse (`?vehicule_id=3&type_depense=peage`)
/// : les boutons « Noter un plein » et « Noter un péage » de la fiche s'en servent. Le type est
/// « Carburant » et la case « plein complet » cochée tant que rien n'est dit ; le kilométrage
/// n'est jamais pré-rempli, pour qu'un compteur d'hier ne passe pas pour celui du plein.
async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieDepense>,
) -> Resultat<Html<String>> {
    if saisie.date_depense.is_empty() {
        saisie.date_depense = chrono::Local::now().date_naive().to_string();
    }
    if saisie.type_depense.is_empty() {
        saisie.type_depense = TypeDepense::Carburant.code().to_string();
    }
    if saisie.plein_complet.is_empty() {
        saisie.plein_complet = "oui".to_string();
    }
    rendre(formulaire(&etat, admin.contexte("depenses"), None, saisie, None).await?)
}

/// Le message dit ce qui a été noté : un plein, ou un péage.
fn fait_enregistre(depense: &Depense) -> Fait {
    if depense.est_un_plein() {
        Fait::PleinEnregistre
    } else {
        Fait::PeageEnregistre
    }
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieDepense>,
) -> Resultat<Response> {
    match depenses::creer(&etat.pool, &saisie).await {
        Ok(d) => Ok(fait::rediriger(
            &format!("/vehicules/{}", d.vehicule_id),
            fait_enregistre(&d),
        )),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("depenses"),
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
    let depense = depenses::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("depenses"),
            Some(id),
            SaisieDepense::depuis(&depense),
            None,
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieDepense>,
) -> Resultat<Response> {
    match depenses::modifier(&etat.pool, id, &saisie).await {
        Ok(d) => Ok(fait::rediriger(
            &format!("/vehicules/{}", d.vehicule_id),
            Fait::DepenseModifiee,
        )),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("depenses"),
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

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    depenses::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger("/depenses", Fait::DepenseSupprimee))
}
