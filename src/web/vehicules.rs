//! Pages des véhicules : liste, fiche, création, modification, suppression.

use askama::Template;
use axum::extract::{FromRequest, Multipart, Query, Request, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

use crate::commun::types::{Carburant, Choix, Couleur, StatutVehicule, TypeDocument};
use crate::donnees::affectations::Affectation;
use crate::donnees::depenses::Depense;
use crate::donnees::documents::Document;
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::affectations::Conduite;
use crate::metier::consommation::{self, Resume};
use crate::metier::documents::SaisieDocument;
use crate::metier::vehicules::{self, SaisieVehicule};
use crate::metier::{affectations, depenses, documents, echeances, interventions};
use crate::web::chemin::Identifiant;
use crate::web::documents::{refus_d_envoi, FormulaireDepot};
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
        .route(
            "/vehicules/{id}/remettre-en-service",
            post(remettre_en_service),
        )
        .route("/vehicules/{id}/supprimer", post(supprimer))
}

/// Une ligne de la liste : le véhicule et, dans le profil entreprise, qui l'a.
struct LigneVehicule {
    v: Vehicule,
    conduite: Conduite,
}

#[derive(Template)]
#[template(path = "vehicules/liste.html")]
struct PageListe {
    ctx: Contexte,
    /// En service et en maintenance : la liste de travail.
    vehicules: Vec<LigneVehicule>,
    /// Réformés : repliés sous la liste, avec leur suppression définitive.
    reformes: Vec<LigneVehicule>,
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
    /// Le nuancier des couleurs de repère, rangée par rangée (`Couleur::rangs`).
    rangs_couleurs: Vec<Vec<Choix>>,
    /// Le champ « Photo du véhicule » : à la création seulement. Ensuite, la photo se remplace
    /// depuis la fiche, comme tout document.
    photo: bool,
    /// Le formulaire revient après un refus alors qu'une photo avait été choisie : le navigateur
    /// ne la garde pas, il faut le dire.
    photo_a_rechoisir: bool,
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
            rangs_couleurs: Couleur::rangs(),
            photo: true,
            photo_a_rechoisir: false,
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
            rangs_couleurs: Couleur::rangs(),
            photo: false,
            photo_a_rechoisir: false,
        }
    }
}

/// Ce que le formulaire « Nouveau véhicule » envoie : la saisie, et la photo si une a été choisie.
#[derive(Default)]
struct Creation {
    saisie: SaisieVehicule,
    photo: Option<SaisieDocument>,
}

/// Lit le formulaire de création. Il arrive en `multipart/form-data` depuis la page (le champ
/// photo l'impose) ; l'ancien encodage `application/x-www-form-urlencoded` reste accepté, sans
/// photo, pour qui envoie le formulaire autrement.
async fn recevoir_creation(requete: Request) -> Resultat<Creation> {
    let multipart = requete
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("multipart/form-data"));
    if !multipart {
        let Form(saisie) = Form::<SaisieVehicule>::from_request(requete, &())
            .await
            .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
        return Ok(Creation {
            saisie,
            photo: None,
        });
    }
    let mut multipart = Multipart::from_request(requete, &())
        .await
        .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
    let mut creation = Creation::default();
    while let Some(champ) = multipart.next_field().await.map_err(refus_d_envoi)? {
        let nom_du_champ = champ.name().unwrap_or("").to_string();
        if nom_du_champ == "photo" {
            let nom = champ.file_name().unwrap_or("").to_string();
            let type_mime = champ.content_type().unwrap_or("").to_string();
            let octets = champ.bytes().await.map_err(refus_d_envoi)?;
            // Sans photo choisie, le navigateur envoie quand même le champ, vide et sans nom.
            if !nom.is_empty() {
                creation.photo = Some(SaisieDocument {
                    type_document: TypeDocument::PhotoVehicule.code().to_string(),
                    nom_original: nom,
                    type_mime,
                    contenu: octets.to_vec(),
                });
            }
            continue;
        }
        let valeur = champ.text().await.map_err(refus_d_envoi)?;
        let s = &mut creation.saisie;
        match nom_du_champ.as_str() {
            "immatriculation" => s.immatriculation = valeur,
            "vin" => s.vin = valeur,
            "couleur" => s.couleur = valeur,
            "marque" => s.marque = valeur,
            "modele" => s.modele = valeur,
            "date_mise_en_service" => s.date_mise_en_service = valeur,
            "carburant" => s.carburant = valeur,
            "kilometrage" => s.kilometrage = valeur,
            "statut" => s.statut = valeur,
            _ => {}
        }
    }
    Ok(creation)
}

