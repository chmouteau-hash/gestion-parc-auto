//! Pages des interventions d'entretien.

use askama::Template;
use axum::extract::{FromRequest, Multipart, Query, Request, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::types::{Choix, Operation, ResultatControle, TypeDocument, TypeIntervention};
use crate::donnees::documents::Document;
use crate::donnees::interventions::Intervention;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::documents::SaisieDocument;
use crate::metier::interventions::{self, Enregistree, Facture, SaisieIntervention, SaisieLigne};
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::documents::refus_d_envoi;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::{Administrateur, UtilisateurConnecte};

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/interventions", get(liste).post(creer))
        .route("/interventions/nouvelle", get(nouvelle))
        .route("/interventions/{id}", post(modifier))
        .route("/interventions/{id}/modifier", get(editer))
        .route("/interventions/{id}/supprimer", post(supprimer))
}

#[derive(Template)]
#[template(path = "interventions/liste.html")]
struct PageListe {
    ctx: Contexte,
    interventions: Vec<Intervention>,
}

#[derive(Template)]
#[template(path = "interventions/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    titre: String,
    action: String,
    saisie: SaisieIntervention,
    erreur: Option<String>,
    vehicules: Vec<Vehicule>,
    types: Vec<Choix>,
    /// Les résultats possibles d'un contrôle technique ou d'une contre-visite (#107).
    resultats: Vec<Choix>,
    /// Les cases « Ce qui a été fait », en deux groupes (pièces et consommables, puis contrôles
    /// et réglages), chacun dans l'ordre du vocabulaire.
    groupes_operations: [(&'static str, Vec<Choix>); 2],
    /// Les documents déjà joints à l'intervention (modification) : la facture, le plus souvent.
    factures: Vec<Document>,
    /// Ce que peut être le document joint, et le choix affiché (#104).
    types_document: Vec<Choix>,
    type_document: String,
    /// Le formulaire revient après un refus alors qu'une facture avait été jointe : le
    /// navigateur ne la garde pas, il faut le dire.
    facture_a_rechoisir: bool,
    /// Le kilométrage contredit une autre intervention du véhicule (règle (a), #94, bloc 2b) :
    /// le formulaire revient avec cette phrase et « Enregistrer quand même ».
    avertissement: Option<String>,
    /// Le kilométrage vient de la fiche (le compteur du jour) : `app.js` l'efface si la date
    /// passe dans le passé.
    kilometrage_prerempli: bool,
    /// La création, pas la modification : la page propose alors de noter une chose à faire.
    nouvelle: bool,
    /// Où « Annuler » ramène : la liste « Entretien », ou la page d'où vient « C'est fait ».
    annuler: String,
}

async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    saisie: SaisieIntervention,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let type_document = interventions::type_de_document_par_defaut(&saisie.type_intervention)
        .code()
        .to_string();
    Ok(PageFormulaire {
        ctx,
        titre: match id {
            None => "Nouvelle intervention".to_string(),
            Some(_) => "Modifier l'intervention".to_string(),
        },
        action: match id {
            None => "/interventions".to_string(),
            Some(id) => format!("/interventions/{id}"),
        },
        saisie,
        erreur,
        vehicules: vehicules::lister(&etat.pool).await?,
        types: TypeIntervention::choix(),
        resultats: ResultatControle::choix(),
        groupes_operations: [
            ("Pièces et consommables", Operation::pieces()),
            ("Contrôles et réglages", Operation::controles()),
        ],
        factures: Vec::new(),
        types_document: interventions::types_de_document(),
        type_document,
        facture_a_rechoisir: false,
        avertissement: None,
        kilometrage_prerempli: false,
        nouvelle: id.is_none(),
        annuler: "/interventions".to_string(),
    })
}

/// Ce que le formulaire envoie : les paires `nom=valeur` (les cases « Ce qui a été fait » se
/// répètent sous le même nom) et, si une a été jointe, la facture (#100).
/// `?kilometrage_confirme=1` : l'adresse du bouton « Enregistrer quand même », que le formulaire
/// propose après l'avertissement de la règle (a). Sans elle, un kilométrage qui contredit une autre
/// intervention suspend l'enregistrement pour avertir. L'API JSON ne passe pas par ici.
#[derive(Debug, Default, serde::Deserialize)]
struct Confirmation {
    #[serde(default)]
    kilometrage_confirme: String,
}

