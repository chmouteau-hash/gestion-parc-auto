//! Pages des échéances réglementaires et d'entretien.

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

use crate::commun::types::{Choix, Operation, StatutEcheance, TypeEcheance};
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::echeances::{self, SaisieEcheance};
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte, LigneEcheance};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/echeances", get(liste).post(creer))
        .route("/echeances/nouvelle", get(nouvelle))
        .route("/echeances/{id}", post(modifier))
        .route("/echeances/{id}/modifier", get(editer))
        .route("/echeances/{id}/traiter", post(traiter))
        .route("/echeances/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "echeances/liste.html")]
struct PageListe {
    ctx: Contexte,
    echeances: Vec<LigneEcheance>,
}

#[derive(Template)]
#[template(path = "echeances/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieEcheance,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    types: Vec<Choix>,
    /// Ce qu'il y a à faire : le vocabulaire des opérations, comme « Ce qui a été fait ».
    operations: Vec<Choix>,
    statuts: Vec<Choix>,
    /// Où mène « Annuler » : la liste, ou la page d'où venait « Marquer traitée ».
    annuler: String,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    mut saisie: SaisieEcheance,
    erreur: Option<String>,
    annuler: String,
) -> Resultat<PageFormulaire> {
    if saisie.statut.is_empty() {
        saisie.statut = StatutEcheance::Ouverte.code().to_string();
    }
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle échéance".to_string(),
            Some(_) => "Modifier l'échéance".to_string(),
        },
        action: match id {
            None => "/echeances".to_string(),
            Some(id) => format!("/echeances/{id}"),
        },
        saisie,
        erreur,
        vehicules: vehicules::lister(&etat.pool).await?,
        types: TypeEcheance::choix(),
        operations: Operation::choix(),
        statuts: StatutEcheance::choix(),
        annuler,
    })
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let aujourd_hui = chrono::Local::now().date_naive();
    rendre(PageListe {
        ctx: utilisateur.contexte("echeances"),
        echeances: LigneEcheance::toutes(
            echeances::lister(&etat.pool).await?,
            aujourd_hui,
            etat.config.alertes_horizon_jours,
        ),
    })
}

/// D'où vient le clic sur « Marquer traitée », et donc où ramène « Annuler » sur le formulaire
/// de la prochaine échéance.
#[derive(Debug, Default, Deserialize)]
struct Origine {
    /// `vehicule` depuis la fiche d'un véhicule, `alertes` depuis la page Alertes, `accueil`
    /// depuis l'accueil du profil ; absent ou autre, depuis la liste des échéances.
    #[serde(default)]
    retour: String,
}

impl Origine {
    /// Le mot-clé reconnu, ou `liste` : l'adresse ne donne jamais une destination, seulement un
    /// mot, et un lien forgé ne peut pas faire rediriger vers un autre site.
    fn mot_cle(&self) -> &'static str {
        match self.retour.as_str() {
            "vehicule" => "vehicule",
            "alertes" => "alertes",
            "accueil" => "accueil",
            _ => "liste",
        }
    }

    fn page(&self, vehicule_id: Option<i64>) -> String {
        match (self.mot_cle(), vehicule_id) {
            ("vehicule", Some(id)) => format!("/vehicules/{id}"),
            ("alertes", _) => "/alertes".to_string(),
            ("accueil", _) => "/".to_string(),
            _ => "/echeances".to_string(),
        }
    }

    /// L'adresse où le formulaire de création s'envoie. Elle garde le mot-clé d'origine : si
    /// l'enregistrement est refusé, la réponse sait encore d'où l'on venait et « Annuler » y
    /// ramène. Depuis la liste, rien à garder.
    fn action_creation(&self) -> String {
        match self.mot_cle() {
            "liste" => "/echeances".to_string(),
            mot => format!("/echeances?retour={mot}"),
        }
    }
}