#[derive(Template)]
#[template(path = "vehicules/fiche.html")]
struct PageFiche {
    ctx: Contexte,
    vehicule: Vehicule,
    /// Affectations dont la période couvre aujourd'hui : un véhicule peut avoir plusieurs
    /// conducteurs à la fois.
    affectations_actives: Vec<Affectation>,
    /// Quand personne n'est en cours : la dernière affectation terminée, pour nommer le dernier
    /// conducteur en tête de fiche (#76).
    dernier_conducteur: Option<Affectation>,
    affectations: Vec<LigneAffectation>,
    interventions: Vec<Intervention>,
    /// Les dernières dépenses du véhicule (#126), dix au plus : la liste filtrée a le reste.
    depenses: Vec<Depense>,
    /// Combien le véhicule en a en tout, pour le lien « Toutes les dépenses (N) ».
    nb_depenses: usize,
    /// Ce que ses dépenses résument : totaux, consommation, coût au kilomètre (#126, bloc 3).
    /// Absent quand il n'a aucune dépense.
    resume: Option<Resume>,
    echeances: Vec<LigneEcheance>,
    /// Les documents rangés par dossier (#109), tous les dossiers, vides compris : le gabarit ne
    /// montre les vides qu'à l'administrateur.
    dossiers: Vec<documents::DossierDeDocuments>,
    /// Aucun document dans aucun dossier : le lecteur, qui ne voit pas les dossiers vides, lit
    /// « Aucun document ».
    sans_document: bool,
    /// La photo de la fiche, pour la vignette en tête : celle qu'on a choisie, à défaut la plus
    /// récente.
    photo: Option<Document>,
    /// Toutes les photos du véhicule, la photo de la fiche en premier (bloc « Photos », #66).
    photos: Vec<Document>,
    depot: FormulaireDepot,
    /// Motif du refus quand un dépôt de document vient d'échouer.
    erreur_depot: Option<String>,
    /// Motif du refus d'une autre action faite depuis la fiche (relevé du kilométrage).
    erreur: Option<String>,
}

/// Combien de dépenses la fiche montre (#126) : les pleins s'accumulent vite, la fiche garde les
/// dernières et renvoie à la liste pour le reste.
const DEPENSES_SUR_LA_FICHE: usize = 10;

/// Le relevé du kilométrage, depuis la carte d'accueil (`?retour=accueil`) ou la fiche.
#[derive(Debug, Default, Deserialize)]
struct Releve {
    #[serde(default)]
    kilometrage: String,
}