impl Confirmation {
    fn donnee(&self) -> bool {
        self.kilometrage_confirme == "1"
    }
}

impl PageFormulaire {
    /// L'adresse de « Enregistrer quand même » : celle d'envoi, plus la confirmation.
    fn action_confirmee(&self) -> String {
        let separateur = if self.action.contains('?') { '&' } else { '?' };
        format!("{}{separateur}kilometrage_confirme=1", self.action)
    }

    /// Le formulaire suspendu par l'avertissement : la saisie revient telle quelle ; une facture
    /// jointe est à choisir de nouveau, le navigateur ne la garde pas.
    fn avertir(&mut self, phrase: String, envoi: &Envoi) {
        self.avertissement = Some(phrase);
        self.facture_a_rechoisir = envoi.facture.is_some();
        if !envoi.type_document().is_empty() {
            self.type_document = envoi.type_document().to_string();
        }
    }
}

/// D'où vient le clic sur « C'est fait » (#94, bloc 3), et donc où ramène « Annuler » : la fiche
/// (`vehicule`), la page Alertes (`alertes`) ou l'accueil (`accueil`) ; absent ou autre, la liste
/// « Entretien ». Comme pour les échéances, l'adresse ne porte qu'un mot, jamais une destination :
/// un lien forgé ne peut pas faire rediriger vers un autre site.
#[derive(Debug, Default, serde::Deserialize)]
struct Origine {
    #[serde(default)]
    retour: String,
}

impl Origine {
    fn mot_cle(&self) -> &'static str {
        match self.retour.as_str() {
            "vehicule" => "vehicule",
            "alertes" => "alertes",
            "accueil" => "accueil",
            _ => "liste",
        }
    }

    /// Où « Annuler » ramène.
    fn page(&self, vehicule_id: Option<i64>) -> String {
        match (self.mot_cle(), vehicule_id) {
            ("vehicule", Some(id)) => format!("/vehicules/{id}#plan"),
            ("alertes", _) => "/alertes".to_string(),
            ("accueil", _) => "/".to_string(),
            _ => "/interventions".to_string(),
        }
    }

    /// L'adresse d'envoi du formulaire de création. Elle garde le mot-clé : un refus ou un
    /// avertissement ré-affiche le formulaire avec le même « Annuler ».
    fn action_creation(&self) -> String {
        match self.mot_cle() {
            "liste" => "/interventions".to_string(),
            mot => format!("/interventions?retour={mot}"),
        }
    }
}

/// L'opération à cocher d'office, donnée par l'adresse du formulaire vierge (« C'est fait »).
#[derive(Debug, Default, serde::Deserialize)]
struct Prechoix {
    #[serde(default)]
    operation: String,
}

#[derive(Default)]
struct Envoi {
    paires: Vec<(String, String)>,
    facture: Option<SaisieDocument>,
}

impl Envoi {
    fn facture<'a>(&'a self, etat: &'a Etat) -> Facture<'a> {
        self.facture
            .as_ref()
            .map(|f| (etat.config.documents_dir.as_path(), f))
    }

    /// Le type choisi dans la liste « Ce document est… » (champ `type_document`) ; vide quand
    /// le formulaire ne l'a pas envoyé.
    fn type_document(&self) -> &str {
        self.paires
            .iter()
            .find(|(n, _)| n == "type_document")
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    /// Donne au document joint le type choisi, ou celui proposé d'office selon l'intervention
    /// (#104). Sans document, le choix n'a pas d'objet.
    fn typer_le_document(&mut self, saisie: &SaisieIntervention) -> Resultat<()> {
        let type_document =
            interventions::type_de_document(self.type_document(), &saisie.type_intervention)?;
        if let Some(f) = &mut self.facture {
            f.type_document = type_document.code().to_string();
        }
        Ok(())
    }
}

/// Lit le formulaire. Il arrive en `multipart/form-data` depuis la page (le champ facture
/// l'impose) ; l'encodage `application/x-www-form-urlencoded` reste accepté, sans facture, pour
/// qui envoie le formulaire autrement.
async fn recevoir(requete: Request) -> Resultat<Envoi> {
    let multipart = requete
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("multipart/form-data"));
    if !multipart {
        let Form(paires) = Form::<Vec<(String, String)>>::from_request(requete, &())
            .await
            .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
        return Ok(Envoi {
            paires,
            facture: None,
        });
    }
    let mut multipart = Multipart::from_request(requete, &())
        .await
        .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
    let mut envoi = Envoi::default();
    while let Some(champ) = multipart.next_field().await.map_err(refus_d_envoi)? {
        let nom_du_champ = champ.name().unwrap_or("").to_string();
        if nom_du_champ == "facture" {
            let nom = champ.file_name().unwrap_or("").to_string();
            let type_mime = champ.content_type().unwrap_or("").to_string();
            let octets = champ.bytes().await.map_err(refus_d_envoi)?;
            // Sans document joint, le navigateur envoie quand même le champ, vide et sans nom.
            // Le type (facture, contrôle technique…) est posé ensuite par `typer_le_document`.
            if !nom.is_empty() {
                envoi.facture = Some(SaisieDocument {
                    type_document: TypeDocument::Facture.code().to_string(),
                    nom_original: nom,
                    type_mime,
                    contenu: octets.to_vec(),
                });
            }
            continue;
        }
        let valeur = champ.text().await.map_err(refus_d_envoi)?;
        envoi.paires.push((nom_du_champ, valeur));
    }
    Ok(envoi)
}