/// Le formulaire vierge accepte des valeurs dans l'adresse : c'est ainsi que « Marquer traitée »
/// propose la prochaine échéance, pré-remplie, avec `retour` pour savoir où « Annuler » ramène.
async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieEcheance>,
    Query(origine): Query<Origine>,
) -> Resultat<Html<String>> {
    // Une chose à faire sans date dans l'adresse (« Noter ce qu'il y a à faire » depuis une fiche
    // ou le formulaire d'intervention) se propose dans un mois ; l'utilisateur la corrige.
    if saisie.type_echeance == TypeEcheance::AFaire.code() && saisie.date_limite.trim().is_empty() {
        let aujourd_hui = chrono::Local::now().date_naive();
        saisie.date_limite = aujourd_hui
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(aujourd_hui)
            .to_string();
    }
    let annuler = origine.page(saisie.vehicule_id.trim().parse().ok());
    let mut page = formulaire(
        &etat,
        admin.contexte("echeances"),
        None,
        saisie,
        None,
        annuler,
    )
    .await?;
    page.action = origine.action_creation();
    rendre(page)
}

/// Le mot-clé `retour` voyage dans l'adresse d'envoi du formulaire (voir
/// [`Origine::action_creation`]) : un refus ré-affiche le formulaire avec le même « Annuler »
/// qu'avant l'envoi, et le même mot-clé pour un second essai.
async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(origine): Query<Origine>,
    Form(saisie): Form<SaisieEcheance>,
) -> Resultat<Response> {
    match echeances::creer(&etat.pool, &saisie).await {
        Ok(e) => Ok(fait::rediriger(
            &format!("/vehicules/{}", e.vehicule_id),
            Fait::EcheanceCreee,
        )),
        Err(ErreurApp::Metier(message)) => {
            let annuler = origine.page(saisie.vehicule_id.trim().parse().ok());
            let mut page = formulaire(
                &etat,
                admin.contexte("echeances"),
                None,
                saisie,
                Some(message),
                annuler,
            )
            .await?;
            page.action = origine.action_creation();
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
    let echeance = echeances::obtenir(&etat.pool, id).await?;
    rendre(
        formulaire(
            &etat,
            admin.contexte("echeances"),
            Some(id),
            SaisieEcheance::depuis(&echeance),
            None,
            "/echeances".to_string(),
        )
        .await?,
    )
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieEcheance>,
) -> Resultat<Response> {
    match echeances::modifier(&etat.pool, id, &saisie).await {
        Ok(e) => Ok(fait::rediriger(
            &format!("/vehicules/{}", e.vehicule_id),
            Fait::EcheanceModifiee,
        )),
        Err(ErreurApp::Metier(message)) => Ok(rendre(
            formulaire(
                &etat,
                admin.contexte("echeances"),
                Some(id),
                saisie,
                Some(message),
                "/echeances".to_string(),
            )
            .await?,
        )?
        .into_response()),
        Err(autre) => Err(autre),
    }
}

/// Bouton « Marquer traitée » de la fiche d'un véhicule, de la liste des échéances, de la page
/// Alertes et de l'accueil. L'échéance est traitée, puis le formulaire de la **prochaine** s'ouvre,
/// pré-rempli (même véhicule, même type, date décalée) : l'utilisateur l'enregistre, ou revient
/// par « Annuler » à la page d'où il est parti. Un « à faire » n'a pas de suivante : fait, il est
/// fait, et l'on revient à la page d'où l'on est parti avec le message de confirmation.
async fn traiter(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Query(origine): Query<Origine>,
) -> Resultat<Response> {
    let echeance = echeances::traiter(&etat.pool, id).await?;
    let vers = match echeances::prochaine(&echeance, chrono::Local::now().date_naive()) {
        Some(suivante) => format!(
            "/echeances/nouvelle?vehicule_id={}&type_echeance={}&date_limite={}&retour={}",
            suivante.vehicule_id,
            suivante.type_echeance,
            suivante.date_limite,
            origine.mot_cle()
        ),
        None => origine.page(Some(echeance.vehicule_id)),
    };
    Ok(fait::rediriger(&vers, Fait::EcheanceTraitee))
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    echeances::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger("/echeances", Fait::EcheanceSupprimee))
}
