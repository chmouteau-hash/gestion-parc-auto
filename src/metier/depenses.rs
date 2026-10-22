//! Les dépenses pour rouler (#126) : validation et opérations.
//!
//! Un seul écran pour les pleins de carburant et les tickets de péage, distingués par un type
//! d'un vocabulaire fermé ([`TypeDepense`]). Seul un plein porte le carburant pris ce jour-là
//! ([`CarburantPlein`] : SP95, SP98, E85, gazole, GPL, GNV, électricité…, #128), une quantité,
//! le compteur et la case « plein complet ». Le carburant est obligatoire sur un plein et refusé
//! sur un péage ; la quantité et le kilométrage sont refusés sur un péage quand ils sont
//! renseignés, parce qu'un formulaire sans script les envoie quand même (vides) ; la case, qui a
//! une valeur d'office, est ignorée.
//!
//! Le carburant d'un plein est **proposé par le formulaire d'après celui du véhicule** (gazole
//! pour un diesel, électricité pour un électrique…), à la création seulement : une modification
//! garde ce qui est noté. C'est lui, pas le véhicule, qui fixe l'unité de la quantité : un hybride
//! rechargeable fait un plein d'essence en litres et une recharge en kWh.
//!
//! Un plein noté avec son kilométrage **relève le compteur du véhicule** s'il est plus haut, comme
//! une intervention ; un plein ancien ne le fait pas reculer. Le kilométrage est facultatif : sans
//! lui, pas de consommation pour ce plein, mais on ne bloque pas qui n'a pas regardé le compteur.
//!
//! Une dépense peut aussi joindre **son ticket** (#126, bloc 2) : un document du véhicule, relié à
//! la dépense, déposé par le même formulaire, comme la facture d'une intervention (#100). Le ticket
//! est contrôlé avant d'écrire quoi que ce soit, et reste quand la dépense est supprimée.
//!
//! Ce que le véhicule **coûte et consomme** (#126, bloc 3 ; l'entretien s'y ajoute avec #94,
//! bloc 6) se calcule dans [`crate::metier::consommation`] et se pose sur chaque plein à la lecture
//! ([`Depense::consommation`]) : rien n'est stocké. La liste se filtre par véhicule et par période
//! ([`Filtre`]) après ce calcul, parce qu'une consommation dépend du plein précédent, qui peut
//! être hors de la période.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::quantite_fr;
use crate::commun::types::{CarburantPlein, TypeDepense, TypeDocument};
use crate::donnees::depenses::{self, Depense, NouvelleDepense};
use crate::donnees::documents as donnees_documents;
use crate::donnees::interventions as donnees_interventions;
use crate::donnees::vehicules;
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::consommation::{self, Entretien, Resume};
use crate::metier::documents::{self as metier_documents, Rattachement, SaisieDocument};
use crate::metier::validation::{
    case_a_cocher, centimes_en_saisie, code_parmi, date_facultative, date_obligatoire,
    entier_positif, identifiant, montant_en_centimes, notes, quantite_en_centiemes,
    texte_facultatif,
};

/// Ce que le formulaire ou l'API envoie : tout en chaînes, une seule validation.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieDepense {
    #[serde(default)]
    pub vehicule_id: String,
    /// Code du vocabulaire [`TypeDepense`].
    #[serde(default)]
    pub type_depense: String,
    #[serde(default)]
    pub date_depense: String,
    /// Montant en euros (`68,40`), converti en centimes à la validation ; vide = 0.
    #[serde(default)]
    pub montant: String,
    /// Code du vocabulaire [`CarburantPlein`] : obligatoire pour un plein, refusé pour un péage.
    #[serde(default)]
    pub carburant: String,
    /// Quantité en litres, en kWh ou en kg (`42,3`) ; vide = non notée. Carburant seulement.
    #[serde(default)]
    pub quantite: String,
    /// Le compteur au moment du plein ; vide = non relevé. Carburant seulement.
    #[serde(default)]
    pub kilometrage: String,
    /// La case : `oui`/`on`/`true`/`1` cochée, vide ou `non`/`false`/`0` décochée. Carburant
    /// seulement : ignorée pour un péage, parce que le formulaire la coche d'office.
    #[serde(default)]
    pub plein_complet: String,
    /// La station, ou le trajet.
    #[serde(default)]
    pub libelle: String,
    #[serde(default)]
    pub notes: String,
}