async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
) -> Resultat<Html<String>> {
    rendre(PageListe {
        ctx: utilisateur.contexte("interventions"),
        interventions: interventions::lister(&etat.pool).await?,
    })
}

/// Le formulaire vierge accepte des valeurs dans l'adresse : la fiche y met le véhicule et son
/// compteur ; « C'est fait » sur une opération du plan (#94, bloc 3) y ajoute le type,
/// l'opération à cocher et `retour`, où « Annuler » ramène.
async fn nouvelle(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(mut saisie): Query<SaisieIntervention>,
    Query(prechoix): Query<Prechoix>,
    Query(origine): Query<Origine>,
) -> Resultat<Html<String>> {
    if saisie.date_intervention.is_empty() {
        saisie.date_intervention = chrono::Local::now().date_naive().to_string();
    }
    if Operation::depuis_code(&prechoix.operation).is_some() {
        saisie.lignes.push(SaisieLigne {
            operation: prechoix.operation,
            ..SaisieLigne::default()
        });
    }
    let prerempli = !saisie.kilometrage.trim().is_empty();
    let annuler = origine.page(saisie.vehicule_id.trim().parse().ok());
    let mut page = formulaire(&etat, admin.contexte("interventions"), None, saisie, None).await?;
    page.kilometrage_prerempli = prerempli;
    page.action = origine.action_creation();
    page.annuler = annuler;
    rendre(page)
}

/// Où mène une intervention enregistrée : à la fiche du véhicule, avec un message qui dit la
/// contre-visite créée ou le contrôle à refaire (#107). Si elle a soldé une échéance (#94, bloc
/// 3), au formulaire de la suivante, pré-rempli et daté de la visite, comme après « Marquer
/// traitée » ; « Annuler » y ramène à la fiche.
fn apres_creation(enregistree: &Enregistree, avec_facture: bool) -> Response {
    let fiche = format!("/vehicules/{}", enregistree.intervention.vehicule_id);
    let Some(solde) = &enregistree.solde else {
        return fait::rediriger(
            &fiche,
            match (enregistree.contre_visite, avec_facture) {
                (Some(suite), _) => Fait::contre_visite(suite, avec_facture),
                (None, true) => Fait::InterventionCreeeAvecFacture,
                (None, false) => Fait::InterventionCreee,
            },
        );
    };
    let vers = match &solde.suivante {
        Some(s) => format!(
            "/echeances/nouvelle?vehicule_id={}&type_echeance={}&date_limite={}&retour=vehicule",
            s.vehicule_id, s.type_echeance, s.date_limite
        ),
        None => fiche,
    };
    fait::rediriger(
        &vers,
        Fait::intervention_soldant(solde.type_echeance, avec_facture),
    )
}

