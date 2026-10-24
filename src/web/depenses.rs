//! Pages des dépenses : les pleins de carburant et les tickets de péage (#126).

use askama::Template;
use axum::extract::{FromRequest, Multipart, Query, Request, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};

use crate::commun::format::date_fr;
use crate::commun::types::{CarburantPlein, Choix, TypeDepense};
use crate::donnees::depenses::Depense;
use crate::donnees::documents::Document;
use crate::donnees::vehicules::Vehicule;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::consommation::{self, Montants, Periode};
use crate::metier::depenses::{self, Filtre, SaisieDepense, SaisieFiltre, Ticket};
use crate::metier::documents::SaisieDocument;
use crate::metier::vehicules;
use crate::web::chemin::Identifiant;
use crate::web::documents::refus_d_envoi;
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

/// Un lien qui règle la période du filtre d'un clic : « Ce mois », « Cette année »…
struct Raccourci {
    libelle: &'static str,
    adresse: String,
    /// Le filtre actuel est déjà celui-là.
    actif: bool,
}

#[derive(Template)]
#[template(path = "depenses/liste.html")]
struct PageListe {
    ctx: Contexte,
    depenses: Vec<Depense>,
    /// Le véhicule sur lequel la liste est restreinte, s'il y en a un.
    vehicule: Option<Vehicule>,
    /// Les véhicules du filtre.
    vehicules: Vec<Vehicule>,
    /// Ce que le formulaire du filtre montre : la saisie, telle qu'elle est arrivée.
    saisie: SaisieFiltre,
    /// Le motif d'un filtre illisible : la liste est alors celle de tout.
    erreur: Option<String>,
    /// Un véhicule ou une période restreint la liste.
    filtre_actif: bool,
    /// « du 01/10/2026 au 31/10/2026 », « depuis le … », « jusqu'au … » ; vide sans période.
    periode_fr: String,
    /// Ce qui est affiché, carburant et péages séparés (#126, bloc 3).
    totaux: Montants,
    raccourcis: Vec<Raccourci>,
    /// Ce qui suit `/depenses.csv` pour exporter ce qui est affiché : même véhicule, même période
    /// (`?vehicule_id=3&du=…&au=…`), vide sans filtre.
    requete_csv: String,
}

/// `/depenses?vehicule_id=3&du=2026-10-01&au=2026-10-31` : un véhicule et une période, chacun
/// facultatif. Les valeurs sont des identifiants et des dates : rien à échapper.
fn adresse(base: &str, vehicule_id: Option<i64>, periode: Option<Periode>) -> String {
    let mut parametres: Vec<String> = Vec::new();
    if let Some(id) = vehicule_id {
        parametres.push(format!("vehicule_id={id}"));
    }
    if let Some((du, au)) = periode {
        parametres.push(format!("du={du}"));
        parametres.push(format!("au={au}"));
    }
    if parametres.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", parametres.join("&"))
    }
}

/// La période du filtre, en une phrase ; vide sans période.
fn decrire_la_periode(filtre: &Filtre) -> String {
    match (filtre.du, filtre.au) {
        (Some(du), Some(au)) => format!("du {} au {}", date_fr(du), date_fr(au)),
        (Some(du), None) => format!("depuis le {}", date_fr(du)),
        (None, Some(au)) => format!("jusqu'au {}", date_fr(au)),
        (None, None) => String::new(),
    }
}

/// Une option de la liste « Carburant » du formulaire, avec l'unité de sa quantité pour que
/// `app.js` la change quand on choisit.
struct OptionCarburant {
    code: &'static str,
    libelle: &'static str,
    unite: &'static str,
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
    carburants: Vec<OptionCarburant>,
    /// L'unité de la quantité pour le carburant déjà choisi (« L », « kWh », « kg »).
    unite: &'static str,
    /// Les documents déjà joints à la dépense (modification) : son ticket, le plus souvent.
    tickets: Vec<Document>,
    /// Le formulaire revient après un refus alors qu'un ticket avait été joint : le navigateur
    /// ne le garde pas, il faut le dire.
    ticket_a_rechoisir: bool,
}

/// Construit le formulaire. Un plein dont la saisie ne dit pas le carburant reçoit celui que
/// propose le véhicule choisi (gazole pour un diesel…) : c'est ce que « Noter un plein » affiche.
async fn formulaire(
    etat: &Etat,
    ctx: Contexte,
    id: Option<i64>,
    mut saisie: SaisieDepense,
    erreur: Option<String>,
) -> Resultat<PageFormulaire> {
    let vehicules = vehicules::lister(&etat.pool).await?;
    if saisie.carburant.is_empty() {
        if let Some(v) = vehicules
            .iter()
            .find(|v| v.id.to_string() == saisie.vehicule_id)
        {
            saisie.carburant = v.carburant_de_plein().to_string();
        }
    }
    let unite = CarburantPlein::depuis_code(&saisie.carburant)
        .map(CarburantPlein::unite)
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
        carburants: CarburantPlein::TOUS
            .iter()
            .map(|c| OptionCarburant {
                code: c.code(),
                libelle: c.libelle(),
                unite: c.unite(),
            })
            .collect(),
        unite,
        tickets: Vec::new(),
        ticket_a_rechoisir: false,
    })
}

