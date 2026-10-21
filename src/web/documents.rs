//! Dépôt, affichage, téléchargement, changement de nom et suppression des documents rattachés à un véhicule ou un
//! conducteur.
//!
//! Un document se dépose depuis la fiche d'un véhicule ou d'un conducteur, ou depuis la page
//! « Déposer un document » (`/documents/nouveau`), pensée pour le téléphone : on choisit le
//! véhicule et le type, on prend la photo, elle est enregistrée.
//!
//! Les fichiers passent par des routes authentifiées. La page d'un document (`/documents/{id}`)
//! l'encadre : une image y est affichée et s'imprime ; un PDF s'ouvre dans un autre onglet, où
//! le lecteur du navigateur l'imprime (les en-têtes de sécurité interdisent de le mettre dans un
//! cadre, même depuis ce site). Seuls une image ou un PDF s'affichent dans le navigateur
//! (`/documents/{id}/voir`) ; tout autre fichier est renvoyé en pièce jointe
//! (`Content-Disposition: attachment`) : un fichier HTML déposé n'est jamais exécuté par le
//! navigateur dans le contexte du site.

use askama::Template;
use axum::extract::multipart::MultipartError;
use axum::extract::{Multipart, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;

use crate::commun::types::{Choix, Dossier};
use crate::donnees::depenses::Depense;
use crate::donnees::documents::Document;
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::conducteurs as metier_conducteurs;
use crate::metier::depenses as metier_depenses;
use crate::metier::documents::{self, Rattachement, SaisieDocument};
use crate::metier::interventions as metier_interventions;
use crate::metier::validation;
use crate::metier::vehicules as metier_vehicules;
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};
use crate::web::{conducteurs, vehicules};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/documents", post(deposer_depuis_la_page))
        .route("/documents/nouveau", get(page_depot))
        .route("/vehicules/{id}/documents", post(deposer_pour_vehicule))
        .route("/conducteurs/{id}/documents", post(deposer_pour_conducteur))
        .route("/documents/{id}", get(page_document))
        .route("/documents/{id}/voir", get(voir))
        .route("/documents/{id}/vignette", get(vignette))
        .route("/documents/{id}/telecharger", get(telecharger))
        .route(
            "/documents/{id}/renommer",
            get(page_renommer).post(renommer),
        )
        .route("/documents/{id}/supprimer", post(supprimer))
        .route("/documents/{id}/mettre-en-tete", post(mettre_en_tete))
}

/// Le formulaire de dépôt (`templates/documents/depot.html`), commun aux deux fiches et à la
/// page « Déposer un document ».
pub(super) struct FormulaireDepot {
    pub action: String,
    /// Véhicules à proposer. Vide sur une fiche : c'est alors l'adresse qui dit à quoi rattacher
    /// le fichier, et la liste déroulante n'est pas affichée.
    pub vehicules: Vec<Vehicule>,
    pub vehicule_choisi: String,
    pub types: Vec<Choix>,
    pub type_choisi: String,
}

impl FormulaireDepot {
    pub(super) fn pour_vehicule(id: i64) -> FormulaireDepot {
        FormulaireDepot {
            action: format!("/vehicules/{id}/documents"),
            vehicules: Vec::new(),
            vehicule_choisi: String::new(),
            types: documents::types_pour_vehicule(),
            type_choisi: String::new(),
        }
    }

    pub(super) fn pour_conducteur(id: i64) -> FormulaireDepot {
        FormulaireDepot {
            action: format!("/conducteurs/{id}/documents"),
            vehicules: Vec::new(),
            vehicule_choisi: String::new(),
            types: documents::types_pour_conducteur(),
            type_choisi: String::new(),
        }
    }
}

/// Champs lus dans le formulaire de dépôt.
#[derive(Default)]
struct Envoi {
    /// Champ `vehicule_id` : seule la page « Déposer un document » l'envoie.
    vehicule_id: String,
    /// Champ `intervention_id`, facultatif : relie le document à une intervention du véhicule,
    /// sa facture (#100).
    intervention_id: String,
    /// Champ `depense_id`, facultatif : relie le document à une dépense du véhicule, son ticket
    /// (#126, bloc 2).
    depense_id: String,
    saisie: SaisieDocument,
}

#[derive(Template)]
#[template(path = "documents/nouveau.html")]
struct PageDepot {
    ctx: Contexte,
    depot: FormulaireDepot,
    erreur: Option<String>,
    /// Le document qui vient d'être enregistré, à confirmer en tête de page.
    depose: Option<Depose>,
}

struct Depose {
    document: Document,
    vehicule: Vehicule,
}