/// Le formulaire arrive en paires `nom=valeur` (et non en `SaisieIntervention`) : les cases
/// « Ce qui a été fait » se répètent sous le même nom, ce qu'un formulaire HTML ne sait pas
/// envoyer autrement. `SaisieIntervention::depuis_formulaire` les décode.
async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Query(confirmation): Query<Confirmation>,
    Query(origine): Query<Origine>,
    requete: Request,
) -> Resultat<Response> {
    let (envoi, resultat) = match recevoir(requete).await {
        Ok(mut envoi) => {
            let saisie = SaisieIntervention::depuis_formulaire(&envoi.paires);
            if !confirmation.donnee() {
                if let Some(phrase) =
                    interventions::kilometrage_a_confirmer_pour(&etat.pool, &saisie, None).await?
                {
                    let annuler = origine.page(saisie.vehicule_id.trim().parse().ok());
                    let mut page =
                        formulaire(&etat, admin.contexte("interventions"), None, saisie, None)
                            .await?;
                    page.avertir(phrase, &envoi);
                    page.action = origine.action_creation();
                    page.annuler = annuler;
                    return Ok(rendre(page)?.into_response());
                }
            }
            let resultat = match envoi.typer_le_document(&saisie) {
                Ok(()) => {
                    interventions::creer_avec_facture(
                        &etat.pool,
                        &saisie,
                        envoi.facture(&etat),
                        chrono::Local::now().date_naive(),
                    )
                    .await
                }
                Err(erreur) => Err(erreur),
            };
            (envoi, resultat)
        }
        Err(erreur) => (Envoi::default(), Err(erreur)),
    };
    match resultat {
        Ok(enregistree) => Ok(apres_creation(&enregistree, envoi.facture.is_some())),
        Err(ErreurApp::Metier(message)) => {
            let mut page = formulaire(
                &etat,
                admin.contexte("interventions"),
                None,
                SaisieIntervention::depuis_formulaire(&envoi.paires),
                Some(message),
            )
            .await?;
            page.action = origine.action_creation();
            page.annuler = origine.page(page.saisie.vehicule_id.trim().parse().ok());
            page.facture_a_rechoisir = envoi.facture.is_some();
            if !envoi.type_document().is_empty() {
                page.type_document = envoi.type_document().to_string();
            }
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
    let intervention = interventions::obtenir(&etat.pool, id).await?;
    let mut page = formulaire(
        &etat,
        admin.contexte("interventions"),
        Some(id),
        SaisieIntervention::depuis(&intervention),
        None,
    )
    .await?;
    page.factures = intervention.documents;
    rendre(page)
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Query(confirmation): Query<Confirmation>,
    requete: Request,
) -> Resultat<Response> {
    let (envoi, resultat) = match recevoir(requete).await {
        Ok(mut envoi) => {
            let saisie = SaisieIntervention::depuis_formulaire(&envoi.paires);
            if !confirmation.donnee() {
                if let Some(phrase) =
                    interventions::kilometrage_a_confirmer_pour(&etat.pool, &saisie, Some(id))
                        .await?
                {
                    let mut page = formulaire(
                        &etat,
                        admin.contexte("interventions"),
                        Some(id),
                        saisie,
                        None,
                    )
                    .await?;
                    page.factures = interventions::obtenir(&etat.pool, id).await?.documents;
                    page.avertir(phrase, &envoi);
                    return Ok(rendre(page)?.into_response());
                }
            }
            let resultat = match envoi.typer_le_document(&saisie) {
                Ok(()) => {
                    interventions::modifier_avec_facture(
                        &etat.pool,
                        id,
                        &saisie,
                        envoi.facture(&etat),
                    )
                    .await
                }
                Err(erreur) => Err(erreur),
            };
            (envoi, resultat)
        }
        Err(erreur) => (Envoi::default(), Err(erreur)),
    };
    match resultat {
        Ok(i) => Ok(fait::rediriger(
            &format!("/vehicules/{}", i.vehicule_id),
            if envoi.facture.is_some() {
                Fait::InterventionModifieeAvecFacture
            } else {
                Fait::InterventionModifiee
            },
        )),
        Err(ErreurApp::Metier(message)) => {
            let mut page = formulaire(
                &etat,
                admin.contexte("interventions"),
                Some(id),
                SaisieIntervention::depuis_formulaire(&envoi.paires),
                Some(message),
            )
            .await?;
            page.factures = interventions::obtenir(&etat.pool, id).await?.documents;
            page.facture_a_rechoisir = envoi.facture.is_some();
            if !envoi.type_document().is_empty() {
                page.type_document = envoi.type_document().to_string();
            }
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
    interventions::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger(
        "/interventions",
        Fait::InterventionSupprimee,
    ))
}