impl SaisieDepense {
    pub fn depuis(d: &Depense) -> SaisieDepense {
        SaisieDepense {
            vehicule_id: d.vehicule_id.to_string(),
            type_depense: d.type_depense.clone(),
            date_depense: d.date_depense.to_string(),
            montant: centimes_en_saisie(d.montant_centimes),
            carburant: d.carburant.clone(),
            quantite: if d.quantite_centiemes == 0 {
                String::new()
            } else {
                quantite_fr(d.quantite_centiemes)
            },
            kilometrage: d.kilometrage.map(|km| km.to_string()).unwrap_or_default(),
            plein_complet: if d.plein_complet {
                "oui".to_string()
            } else {
                String::new()
            },
            libelle: d.libelle.clone(),
            notes: d.notes.clone(),
        }
    }

    /// Pour le formulaire : la case « plein complet » est-elle cochée ? Une valeur illisible
    /// compte pour décochée ; la validation, elle, la refuse.
    pub fn plein_complet_coche(&self) -> bool {
        case_a_cocher(&self.plein_complet, "plein complet").unwrap_or(false)
    }
}

pub fn valider(s: &SaisieDepense) -> Resultat<NouvelleDepense> {
    let type_depense = code_parmi(&s.type_depense, "type", TypeDepense::depuis_code)?;
    let (carburant, quantite_centiemes, kilometrage, plein_complet) = match type_depense {
        TypeDepense::Carburant => {
            if s.carburant.trim().is_empty() {
                return Err(ErreurApp::metier(
                    "Indiquer le carburant du plein : SP95, SP98, E85, gazole, GPL, GNV, électricité…",
                ));
            }
            (
                Some(code_parmi(
                    &s.carburant,
                    "carburant",
                    CarburantPlein::depuis_code,
                )?),
                if s.quantite.trim().is_empty() {
                    0
                } else {
                    quantite_en_centiemes(&s.quantite, "quantité")?
                },
                if s.kilometrage.trim().is_empty() {
                    None
                } else {
                    Some(entier_positif(&s.kilometrage, "kilométrage")?)
                },
                case_a_cocher(&s.plein_complet, "plein complet")?,
            )
        }
        TypeDepense::Peage => {
            if !s.carburant.trim().is_empty() {
                return Err(ErreurApp::metier(
                    "Un péage n'a pas de carburant : c'est un plein qui en a un.",
                ));
            }
            if !s.quantite.trim().is_empty() || !s.kilometrage.trim().is_empty() {
                return Err(ErreurApp::metier(
                    "Un péage n'a ni quantité ni kilométrage : c'est un plein de carburant qui les porte.",
                ));
            }
            (None, 0, None, false)
        }
    };
    Ok(NouvelleDepense {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_depense,
        date_depense: date_obligatoire(&s.date_depense, "date")?,
        montant_centimes: montant_en_centimes(&s.montant, "montant")?,
        carburant,
        quantite_centiemes,
        kilometrage,
        plein_complet,
        libelle: texte_facultatif(&s.libelle, "station ou trajet")?,
        notes: notes(&s.notes, "notes")?,
    })
}

fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

/// Attache à chaque dépense ses documents (son ticket), lus en une requête pour toute la liste,
/// puis la consommation de chaque plein qui en a une. La liste doit être **entière** (un véhicule
/// ou tous) : la consommation d'un plein dépend du plein complet précédent.
async fn completer(pool: &PgPool, mut liste: Vec<Depense>) -> Resultat<Vec<Depense>> {
    if liste.is_empty() {
        return Ok(liste);
    }
    let ids: Vec<i64> = liste.iter().map(|d| d.id).collect();
    for document in donnees_documents::lister_par_depenses(pool, &ids).await? {
        if let Some(d) = liste.iter_mut().find(|d| Some(d.id) == document.depense_id) {
            d.documents.push(document);
        }
    }
    let calculees = consommation::consommations(&liste);
    for d in liste.iter_mut() {
        d.consommation = calculees.get(&d.id).copied();
    }
    Ok(liste)
}

/// Ce que la liste « Dépenses » (et son export, et l'API) montre : un véhicule, une période, ou
/// tout. Des chaînes, comme une saisie : une adresse de page ou un appel d'API arrive ainsi.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieFiltre {
    /// L'identifiant d'un véhicule ; vide = tous.
    #[serde(default)]
    pub vehicule_id: String,
    /// Premier jour compris, `AAAA-MM-JJ` ; vide = depuis le début.
    #[serde(default)]
    pub du: String,
    /// Dernier jour compris ; vide = jusqu'à la fin.
    #[serde(default)]
    pub au: String,
}