/// Ce que le formulaire envoie : la saisie, et, si un a été joint, le ticket (#126, bloc 2).
#[derive(Default)]
struct Envoi {
    saisie: SaisieDepense,
    ticket: Option<SaisieDocument>,
}

impl Envoi {
    fn ticket<'a>(&'a self, etat: &'a Etat) -> Ticket<'a> {
        self.ticket
            .as_ref()
            .map(|t| (etat.config.documents_dir.as_path(), t))
    }
}

/// Pose un champ du formulaire dans la saisie. Un nom inconnu est ignoré, comme le ferait
/// `Form<SaisieDepense>`.
fn poser(saisie: &mut SaisieDepense, nom: &str, valeur: String) {
    match nom {
        "vehicule_id" => saisie.vehicule_id = valeur,
        "type_depense" => saisie.type_depense = valeur,
        "date_depense" => saisie.date_depense = valeur,
        "montant" => saisie.montant = valeur,
        "carburant" => saisie.carburant = valeur,
        "quantite" => saisie.quantite = valeur,
        "kilometrage" => saisie.kilometrage = valeur,
        "plein_complet" => saisie.plein_complet = valeur,
        "libelle" => saisie.libelle = valeur,
        "notes" => saisie.notes = valeur,
        _ => {}
    }
}

/// Lit le formulaire. Il arrive en `multipart/form-data` depuis la page (le champ ticket
/// l'impose) ; l'encodage `application/x-www-form-urlencoded` reste accepté, sans ticket, pour
/// qui envoie le formulaire autrement.
async fn recevoir(requete: Request) -> Resultat<Envoi> {
    let multipart = requete
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("multipart/form-data"));
    if !multipart {
        let Form(saisie) = Form::<SaisieDepense>::from_request(requete, &())
            .await
            .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
        return Ok(Envoi {
            saisie,
            ticket: None,
        });
    }
    let mut multipart = Multipart::from_request(requete, &())
        .await
        .map_err(|rejet| ErreurApp::metier(rejet.body_text()))?;
    let mut envoi = Envoi::default();
    while let Some(champ) = multipart.next_field().await.map_err(refus_d_envoi)? {
        let nom_du_champ = champ.name().unwrap_or("").to_string();
        if nom_du_champ == "ticket" {
            let nom = champ.file_name().unwrap_or("").to_string();
            let type_mime = champ.content_type().unwrap_or("").to_string();
            let octets = champ.bytes().await.map_err(refus_d_envoi)?;
            // Sans ticket joint, le navigateur envoie quand même le champ, vide et sans nom.
            // Le type est posé par la couche métier : toujours celui du ticket.
            if !nom.is_empty() {
                envoi.ticket = Some(SaisieDocument {
                    type_document: String::new(),
                    nom_original: nom,
                    type_mime,
                    contenu: octets.to_vec(),
                });
            }
            continue;
        }
        let valeur = champ.text().await.map_err(refus_d_envoi)?;
        poser(&mut envoi.saisie, &nom_du_champ, valeur);
    }
    Ok(envoi)
}

/// La liste, filtrée par véhicule (`?vehicule_id=3`, le lien « Toutes les dépenses » de la fiche)
/// et par période (`?du=2026-10-01&au=2026-10-31`). Un filtre illisible ne casse pas la page : elle
/// montre tout, avec le motif.
async fn liste(
    utilisateur: UtilisateurConnecte,
    State(etat): State<Etat>,
    Query(saisie): Query<SaisieFiltre>,
) -> Resultat<Html<String>> {
    let ctx = utilisateur.contexte("depenses");
    let (filtre, erreur) = match saisie.valider() {
        Ok(filtre) => (filtre, None),
        Err(ErreurApp::Metier(motif)) => (Filtre::default(), Some(motif)),
        Err(autre) => return Err(autre),
    };
    let vehicule = match filtre.vehicule_id {
        Some(id) => Some(vehicules::obtenir(&etat.pool, id).await?),
        None => None,
    };
    let depenses = depenses::lister_filtre(&etat.pool, &filtre).await?;
    let aujourd_hui = crate::commun::dates::aujourd_hui();
    let periodes = [
        ("Ce mois", consommation::mois_en_cours(aujourd_hui)),
        ("Cette année", consommation::annee_en_cours(aujourd_hui)),
        (
            "Douze derniers mois",
            consommation::douze_derniers_mois(aujourd_hui),
        ),
    ];
    let mut raccourcis: Vec<Raccourci> = periodes
        .iter()
        .map(|&(libelle, periode)| Raccourci {
            libelle,
            adresse: adresse("/depenses", filtre.vehicule_id, Some(periode)),
            actif: (filtre.du, filtre.au) == (Some(periode.0), Some(periode.1)),
        })
        .collect();
    raccourcis.push(Raccourci {
        libelle: "Tout",
        adresse: adresse("/depenses", filtre.vehicule_id, None),
        actif: filtre.du.is_none() && filtre.au.is_none(),
    });
    let periode = filtre.du.zip(filtre.au);
    rendre(PageListe {
        ctx,
        totaux: Montants::de(&depenses),
        depenses,
        vehicule,
        vehicules: vehicules::lister(&etat.pool).await?,
        saisie,
        erreur,
        filtre_actif: !filtre.est_vide(),
        periode_fr: decrire_la_periode(&filtre),
        raccourcis,
        requete_csv: requete_csv(&filtre, periode),
    })
}