/// Ce que l'adresse de la page de dépôt peut porter.
#[derive(Debug, Default, Deserialize)]
struct ParametresDepot {
    /// Véhicule à présélectionner.
    #[serde(default)]
    vehicule_id: String,
    /// Type à présélectionner (« Ajouter une photo » depuis une fiche).
    #[serde(default)]
    type_document: String,
    /// Dossier de la fiche d'où vient « Ajouter » (#109) : la liste ne propose que ses types,
    /// le premier pré-choisi. Un code inconnu est ignoré : liste complète.
    #[serde(default)]
    dossier: String,
    /// Identifiant du document qui vient d'être enregistré : la page le confirme.
    #[serde(default)]
    depose: String,
}

async fn page_depot(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(parametres): Query<ParametresDepot>,
) -> Resultat<Html<String>> {
    let mut envoi = Envoi {
        vehicule_id: parametres.vehicule_id,
        ..Envoi::default()
    };
    envoi.saisie.type_document = parametres.type_document;
    let dossier = Dossier::depuis_code(&parametres.dossier);
    let liste = metier_vehicules::lister(&etat.pool).await?;

    // Un identifiant illisible, ou celui d'un document supprimé depuis, n'affiche rien.
    let mut depose = None;
    if let Ok(id) = parametres.depose.parse::<i64>() {
        let document = match documents::obtenir(&etat.pool, id).await {
            Ok(document) => Some(document),
            Err(ErreurApp::Introuvable) => None,
            Err(autre) => return Err(autre),
        };
        let vehicule = document
            .as_ref()
            .and_then(|d| liste.iter().find(|v| Some(v.id) == d.vehicule_id).cloned());
        if let (Some(document), Some(vehicule)) = (document, vehicule) {
            // Le même véhicule reste choisi : une facture et son procès-verbal se suivent.
            envoi.vehicule_id = vehicule.id.to_string();
            depose = Some(Depose { document, vehicule });
        }
    }

    rendre(PageDepot {
        ctx: admin.contexte("documents"),
        depot: formulaire_de_la_page(liste, &envoi, dossier),
        erreur: None,
        depose,
    })
}

/// Le formulaire de la page. Ouvert sur un dossier, il ne propose que les types de ce dossier,
/// le premier pré-choisi quand l'adresse n'en nomme pas ; sinon tous les types d'un véhicule.
fn formulaire_de_la_page(
    vehicules: Vec<Vehicule>,
    envoi: &Envoi,
    dossier: Option<Dossier>,
) -> FormulaireDepot {
    let types = match dossier {
        Some(dossier) => documents::types_du_dossier(dossier),
        None => documents::types_pour_vehicule(),
    };
    let mut type_choisi = envoi.saisie.type_document.trim().to_string();
    if type_choisi.is_empty() && dossier.is_some() {
        type_choisi = types[0].code.to_string();
    }
    FormulaireDepot {
        action: "/documents".to_string(),
        vehicules,
        vehicule_choisi: envoi.vehicule_id.trim().to_string(),
        types,
        type_choisi,
    }
}