/// Le filtre validé.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filtre {
    pub vehicule_id: Option<i64>,
    pub du: Option<NaiveDate>,
    pub au: Option<NaiveDate>,
}

impl SaisieFiltre {
    pub fn valider(&self) -> Resultat<Filtre> {
        let vehicule_id = if self.vehicule_id.trim().is_empty() {
            None
        } else {
            Some(identifiant(&self.vehicule_id, "véhicule")?)
        };
        let du = date_facultative(&self.du, "du")?;
        let au = date_facultative(&self.au, "au")?;
        if let (Some(du), Some(au)) = (du, au) {
            if du > au {
                return Err(ErreurApp::metier(
                    "La période commence après sa fin : la date « du » doit précéder la date « au ».",
                ));
            }
        }
        Ok(Filtre {
            vehicule_id,
            du,
            au,
        })
    }
}

impl Filtre {
    /// Vrai quand rien ne restreint la liste.
    pub fn est_vide(&self) -> bool {
        *self == Filtre::default()
    }

    /// Une dépense passe-t-elle le filtre ?
    pub fn garde(&self, d: &Depense) -> bool {
        self.vehicule_id.is_none_or(|id| d.vehicule_id == id)
            && self.du.is_none_or(|du| d.date_depense >= du)
            && self.au.is_none_or(|au| d.date_depense <= au)
    }
}

/// Les dépenses qui passent le filtre, les plus récentes en premier. La consommation de chaque
/// plein est calculée sur **toutes** les dépenses du véhicule, avant le filtre : un plein du
/// début de la période a pour borne un plein d'avant.
pub async fn lister_filtre(pool: &PgPool, filtre: &Filtre) -> Resultat<Vec<Depense>> {
    let liste = match filtre.vehicule_id {
        Some(id) => lister_par_vehicule(pool, id).await?,
        None => lister(pool).await?,
    };
    Ok(liste.into_iter().filter(|d| filtre.garde(d)).collect())
}