/// La page d'où part un geste en un clic (`?retour=accueil`, `liste`, `fiche`) : il y revient.
#[derive(Debug, Default, Deserialize)]
struct Origine {
    #[serde(default)]
    retour: String,
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    let ctx = utilisateur.contexte("vehicules");
    let aujourd_hui = chrono::Local::now().date_naive();
    // Qui a chaque véhicule n'existe que dans le profil entreprise : le particulier n'a ni
    // affectation ni colonne, donc pas de lecture en plus.
    let mut conduites = if ctx.montre_affectations() {
        affectations::conduites_par_vehicule(
            affectations::lister(&etat.pool, aujourd_hui).await?,
            aujourd_hui,
        )
    } else {
        Default::default()
    };
    let (reformes, vehicules): (Vec<_>, Vec<_>) = vehicules::lister(&etat.pool)
        .await?
        .into_iter()
        .map(|v| LigneVehicule {
            conduite: conduites.remove(&v.id).unwrap_or_default(),
            v,
        })
        .partition(|l| l.v.est_reforme());
    rendre(PageListe {
        ctx,
        vehicules,
        reformes,
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
    let photos = documents::photos_du_vehicule(&etat.pool, id).await?;
    let aujourd_hui = chrono::Local::now().date_naive();
    let conduite = affectations::conduite(&affectations, aujourd_hui);
    let dossiers = documents::par_dossier(documents::lister_par_vehicule(&etat.pool, id).await?);
    let mut depenses = depenses::lister_par_vehicule(&etat.pool, id).await?;
    let nb_depenses = depenses.len();
    let interventions = interventions::lister_par_vehicule(&etat.pool, id).await?;
    // Le résumé se calcule sur toutes les dépenses, avant de ne garder que les dix dernières.
    let resume = (!depenses.is_empty()).then(|| {
        consommation::resume(
            &depenses,
            &depenses::releves_des_interventions(&interventions),
            aujourd_hui,
        )
    });
    depenses.truncate(DEPENSES_SUR_LA_FICHE);
    rendre(PageFiche {
        ctx,
        affectations_actives: conduite.en_cours,
        dernier_conducteur: conduite.dernier,
        affectations: LigneAffectation::toutes(affectations, aujourd_hui),
        interventions,
        depenses,
        nb_depenses,
        resume,
        echeances: LigneEcheance::toutes(
            echeances::lister_par_vehicule(&etat.pool, id).await?,
            aujourd_hui,
            etat.config.alertes_horizon_jours,
        ),
        sans_document: dossiers.iter().all(|d| d.documents.is_empty()),
        dossiers,
        photo: photos.first().cloned(),
        photos,
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
    Query(origine): Query<Origine>,
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

/// Crée le véhicule, avec sa photo si une a été choisie. Un refus (saisie, ou photo illisible,
/// trop grosse…) ré-affiche le formulaire avec le motif et la saisie ; la photo, elle, est à
/// choisir de nouveau, le navigateur ne la conserve pas.
async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    requete: Request,
) -> Resultat<Response> {
    let mut creation = Creation::default();
    let resultat = match recevoir_creation(requete).await {
        Ok(recue) => {
            creation = recue;
            vehicules::creer_avec_photo(
                &etat.pool,
                &etat.config.documents_dir,
                &creation.saisie,
                creation.photo.as_ref(),
            )
            .await
        }
        Err(erreur) => Err(erreur),
    };
    match resultat {
        Ok(vehicule) => Ok(fait::rediriger(
            &format!("/vehicules/{}", vehicule.id),
            if creation.photo.is_some() {
                Fait::VehiculeCreeAvecPhoto
            } else {
                Fait::VehiculeCree
            },
        )),
        Err(ErreurApp::Metier(message)) => {
            let mut page = PageFormulaire::nouveau(
                admin.contexte("vehicules"),
                creation.saisie,
                Some(message),
            );
            page.photo_a_rechoisir = creation.photo.is_some();
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

/// Remet un véhicule réformé en service, d'un clic : sans boîte de confirmation, le geste se
/// défait (« Modifier », champ « Statut »). Retour à la liste ou à la fiche, d'où il vient. Le
/// message ne parle des affectations que dans le profil entreprise, qui seul en a.
async fn remettre_en_service(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Query(origine): Query<Origine>,
) -> Resultat<Response> {
    vehicules::remettre_en_service(&etat.pool, id).await?;
    let vers = match origine.retour.as_str() {
        "liste" => "/vehicules".to_string(),
        _ => format!("/vehicules/{id}"),
    };
    let fait = if admin.contexte("vehicules").montre_affectations() {
        Fait::VehiculeRemisEnService
    } else {
        Fait::VehiculeRemisEnServiceSansAffectations
    };
    Ok(fait::rediriger(&vers, fait))
}