/// Dépôt depuis la page « Déposer un document » : le véhicule vient du formulaire.
///
/// Un dépôt refusé ré-affiche la page avec le motif, le véhicule et le type déjà choisis.
async fn deposer_depuis_la_page(
    admin: Administrateur,
    State(etat): State<Etat>,
    mut multipart: Multipart,
) -> Resultat<Response> {
    let mut envoi = Envoi::default();
    let resultat = match recevoir(&mut multipart, &mut envoi).await {
        Ok(()) => match Rattachement::vehicule_choisi(&envoi.vehicule_id) {
            Ok(rattachement) => enregistrer(&etat, rattachement, &envoi).await,
            Err(erreur) => Err(erreur),
        },
        Err(erreur) => Err(erreur),
    };
    match resultat {
        Ok(document) => {
            Ok(Redirect::to(&format!("/documents/nouveau?depose={}", document.id)).into_response())
        }
        Err(ErreurApp::Metier(message)) => {
            let liste = metier_vehicules::lister(&etat.pool).await?;
            Ok(rendre(PageDepot {
                ctx: admin.contexte("documents"),
                depot: formulaire_de_la_page(liste, &envoi, None),
                erreur: Some(message),
                depose: None,
            })?
            .into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn deposer_pour_vehicule(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    multipart: Multipart,
) -> Resultat<Response> {
    deposer(&admin, &etat, Rattachement::Vehicule(id), multipart).await
}

async fn deposer_pour_conducteur(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    multipart: Multipart,
) -> Resultat<Response> {
    deposer(&admin, &etat, Rattachement::Conducteur(id), multipart).await
}

/// Dépôt depuis une fiche (`multipart/form-data`, champs `type_document` et `fichier`).
///
/// C'est l'adresse qui dit à quoi rattacher le fichier. Elle est connue avant toute lecture de
/// l'envoi : un dépôt refusé (fichier trop gros, vide…) ré-affiche donc toujours la fiche d'où
/// il vient, avec le motif.
async fn deposer(
    admin: &Administrateur,
    etat: &Etat,
    rattachement: Rattachement,
    mut multipart: Multipart,
) -> Resultat<Response> {
    let mut envoi = Envoi::default();
    let resultat = match recevoir(&mut multipart, &mut envoi).await {
        Ok(()) => enregistrer(etat, rattachement, &envoi).await,
        Err(erreur) => Err(erreur),
    };
    match resultat {
        Ok(_) => Ok(fait::rediriger(
            &page_de_retour(rattachement),
            Fait::DocumentDepose,
        )),
        Err(ErreurApp::Metier(message)) => {
            let page = match rattachement {
                Rattachement::Vehicule(id) => {
                    let ctx = admin.contexte("vehicules");
                    vehicules::page_fiche(etat, ctx, id, Some(message), None).await?
                }
                Rattachement::Conducteur(id) => {
                    let ctx = admin.contexte("conducteurs");
                    conducteurs::page_fiche(etat, ctx, id, Some(message)).await?
                }
            };
            Ok(page.into_response())
        }
        Err(autre) => Err(autre),
    }
}

/// Lit les champs du formulaire dans `envoi`, au fur et à mesure : si la lecture échoue en
/// cours de route (fichier trop gros), les champs déjà lus restent connus.
async fn recevoir(multipart: &mut Multipart, envoi: &mut Envoi) -> Resultat<()> {
    while let Some(champ) = multipart.next_field().await.map_err(refus_d_envoi)? {
        let nom_du_champ = champ.name().unwrap_or("").to_string();
        match nom_du_champ.as_str() {
            "vehicule_id" => envoi.vehicule_id = champ.text().await.map_err(refus_d_envoi)?,
            "intervention_id" => {
                envoi.intervention_id = champ.text().await.map_err(refus_d_envoi)?;
            }
            "depense_id" => {
                envoi.depense_id = champ.text().await.map_err(refus_d_envoi)?;
            }
            "type_document" => {
                envoi.saisie.type_document = champ.text().await.map_err(refus_d_envoi)?;
            }
            "fichier" => {
                let nom = champ.file_name().unwrap_or("").to_string();
                let type_mime = champ.content_type().unwrap_or("").to_string();
                let octets = champ.bytes().await.map_err(refus_d_envoi)?;
                // Le formulaire a deux champs `fichier` (appareil photo, fichier existant) :
                // celui qui n'a pas servi arrive sans nom. On garde le premier qui en a un.
                if envoi.saisie.nom_original.is_empty() && !nom.is_empty() {
                    envoi.saisie.nom_original = nom;
                    envoi.saisie.type_mime = type_mime;
                    envoi.saisie.contenu = octets.to_vec();
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn enregistrer(etat: &Etat, rattachement: Rattachement, envoi: &Envoi) -> Resultat<Document> {
    if !envoi.depense_id.trim().is_empty() {
        return enregistrer_pour_depense(etat, rattachement, envoi).await;
    }
    if envoi.intervention_id.trim().is_empty() {
        return documents::deposer(
            &etat.pool,
            &etat.config.documents_dir,
            rattachement,
            &envoi.saisie,
        )
        .await;
    }
    // Relié à une intervention (sa facture) : elle doit être une intervention de ce véhicule.
    let intervention_id = validation::identifiant(&envoi.intervention_id, "intervention")?;
    let intervention = metier_interventions::obtenir(&etat.pool, intervention_id)
        .await
        .map_err(|erreur| match erreur {
            ErreurApp::Introuvable => ErreurApp::metier("L'intervention indiquée n'existe plus."),
            autre => autre,
        })?;
    match rattachement {
        Rattachement::Vehicule(vehicule_id) if intervention.vehicule_id == vehicule_id => {
            documents::deposer_pour_intervention(
                &etat.pool,
                &etat.config.documents_dir,
                vehicule_id,
                intervention_id,
                &envoi.saisie,
            )
            .await
        }
        _ => Err(ErreurApp::metier(
            "L'intervention indiquée n'est pas une intervention de ce véhicule.",
        )),
    }
}

/// Relié à une dépense (son ticket, #126) : elle doit être une dépense de ce véhicule.
async fn enregistrer_pour_depense(
    etat: &Etat,
    rattachement: Rattachement,
    envoi: &Envoi,
) -> Resultat<Document> {
    let depense_id = validation::identifiant(&envoi.depense_id, "dépense")?;
    let depense = metier_depenses::obtenir(&etat.pool, depense_id)
        .await
        .map_err(|erreur| match erreur {
            ErreurApp::Introuvable => ErreurApp::metier("La dépense indiquée n'existe plus."),
            autre => autre,
        })?;
    match rattachement {
        Rattachement::Vehicule(vehicule_id) if depense.vehicule_id == vehicule_id => {
            documents::deposer_pour_depense(
                &etat.pool,
                &etat.config.documents_dir,
                vehicule_id,
                depense_id,
                &envoi.saisie,
            )
            .await
        }
        _ => Err(ErreurApp::metier(
            "La dépense indiquée n'est pas une dépense de ce véhicule.",
        )),
    }
}

/// Traduit un échec de lecture du formulaire. Le serveur cesse de lire dès que l'envoi dépasse
/// la taille permise : c'est le cas du fichier trop gros. Pour tout autre échec, le détail
/// technique (en anglais) va au journal, pas à l'écran.
pub(super) fn refus_d_envoi(erreur: MultipartError) -> ErreurApp {
    if erreur.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ErreurApp::metier(documents::message_trop_volumineux())
    } else {
        tracing::warn!(%erreur, "formulaire de dépôt illisible");
        ErreurApp::metier("L'envoi du fichier a échoué. Réessayez.")
    }
}

/// La page d'un document (`templates/documents/voir.html`) : le fichier dans le cadre de
/// l'application, avec de quoi l'imprimer, le télécharger ou l'ouvrir en plein écran.
#[derive(Template)]
#[template(path = "documents/voir.html")]
struct PageDocument {
    ctx: Contexte,
    document: Document,
    rattache: Rattache,
    /// L'intervention que le document justifie, s'il y a été joint (#100).
    intervention: Option<Intervention>,
    /// La dépense dont il est le ticket, s'il y a été joint (#126, bloc 2).
    depense: Option<Depense>,
    est_image: bool,
    est_pdf: bool,
}

/// La fiche à laquelle le document est rattaché : où revenir, et comment la nommer.
struct Rattache {
    /// La fiche, à la hauteur de ses documents.
    lien: String,
    /// Le nom court, pour le lien de retour : l'immatriculation, ou le conducteur.
    nom: String,
    /// Le nom complet, pour le titre : « AB-123-CD (Renault Clio) », ou le conducteur.
    designation: String,
}

async fn page_document(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let document = documents::obtenir(&etat.pool, id).await?;
    let (page, rattache) = match (document.vehicule_id, document.conducteur_id) {
        (Some(id), _) => {
            let vehicule = metier_vehicules::obtenir(&etat.pool, id).await?;
            let rattache = Rattache {
                lien: page_de_retour(Rattachement::Vehicule(id)),
                nom: vehicule.immatriculation.clone(),
                designation: vehicule.designation(),
            };
            ("vehicules", rattache)
        }
        (_, Some(id)) => {
            let conducteur = metier_conducteurs::obtenir(&etat.pool, id).await?;
            let nom = conducteur.nom_complet();
            let rattache = Rattache {
                lien: page_de_retour(Rattachement::Conducteur(id)),
                designation: nom.clone(),
                nom,
            };
            ("conducteurs", rattache)
        }
        // La base l'interdit (un document est rattaché à l'un ou l'autre) : par prudence.
        _ => return Err(ErreurApp::Introuvable),
    };
    let intervention = match document.intervention_id {
        Some(id) => Some(metier_interventions::obtenir(&etat.pool, id).await?),
        None => None,
    };
    let depense = match document.depense_id {
        Some(id) => Some(metier_depenses::obtenir(&etat.pool, id).await?),
        None => None,
    };
    rendre(PageDocument {
        ctx: utilisateur.contexte(page),
        est_image: document.est_image(),
        est_pdf: document.est_pdf(),
        document,
        rattache,
        intervention,
        depense,
    })
}

/// Montre le document dans le navigateur si c'est une image ou un PDF ; sinon le fait
/// télécharger, comme [`telecharger`].
async fn voir(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    servir(&etat, id, true).await
}

/// La vignette d'une photo du véhicule (JPEG, 640 px au plus). Un jour de cache, privé : la
/// vignette d'un document ne change jamais, un nouveau dépôt fait un nouvel identifiant.
async fn vignette(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let octets = documents::lire_vignette(&etat.pool, &etat.config.documents_dir, id).await?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg")),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("inline"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=86400"),
            ),
        ],
        octets,
    )
        .into_response())
}

async fn telecharger(
    _utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    servir(&etat, id, false).await
}

async fn servir(etat: &Etat, id: i64, afficher: bool) -> Resultat<Response> {
    let (document, contenu) = documents::lire(&etat.pool, &etat.config.documents_dir, id).await?;
    let type_mime = HeaderValue::from_str(&document.type_mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mode = if afficher && document.est_affichable() {
        "inline"
    } else {
        "attachment"
    };
    let disposition = HeaderValue::from_str(&documents::en_tete_disposition(
        mode,
        &document.nom_original,
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, type_mime),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        contenu,
    )
        .into_response())
}

/// Le changement de nom (`templates/documents/renommer.html`) : on ne saisit que la racine du nom,
/// l'extension du fichier reste affichée à côté, fixe.
#[derive(Template)]
#[template(path = "documents/renommer.html")]
struct PageRenommer {
    ctx: Contexte,
    document: Document,
    /// Le nom sans son extension : celui du fichier, ou ce qui vient d'être refusé.
    nom: String,
    extension: String,
    erreur: Option<String>,
}

/// Le nom proposé par le formulaire de changement de nom.
#[derive(Debug, Default, Deserialize)]
struct Renommage {
    #[serde(default)]
    nom: String,
}

impl PageRenommer {
    fn nouvelle(
        ctx: Contexte,
        document: Document,
        nom: Option<String>,
        erreur: Option<String>,
    ) -> Self {
        let (racine, extension) = documents::separer_extension(&document.nom_original);
        let (racine, extension) = (racine.to_string(), extension.to_string());
        PageRenommer {
            ctx,
            nom: nom.unwrap_or(racine),
            extension,
            document,
            erreur,
        }
    }
}

/// Le menu qu'on laisse allumé : celui de la fiche d'où vient le document.
fn menu_du_document(document: &Document) -> &'static str {
    if document.vehicule_id.is_some() {
        "vehicules"
    } else {
        "conducteurs"
    }
}

async fn page_renommer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let document = documents::obtenir(&etat.pool, id).await?;
    let ctx = admin.contexte(menu_du_document(&document));
    rendre(PageRenommer::nouvelle(ctx, document, None, None))
}

/// Enregistre le nouveau nom et revient à la page du document. Un nom refusé (vide, trop long)
/// ré-affiche le formulaire avec le motif et ce qui a été tapé.
async fn renommer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<Renommage>,
) -> Resultat<Response> {
    match documents::renommer(&etat.pool, id, &saisie.nom).await {
        Ok(document) => Ok(fait::rediriger(
            &format!("/documents/{}", document.id),
            Fait::DocumentRenomme,
        )),
        Err(ErreurApp::Metier(message)) => {
            let document = documents::obtenir(&etat.pool, id).await?;
            let ctx = admin.contexte(menu_du_document(&document));
            Ok(rendre(PageRenommer::nouvelle(
                ctx,
                document,
                Some(saisie.nom),
                Some(message),
            ))?
            .into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn supprimer(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let document = documents::supprimer(&etat.pool, &etat.config.documents_dir, id).await?;
    let retour = match (document.vehicule_id, document.conducteur_id) {
        (Some(id), _) => page_de_retour(Rattachement::Vehicule(id)),
        (_, Some(id)) => page_de_retour(Rattachement::Conducteur(id)),
        _ => "/".to_string(),
    };
    Ok(fait::rediriger(&retour, Fait::DocumentSupprime))
}

/// Choisit la photo de la fiche du véhicule et de sa carte d'accueil (#66). Retour sur le bloc
/// « Photos » de la fiche, où l'on voit le choix.
async fn mettre_en_tete(
    _admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let document = documents::mettre_en_tete(&etat.pool, id).await?;
    let vehicule_id = document.vehicule_id.ok_or(ErreurApp::Introuvable)?;
    Ok(fait::rediriger(
        &format!("/vehicules/{vehicule_id}#photos"),
        Fait::PhotoMiseEnTete,
    ))
}

/// La fiche, à la hauteur de sa rubrique « Documents » (`#documents`) : après un dépôt ou une
/// suppression, on revoit la liste sans faire défiler la page.
fn page_de_retour(rattachement: Rattachement) -> String {
    match rattachement {
        Rattachement::Vehicule(id) => format!("/vehicules/{id}#documents"),
        Rattachement::Conducteur(id) => format!("/conducteurs/{id}#documents"),
    }
}