/// Le résumé du coût d'un véhicule, comme le bloc « Coût » de sa fiche : l'API le rend tel quel.
/// Ses dépenses et ses interventions (avec le prix de leurs lignes) se lisent ici.
pub async fn resume_du_vehicule(
    pool: &PgPool,
    vehicule_id: i64,
    aujourd_hui: NaiveDate,
) -> Resultat<Resume> {
    vehicules::obtenir(pool, vehicule_id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    let depenses = lister_par_vehicule(pool, vehicule_id).await?;
    let interventions =
        crate::metier::interventions::lister_par_vehicule(pool, vehicule_id).await?;
    Ok(consommation::resume(
        &depenses,
        &entretiens(&interventions),
        aujourd_hui,
    ))
}

/// Les interventions vues par le calcul du coût : leur date et leur kilométrage (ce sont des
/// relevés, qui avec ceux des pleins bornent le coût au kilomètre), leur facture et la somme des
/// prix de leurs lignes. Les interventions doivent venir avec leurs lignes.
pub fn entretiens(interventions: &[donnees_interventions::Intervention]) -> Vec<Entretien> {
    interventions
        .iter()
        .map(|i| Entretien {
            id: i.id,
            date: i.date_intervention,
            kilometrage: i.kilometrage,
            cout_centimes: i.cout_centimes,
            pieces_centimes: i.lignes.iter().map(|l| l.prix_centimes).sum(),
        })
        .collect()
}

/// Le ticket joint à la saisie : le dossier des documents et le fichier reçu. Toujours du type
/// « Ticket de carburant ou de péage » : la couche web n'a pas à le choisir.
pub type Ticket<'a> = Option<(&'a Path, &'a SaisieDocument)>;

/// Le type d'un document joint à une dépense (#126, bloc 2).
pub const TYPE_DU_TICKET: TypeDocument = TypeDocument::Ticket;

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Depense>> {
    completer(pool, depenses::lister(pool).await?).await
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Depense>> {
    completer(
        pool,
        depenses::lister_par_vehicule(pool, vehicule_id).await?,
    )
    .await
}

/// Une dépense, avec son ticket et sa consommation : celle-ci dépend des autres pleins du véhicule,
/// d'où la lecture de toute sa liste.
pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Depense> {
    let depense = depenses::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    lister_par_vehicule(pool, depense.vehicule_id)
        .await?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or(ErreurApp::Introuvable)
}

/// Enregistre la dépense et, pour un plein avec kilométrage, relève le compteur du véhicule s'il
/// est plus bas. Tout est dans une même transaction : soit tout réussit, soit rien.
pub async fn creer(pool: &PgPool, saisie: &SaisieDepense) -> Resultat<Depense> {
    creer_avec_ticket(pool, saisie, None).await
}

/// [`creer`], puis dépose le ticket joint, s'il y en a un, comme document du véhicule relié à la
/// dépense (#126, bloc 2). Le ticket est contrôlé (fichier vide, trop gros) **avant** d'écrire la
/// dépense : un refus ne laisse rien derrière lui, et le formulaire revient avec sa saisie.
pub async fn creer_avec_ticket(
    pool: &PgPool,
    saisie: &SaisieDepense,
    ticket: Ticket<'_>,
) -> Resultat<Depense> {
    let validee = valider(saisie)?;
    let ticket_pret = preparer_le_ticket(ticket).await?;
    let mut transaction = pool.begin().await?;
    let id = depenses::inserer(&mut *transaction, &validee)
        .await
        .map_err(traduire)?;
    relever(&mut transaction, &validee).await?;
    transaction.commit().await?;
    joindre(pool, &validee, id, ticket, ticket_pret).await?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieDepense) -> Resultat<Depense> {
    modifier_avec_ticket(pool, id, saisie, None).await
}

/// [`modifier`], puis dépose le ticket joint, s'il y en a un : le ticket arrive parfois après la
/// dépense. Même ordre qu'à la création : contrôle avant, écriture après.
pub async fn modifier_avec_ticket(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieDepense,
    ticket: Ticket<'_>,
) -> Resultat<Depense> {
    let validee = valider(saisie)?;
    let ticket_pret = preparer_le_ticket(ticket).await?;
    let mut transaction = pool.begin().await?;
    if !depenses::mettre_a_jour(&mut *transaction, id, &validee)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    relever(&mut transaction, &validee).await?;
    transaction.commit().await?;
    joindre(pool, &validee, id, ticket, ticket_pret).await?;
    obtenir(pool, id).await
}

/// Contrôle le ticket, sans rien écrire. Son type est toujours celui du ticket, quoi qu'ait
/// envoyé le formulaire.
async fn preparer_le_ticket(
    ticket: Ticket<'_>,
) -> Resultat<Option<metier_documents::DepotPrepare>> {
    match ticket {
        Some((_, fichier)) => {
            let saisie = SaisieDocument {
                type_document: TYPE_DU_TICKET.code().to_string(),
                ..fichier.clone()
            };
            Ok(Some(metier_documents::preparer(&saisie).await?))
        }
        None => Ok(None),
    }
}

/// Écrit le ticket contrôlé, relié à la dépense, une fois celle-ci enregistrée.
async fn joindre(
    pool: &PgPool,
    validee: &NouvelleDepense,
    id: i64,
    ticket: Ticket<'_>,
    pret: Option<metier_documents::DepotPrepare>,
) -> Resultat<()> {
    if let (Some((documents_dir, _)), Some(pret)) = (ticket, pret) {
        metier_documents::ecrire(
            pool,
            documents_dir,
            Rattachement::Vehicule(validee.vehicule_id),
            pret.pour_depense(id),
        )
        .await?;
    }
    Ok(())
}

/// Le compteur du véhicule suit le plein, jamais en arrière (`GREATEST` en base).
async fn relever(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    validee: &NouvelleDepense,
) -> Resultat<()> {
    if let Some(kilometrage) = validee.kilometrage {
        vehicules::relever_kilometrage(&mut **transaction, validee.vehicule_id, kilometrage)
            .await?;
    }
    Ok(())
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if depenses::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commun::types::Carburant;

    fn plein() -> SaisieDepense {
        SaisieDepense {
            vehicule_id: "3".into(),
            type_depense: "carburant".into(),
            date_depense: "2026-10-12".into(),
            montant: "68,40".into(),
            carburant: "gazole".into(),
            quantite: "45,2".into(),
            kilometrage: "61 200".into(),
            plein_complet: "oui".into(),
            libelle: " Station fictive ".into(),
            notes: String::new(),
        }
    }

    fn refus(s: &SaisieDepense) -> String {
        match valider(s) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        }
    }

    #[test]
    fn le_filtre_se_valide_et_garde_ce_qui_est_dans_la_periode() {
        let ok = |vehicule: &str, du: &str, au: &str| {
            SaisieFiltre {
                vehicule_id: vehicule.into(),
                du: du.into(),
                au: au.into(),
            }
            .valider()
        };
        assert!(ok("", "", "").unwrap().est_vide());
        let f = ok("3", "2026-10-01", "2026-10-31").unwrap();
        assert!(!f.est_vide());
        assert_eq!(f.vehicule_id, Some(3));
        // Les bornes sont comprises.
        let jour = |j: u32| chrono::NaiveDate::from_ymd_opt(2026, 10, j).unwrap();
        let depense = |vehicule: i64, date| Depense {
            id: 1,
            vehicule_id: vehicule,
            type_depense: "peage".into(),
            date_depense: date,
            montant_centimes: 100,
            carburant: String::new(),
            quantite_centiemes: 0,
            kilometrage: None,
            plein_complet: false,
            libelle: String::new(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AB-123-CD".into(),
            couleur: String::new(),
            documents: Vec::new(),
            consommation: None,
        };
        assert!(f.garde(&depense(3, jour(1))));
        assert!(f.garde(&depense(3, jour(31))));
        assert!(!f.garde(&depense(
            3,
            chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
        )));
        assert!(!f.garde(&depense(4, jour(15))), "un autre véhicule");
        // Une seule borne suffit.
        assert!(ok("", "2026-10-15", "")
            .unwrap()
            .garde(&depense(9, jour(20))));
        assert!(!ok("", "2026-10-15", "")
            .unwrap()
            .garde(&depense(9, jour(14))));
        assert!(ok("", "", "2026-10-15")
            .unwrap()
            .garde(&depense(9, jour(14))));
        // Refus, en français.
        let refus = |v: &str, du: &str, au: &str| match ok(v, du, au) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        };
        assert!(refus("abc", "", "").contains("véhicule"));
        assert!(refus("", "demain", "").contains("« du »"));
        assert!(refus("", "", "31/10/2026").contains("« au »"));
        assert!(refus("", "2026-10-31", "2026-10-01").contains("commence après sa fin"));
    }

    #[test]
    fn un_plein_se_valide_en_centimes_et_en_centiemes() {
        let v = valider(&plein()).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Carburant);
        assert_eq!(v.montant_centimes, 6840);
        assert_eq!(v.carburant, Some(CarburantPlein::Gazole));
        assert_eq!(v.quantite_centiemes, 4520);
        assert_eq!(v.kilometrage, Some(61200));
        assert!(v.plein_complet);
        assert_eq!(v.libelle, "Station fictive");
    }

    #[test]
    fn le_carburant_du_plein_est_obligatoire_connu_et_propose_d_apres_le_vehicule() {
        let mut s = plein();
        s.carburant = String::new();
        assert!(refus(&s).contains("carburant"));
        s.carburant = "kerosene".into();
        assert!(refus(&s).contains("kerosene"));
        s.carburant = "sp98".into();
        assert_eq!(valider(&s).unwrap().carburant, Some(CarburantPlein::Sp98));
        // La proposition du formulaire, d'après le carburant du véhicule.
        for (vehicule, propose) in [
            (Carburant::Diesel, CarburantPlein::Gazole),
            (Carburant::Essence, CarburantPlein::Sp95E10),
            (Carburant::Hybride, CarburantPlein::Sp95E10),
            (Carburant::Electrique, CarburantPlein::Electricite),
            (Carburant::Gpl, CarburantPlein::Gpl),
            (Carburant::Autre, CarburantPlein::Autre),
        ] {
            assert_eq!(CarburantPlein::propose_pour(vehicule), propose);
        }
        // Les unités suivent le carburant du plein.
        assert_eq!(CarburantPlein::Sp98.unite(), "L");
        assert_eq!(CarburantPlein::Electricite.unite(), "kWh");
        assert_eq!(CarburantPlein::Gnv.unite(), "kg");
    }

    #[test]
    fn la_quantite_et_le_kilometrage_sont_facultatifs_et_la_case_se_lit() {
        let mut s = plein();
        s.quantite = String::new();
        s.kilometrage = String::new();
        s.plein_complet = String::new();
        let v = valider(&s).unwrap();
        assert_eq!(v.quantite_centiemes, 0);
        assert_eq!(v.kilometrage, None);
        assert!(!v.plein_complet);
        for cochee in ["on", "oui", "true", "1"] {
            s.plein_complet = cochee.into();
            assert!(valider(&s).unwrap().plein_complet, "{cochee}");
        }
        s.plein_complet = "peut-être".into();
        assert!(refus(&s).contains("plein complet"));
        s.plein_complet = String::new();
        s.quantite = "0".into();
        assert!(refus(&s).contains("quantité"));
        s.quantite = "abc".into();
        assert!(refus(&s).contains("quantité"));
        s.quantite = String::new();
        s.kilometrage = "-5".into();
        assert!(refus(&s).contains("kilométrage"));
    }

    #[test]
    fn un_peage_refuse_carburant_quantite_et_kilometrage_et_ignore_la_case() {
        let mut s = plein();
        s.type_depense = "peage".into();
        assert!(refus(&s).contains("carburant"));
        s.carburant = String::new();
        assert!(refus(&s).contains("péage"));
        s.quantite = String::new();
        assert!(refus(&s).contains("péage"));
        s.kilometrage = String::new();
        let v = valider(&s).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Peage);
        assert_eq!(
            (
                v.carburant,
                v.quantite_centiemes,
                v.kilometrage,
                v.plein_complet
            ),
            (None, 0, None, false)
        );
    }

    #[test]
    fn type_inconnu_vehicule_absent_et_montant_illisible_sont_refuses() {
        let mut s = plein();
        s.type_depense = "parking".into();
        assert!(refus(&s).contains("parking"));
        let mut s = plein();
        s.vehicule_id = String::new();
        assert!(refus(&s).contains("véhicule"));
        let mut s = plein();
        s.montant = "abc".into();
        assert!(refus(&s).contains("montant"));
        let mut s = plein();
        s.montant = String::new();
        assert_eq!(valider(&s).unwrap().montant_centimes, 0);
    }

    #[test]
    fn la_saisie_se_reconstruit_depuis_la_depense() {
        let d = Depense {
            id: 1,
            vehicule_id: 3,
            type_depense: "carburant".into(),
            date_depense: chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            montant_centimes: 6840,
            carburant: "sp98".into(),
            quantite_centiemes: 4520,
            kilometrage: Some(61200),
            plein_complet: true,
            libelle: "Station fictive".into(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AB-123-CD".into(),
            couleur: String::new(),
            documents: Vec::new(),
            consommation: None,
        };
        let s = SaisieDepense::depuis(&d);
        assert_eq!(s.montant, "68,40");
        assert_eq!(s.carburant, "sp98");
        assert_eq!(s.quantite, "45,2");
        assert_eq!(s.kilometrage, "61200");
        assert!(s.plein_complet_coche());
        assert_eq!(
            d.detail_fr(),
            "45,2 L SP98 · 61\u{202F}200 km · Station fictive"
        );
        let v = valider(&s).unwrap();
        assert_eq!(v.quantite_centiemes, 4520);
        assert_eq!(v.carburant, Some(CarburantPlein::Sp98));

        let mut partiel = d.clone();
        partiel.plein_complet = false;
        partiel.quantite_centiemes = 0;
        partiel.kilometrage = None;
        partiel.carburant = "electricite".into();
        assert_eq!(
            partiel.detail_fr(),
            "Électricité · plein partiel · Station fictive"
        );
        assert_eq!(SaisieDepense::depuis(&partiel).quantite, "");
        assert_eq!(partiel.unite(), "kWh");

        let mut peage = d.clone();
        peage.type_depense = "peage".into();
        peage.carburant = String::new();
        peage.quantite_centiemes = 0;
        peage.kilometrage = None;
        peage.plein_complet = false;
        peage.libelle = "Lyon – Grenoble".into();
        assert_eq!(peage.detail_fr(), "Lyon – Grenoble");
        assert_eq!(peage.carburant_libelle(), "");
        assert!(!peage.est_un_plein());
        assert_eq!(d.nom_fr(), "Plein du 12/10/2026");
        assert_eq!(peage.nom_fr(), "Péage du 12/10/2026");
    }
}