/// La requête de l'export du filtre, `?…` ou vide : les dates sont celles saisies, même une seule.
fn requete_csv(filtre: &Filtre, periode: Option<Periode>) -> String {
    let mut parametres: Vec<String> = Vec::new();
    if let Some(id) = filtre.vehicule_id {
        parametres.push(format!("vehicule_id={id}"));
    }
    match (periode, filtre.du, filtre.au) {
        (Some((du, au)), _, _) => {
            parametres.push(format!("du={du}"));
            parametres.push(format!("au={au}"));
        }
        (None, Some(du), _) => parametres.push(format!("du={du}")),
        (None, None, Some(au)) => parametres.push(format!("au={au}")),
        (None, None, None) => {}
    }
    if parametres.is_empty() {
        String::new()
    } else {
        format!("?{}", parametres.join("&"))
    }
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
        saisie.date_depense = crate::commun::dates::aujourd_hui().to_string();
    }
    if saisie.type_depense.is_empty() {
        saisie.type_depense = TypeDepense::Carburant.code().to_string();
    }
    if saisie.plein_complet.is_empty() {
        saisie.plein_complet = "oui".to_string();
    }
    rendre(formulaire(&etat, admin.contexte("depenses"), None, saisie, None).await?)
}

/// Le message dit ce qui a été noté : un plein, ou un péage, avec ou sans son ticket.
fn fait_enregistre(depense: &Depense, avec_ticket: bool) -> Fait {
    match (depense.est_un_plein(), avec_ticket) {
        (true, false) => Fait::PleinEnregistre,
        (true, true) => Fait::PleinEnregistreAvecTicket,
        (false, false) => Fait::PeageEnregistre,
        (false, true) => Fait::PeageEnregistreAvecTicket,
    }
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    requete: Request,
) -> Resultat<Response> {
    let (envoi, resultat) = match recevoir(requete).await {
        Ok(envoi) => {
            let resultat =
                depenses::creer_avec_ticket(&etat.pool, &envoi.saisie, envoi.ticket(&etat)).await;
            (envoi, resultat)
        }
        Err(erreur) => (Envoi::default(), Err(erreur)),
    };
    match resultat {
        Ok(d) => Ok(fait::rediriger(
            &format!("/vehicules/{}", d.vehicule_id),
            fait_enregistre(&d, envoi.ticket.is_some()),
        )),
        Err(ErreurApp::Metier(message)) => {
            let mut page = formulaire(
                &etat,
                admin.contexte("depenses"),
                None,
                envoi.saisie,
                Some(message),
            )
            .await?;
            page.ticket_a_rechoisir = envoi.ticket.is_some();
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
    let depense = depenses::obtenir(&etat.pool, id).await?;
    let mut page = formulaire(
        &etat,
        admin.contexte("depenses"),
        Some(id),
        SaisieDepense::depuis(&depense),
        None,
    )
    .await?;
    page.tickets = depense.documents;
    rendre(page)
}

async fn modifier(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    requete: Request,
) -> Resultat<Response> {
    let (envoi, resultat) = match recevoir(requete).await {
        Ok(envoi) => {
            let resultat =
                depenses::modifier_avec_ticket(&etat.pool, id, &envoi.saisie, envoi.ticket(&etat))
                    .await;
            (envoi, resultat)
        }
        Err(erreur) => (Envoi::default(), Err(erreur)),
    };
    match resultat {
        Ok(d) => Ok(fait::rediriger(
            &format!("/vehicules/{}", d.vehicule_id),
            if envoi.ticket.is_some() {
                Fait::DepenseModifieeAvecTicket
            } else {
                Fait::DepenseModifiee
            },
        )),
        Err(ErreurApp::Metier(message)) => {
            let mut page = formulaire(
                &etat,
                admin.contexte("depenses"),
                Some(id),
                envoi.saisie,
                Some(message),
            )
            .await?;
            page.tickets = depenses::obtenir(&etat.pool, id).await?.documents;
            page.ticket_a_rechoisir = envoi.ticket.is_some();
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
    depenses::supprimer(&etat.pool, id).await?;
    Ok(fait::rediriger("/depenses", Fait::DepenseSupprimee))
}
