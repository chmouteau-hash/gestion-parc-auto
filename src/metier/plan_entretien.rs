//! Le plan d'entretien d'un véhicule (#94, bloc 2) : les opérations suivies, chacune avec sa
//! périodicité en kilomètres, en mois, ou les deux, et pour chacune la dernière fois et la
//! prochaine. La première borne atteinte fait foi.
//!
//! **Ce qui se déduit se calcule, il ne se stocke pas.** La dernière fois est la plus récente
//! intervention du véhicule, par date, qui contient une ligne de l'opération
//! ([`derniere_realisation`]) : corriger ou supprimer une intervention corrige la prochaine, et
//! l'historique ancien se saisit dans n'importe quel ordre. Une intervention datée dans le futur
//! n'est pas une réalisation : le plan l'ignore.
//!
//! Sans aucune réalisation, les bornes se comptent depuis la mise en service du véhicule, à 0 km.
//! Une opération jamais notée n'est jamais « dépassée » : si la borne comptée depuis la mise en
//! service est déjà passée, l'écran dit que la dernière fois est inconnue ([`EtatPlan::Inconnue`]).
//!
//! La « dernière fois » saisie en ajoutant une opération au plan s'enregistre comme une vraie
//! intervention datée, visible dans l'entretien : « Révision » pour une pièce, « Entretien
//! courant » pour un contrôle (décidé le 23/10/2026).

use chrono::{Months, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::{date_fr, nombre_fr};
use crate::commun::types::{Choix, Operation, TypeIntervention};
use crate::donnees::interventions::{
    self as donnees_interventions, Intervention, NouvelleIntervention,
};
use crate::donnees::lignes_intervention::{self, NouvelleLigne};
use crate::donnees::plan_entretien::{
    self, LignePlan, NouvelleLignePlan, INDEX_UNE_LIGNE_PAR_OPERATION,
};
use crate::donnees::vehicules::{self, Vehicule};
use crate::erreurs::{
    a_le_code, contrainte_violee, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE,
    CODE_VIOLATION_UNICITE,
};
use crate::metier::alertes::delai_fr;
use crate::metier::interventions;
use crate::metier::validation::{code_parmi, date_facultative, entier_positif, identifiant};

/// Le plus grand intervalle accepté en kilomètres : au-delà, c'est une faute de frappe.
pub const PERIODICITE_KM_MAX: i32 = 1_000_000;
/// Le plus grand intervalle accepté en mois : vingt ans.
pub const PERIODICITE_MOIS_MAX: i32 = 240;

/// La note de l'intervention créée pour la « dernière fois » saisie avec le plan.
pub const NOTE_DERNIERE_FOIS: &str =
    "Dernière fois notée en ajoutant l'opération au plan d'entretien.";

/// Une fois où l'opération a été faite : l'intervention qui la contient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Realisation {
    pub date: NaiveDate,
    pub kilometrage: i32,
    pub intervention_id: i64,
}

/// Ce que l'écran dit d'une opération suivie. Il se calcule à l'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatPlan {
    /// Aucune borne n'est passée.
    AJour,
    /// Une borne est passée depuis la dernière fois.
    Depassee,
    /// L'opération n'a jamais été notée, et la borne comptée depuis la mise en service est déjà
    /// passée : on ne sait pas quand elle a été faite. Jamais « dépassée » sans réalisation.
    Inconnue,
}

impl EtatPlan {
    pub fn libelle(self) -> &'static str {
        match self {
            EtatPlan::AJour => "À jour",
            EtatPlan::Depassee => "Dépassée",
            EtatPlan::Inconnue => "Dernière fois inconnue",
        }
    }

    /// Classe CSS de la pastille et de la barre de trajet.
    pub fn classe(self) -> &'static str {
        match self {
            EtatPlan::AJour => "plan-a_jour",
            EtatPlan::Depassee => "plan-depassee",
            EtatPlan::Inconnue => "plan-inconnue",
        }
    }

    pub fn est_inconnue(self) -> bool {
        self == EtatPlan::Inconnue
    }
}

/// La borne qui presse le plus : celle dont la part déjà parcourue est la plus grande.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Borne {
    Kilometrage,
    Date,
}

/// Une opération suivie, avec ce qui s'en calcule à la date du jour.
#[derive(Debug, Clone, Serialize)]
pub struct Suivi {
    #[serde(flatten)]
    pub ligne: LignePlan,
    /// La dernière fois qu'elle a été faite ; absente si elle n'a jamais été notée.
    pub derniere_fois: Option<Realisation>,
    /// La prochaine borne en kilomètres, au compteur ; absente sans périodicité en kilomètres,
    /// ou quand la dernière fois est inconnue.
    pub prochaine_km: Option<i32>,
    /// La prochaine borne en date ; absente sans périodicité en mois, ou quand la dernière fois
    /// est inconnue.
    pub prochaine_date: Option<NaiveDate>,
    /// Kilomètres avant la borne (négatif si elle est passée).
    pub reste_km: Option<i64>,
    /// Jours avant la borne (négatif si elle est passée).
    pub reste_jours: Option<i64>,
    /// Part de l'intervalle déjà parcourue, de 0 à 100, sur la borne qui presse le plus.
    pub avancement: u8,
    pub borne: Option<Borne>,
    pub etat: EtatPlan,
    /// La date de mise en service, d'où se comptent les bornes sans réalisation.
    #[serde(skip)]
    pub mise_en_service: NaiveDate,
}

impl Suivi {
    /// « 10/03/2025 à 30 000 km », « jamais notée : depuis la mise en service (01/06/2023) »,
    /// « inconnue ».
    pub fn derniere_fois_fr(&self) -> String {
        match (self.derniere_fois, self.etat) {
            (Some(r), _) => format!(
                "{} à {} km",
                date_fr(r.date),
                nombre_fr(i64::from(r.kilometrage))
            ),
            (None, EtatPlan::Inconnue) => "Inconnue".to_string(),
            (None, _) => format!(
                "Jamais notée : compté depuis la mise en service ({})",
                date_fr(self.mise_en_service)
            ),
        }
    }

    /// La borne en kilomètres et son délai : « 45 000 km · dans 3 200 km », « 45 000 km ·
    /// dépassée de 500 km ».
    pub fn prochaine_km_fr(&self) -> Option<String> {
        let borne = self.prochaine_km?;
        let reste = self.reste_km?;
        Some(format!(
            "{} km · {}",
            nombre_fr(i64::from(borne)),
            delai_km_fr(reste)
        ))
    }

    /// La borne en date et son délai : « 10/03/2026 · dans 140 jours ».
    pub fn prochaine_date_fr(&self) -> Option<String> {
        let borne = self.prochaine_date?;
        let reste = self.reste_jours?;
        Some(format!("{} · {}", date_fr(borne), delai_fr(reste)))
    }

    /// L'avancement arrondi à la dizaine, pour la classe de la barre de trajet.
    pub fn avancement_dizaine(&self) -> u8 {
        (self.avancement + 5) / 10 * 10
    }

    /// Vrai si la borne en kilomètres est celle qui presse : l'écran la met en avant.
    pub fn km_presse(&self) -> bool {
        self.borne == Some(Borne::Kilometrage)
    }
}

/// « dans 3 200 km », « maintenant », « dépassée de 500 km ».
pub fn delai_km_fr(reste: i64) -> String {
    match reste {
        r if r > 0 => format!("dans {} km", nombre_fr(r)),
        0 => "maintenant".to_string(),
        r => format!("dépassée de {} km", nombre_fr(-r)),
    }
}

/// La dernière fois que l'opération a été faite : la plus récente intervention du véhicule, par
/// date (puis par kilométrage), qui en contient une ligne. Une intervention datée après
/// `aujourd_hui` n'est pas une réalisation. Fonction pure : l'ordre des interventions ne compte
/// pas.
pub fn derniere_realisation(
    operation: &str,
    interventions: &[Intervention],
    aujourd_hui: NaiveDate,
) -> Option<Realisation> {
    interventions
        .iter()
        .filter(|i| i.date_intervention <= aujourd_hui)
        .filter(|i| i.lignes.iter().any(|l| l.operation == operation))
        .max_by_key(|i| (i.date_intervention, i.kilometrage, i.id))
        .map(|i| Realisation {
            date: i.date_intervention,
            kilometrage: i.kilometrage,
            intervention_id: i.id,
        })
}

/// Part parcourue d'un intervalle, en millièmes (négatif ramené à 0, plus de 1 000 si dépassé).
fn part(parcouru: i64, intervalle: i64) -> i64 {
    if intervalle <= 0 {
        return 1000;
    }
    parcouru.max(0) * 1000 / intervalle
}

/// Calcule la dernière fois, la prochaine borne et l'état d'une opération suivie. Fonction pure.
pub fn suivre(
    ligne: LignePlan,
    interventions: &[Intervention],
    vehicule: &Vehicule,
    aujourd_hui: NaiveDate,
) -> Suivi {
    let derniere_fois = derniere_realisation(&ligne.operation, interventions, aujourd_hui);
    let (date_depart, km_depart) = match derniere_fois {
        Some(r) => (r.date, r.kilometrage),
        None => (vehicule.date_mise_en_service, 0),
    };
    let compteur = i64::from(vehicule.kilometrage);
    let prochaine_km = ligne.periodicite_km.map(|p| km_depart.saturating_add(p));
    let prochaine_date = ligne.periodicite_mois.map(|m| {
        date_depart
            .checked_add_months(Months::new(m.unsigned_abs()))
            .unwrap_or(NaiveDate::MAX)
    });
    let reste_km = prochaine_km.map(|k| i64::from(k) - compteur);
    let reste_jours = prochaine_date.map(|d| (d - aujourd_hui).num_days());

    let part_km = ligne
        .periodicite_km
        .map(|p| part(compteur - i64::from(km_depart), i64::from(p)));
    let part_date = prochaine_date.map(|d| {
        part(
            (aujourd_hui - date_depart).num_days(),
            (d - date_depart).num_days(),
        )
    });
    let (borne, part_max) = match (part_km, part_date) {
        (Some(k), Some(d)) if k >= d => (Some(Borne::Kilometrage), k),
        (Some(_), Some(d)) => (Some(Borne::Date), d),
        (Some(k), None) => (Some(Borne::Kilometrage), k),
        (None, Some(d)) => (Some(Borne::Date), d),
        (None, None) => (None, 0),
    };
    let depassee = reste_km.is_some_and(|r| r < 0) || reste_jours.is_some_and(|r| r < 0);
    let etat = match (depassee, derniere_fois) {
        (true, None) => EtatPlan::Inconnue,
        (true, Some(_)) => EtatPlan::Depassee,
        (false, _) => EtatPlan::AJour,
    };
    if etat == EtatPlan::Inconnue {
        // Les bornes comptées depuis la mise en service ne disent rien d'utile : on ne les
        // montre pas, pour ne pas faire croire à un retard.
        return Suivi {
            ligne,
            derniere_fois,
            prochaine_km: None,
            prochaine_date: None,
            reste_km: None,
            reste_jours: None,
            avancement: 0,
            borne: None,
            etat,
            mise_en_service: vehicule.date_mise_en_service,
        };
    }
    Suivi {
        ligne,
        derniere_fois,
        prochaine_km,
        prochaine_date,
        reste_km,
        reste_jours,
        avancement: (part_max.min(1000) / 10) as u8,
        borne,
        etat,
        mise_en_service: vehicule.date_mise_en_service,
    }
}

/// Le plan du véhicule, calculé à partir de ses interventions (avec leurs lignes) déjà lues :
/// la fiche les a sous la main.
pub fn suivis(
    lignes: Vec<LignePlan>,
    interventions: &[Intervention],
    vehicule: &Vehicule,
    aujourd_hui: NaiveDate,
) -> Vec<Suivi> {
    lignes
        .into_iter()
        .map(|l| suivre(l, interventions, vehicule, aujourd_hui))
        .collect()
}

/// Le plan du véhicule, tout lu et calculé (pour l'API).
pub async fn plan_du_vehicule(
    pool: &PgPool,
    vehicule_id: i64,
    aujourd_hui: NaiveDate,
) -> Resultat<Vec<Suivi>> {
    let vehicule = vehicules::obtenir(pool, vehicule_id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    let lignes = plan_entretien::lister_par_vehicule(pool, vehicule_id).await?;
    let interventions = interventions::lister_par_vehicule(pool, vehicule_id).await?;
    Ok(suivis(lignes, &interventions, &vehicule, aujourd_hui))
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<LignePlan>> {
    Ok(plan_entretien::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<LignePlan> {
    plan_entretien::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

/// Ce que le formulaire (ou le JSON) envoie pour une opération suivie.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisiePlan {
    #[serde(default)]
    pub vehicule_id: String,
    /// Code du vocabulaire [`Operation`], sauf « autre ».
    #[serde(default)]
    pub operation: String,
    /// Tous les combien de kilomètres ; vide = pas de borne en kilomètres.
    #[serde(default)]
    pub periodicite_km: String,
    /// Tous les combien de mois ; vide = pas de borne en date.
    #[serde(default)]
    pub periodicite_mois: String,
    /// La dernière fois, si on la connaît : sa date (création seulement).
    #[serde(default)]
    pub derniere_date: String,
    /// … et le kilométrage ce jour-là.
    #[serde(default)]
    pub derniere_kilometrage: String,
}

impl SaisiePlan {
    pub fn depuis(l: &LignePlan) -> SaisiePlan {
        SaisiePlan {
            vehicule_id: l.vehicule_id.to_string(),
            operation: l.operation.clone(),
            periodicite_km: l.periodicite_km.map(|k| k.to_string()).unwrap_or_default(),
            periodicite_mois: l
                .periodicite_mois
                .map(|m| m.to_string())
                .unwrap_or_default(),
            derniere_date: String::new(),
            derniere_kilometrage: String::new(),
        }
    }
}

/// Une opération suivie validée, avec la dernière fois si elle a été donnée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValide {
    pub ligne: NouvelleLignePlan,
    pub derniere_fois: Option<(NaiveDate, i32)>,
}

/// Une périodicité : vide = absente, sinon un entier de 1 à `max`.
fn periodicite(valeur: &str, champ: &str, max: i32) -> Resultat<Option<i32>> {
    let propre = valeur.trim().replace([' ', '\u{202F}', '\u{A0}'], "");
    if propre.is_empty() {
        return Ok(None);
    }
    match propre.parse::<i32>() {
        Ok(n) if (1..=max).contains(&n) => Ok(Some(n)),
        _ => Err(ErreurApp::metier(format!(
            "Le champ « {champ} » doit être un nombre entier de 1 à {} (reçu « {propre} »).",
            nombre_fr(i64::from(max))
        ))),
    }
}

/// L'opération suivie : du vocabulaire, sauf « Autre ».
fn operation_suivie(valeur: &str) -> Resultat<Operation> {
    if valeur.trim().is_empty() {
        return Err(ErreurApp::metier(
            "Choisir l'opération à suivre dans la liste.",
        ));
    }
    let operation = code_parmi(valeur, "opération", Operation::depuis_code)?;
    if operation == Operation::Autre {
        return Err(ErreurApp::metier(
            "« Autre » ne se suit pas dans le plan : son texte libre ne permet pas de retrouver la dernière fois. Choisir une opération de la liste.",
        ));
    }
    Ok(operation)
}

/// Valide l'opération et sa périodicité. La dernière fois n'est lue qu'à la création
/// (`avec_derniere_fois`) ; une date dans le futur est refusée : ce qui est à venir n'est pas
/// encore fait.
pub fn valider(
    s: &SaisiePlan,
    avec_derniere_fois: bool,
    aujourd_hui: NaiveDate,
) -> Resultat<PlanValide> {
    let vehicule_id = identifiant(&s.vehicule_id, "véhicule")?;
    let operation = operation_suivie(&s.operation)?;
    let periodicite_km = periodicite(&s.periodicite_km, "tous les … km", PERIODICITE_KM_MAX)?;
    let periodicite_mois =
        periodicite(&s.periodicite_mois, "tous les … mois", PERIODICITE_MOIS_MAX)?;
    if periodicite_km.is_none() && periodicite_mois.is_none() {
        return Err(ErreurApp::metier(
            "Indiquer tous les combien la refaire : en kilomètres, en mois, ou les deux.",
        ));
    }
    let derniere_fois = if avec_derniere_fois {
        let date = date_facultative(&s.derniere_date, "dernière fois")?;
        let km_saisi = !s.derniere_kilometrage.trim().is_empty();
        match date {
            None if km_saisi => {
                return Err(ErreurApp::metier(
                    "Indiquer aussi la date de la dernière fois, ou effacer son kilométrage.",
                ))
            }
            None => None,
            Some(d) if d > aujourd_hui => {
                return Err(ErreurApp::metier(
                    "La dernière fois ne peut pas être dans le futur : ce qui est à venir n'est pas encore fait.",
                ))
            }
            Some(d) => Some((
                d,
                entier_positif(&s.derniere_kilometrage, "kilométrage de la dernière fois")?,
            )),
        }
    } else {
        None
    };
    Ok(PlanValide {
        ligne: NouvelleLignePlan {
            vehicule_id,
            operation,
            periodicite_km,
            periodicite_mois,
        },
        derniere_fois,
    })
}

fn traduire(erreur: sqlx::Error, operation: Operation) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_UNICITE)
        && contrainte_violee(&erreur).as_deref() == Some(INDEX_UNE_LIGNE_PAR_OPERATION)
    {
        ErreurApp::metier(format!(
            "L'opération « {} » est déjà suivie pour ce véhicule : modifiez sa ligne du plan.",
            operation.libelle()
        ))
    } else if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

/// Le type de l'intervention qui note la dernière fois : « Entretien courant » pour un contrôle,
/// « Révision » pour une pièce (décidé le 23/10/2026).
pub fn type_de_la_derniere_fois(operation: Operation) -> TypeIntervention {
    if operation.est_controle() {
        TypeIntervention::EntretienCourant
    } else {
        TypeIntervention::Revision
    }
}

/// Ajoute l'opération au plan du véhicule. Si la dernière fois est donnée, elle s'enregistre en
/// même temps comme une intervention datée, avec une ligne de l'opération, et relève le compteur
/// s'il est plus bas : tout ou rien, dans une transaction. Rend la ligne du plan et, le cas
/// échéant, l'intervention créée.
pub async fn creer(
    pool: &PgPool,
    saisie: &SaisiePlan,
    aujourd_hui: NaiveDate,
) -> Resultat<(LignePlan, Option<i64>)> {
    let valide = valider(saisie, true, aujourd_hui)?;
    let operation = valide.ligne.operation;
    let mut transaction = pool.begin().await?;
    let id = plan_entretien::inserer(&mut *transaction, &valide.ligne)
        .await
        .map_err(|e| traduire(e, operation))?;
    let mut intervention_id = None;
    if let Some((date, kilometrage)) = valide.derniere_fois {
        let nouvelle = NouvelleIntervention {
            vehicule_id: valide.ligne.vehicule_id,
            type_intervention: type_de_la_derniere_fois(operation),
            resultat_controle: None,
            date_intervention: date,
            kilometrage,
            cout_centimes: 0,
            prestataire: String::new(),
            notes: NOTE_DERNIERE_FOIS.to_string(),
        };
        let iid = donnees_interventions::inserer(&mut *transaction, &nouvelle).await?;
        lignes_intervention::inserer(
            &mut *transaction,
            iid,
            &NouvelleLigne {
                operation,
                libelle: String::new(),
                reference_piece: String::new(),
                quantite_centiemes: 100,
                prix_centimes: 0,
            },
        )
        .await?;
        vehicules::relever_kilometrage(&mut *transaction, valide.ligne.vehicule_id, kilometrage)
            .await?;
        intervention_id = Some(iid);
    }
    transaction.commit().await?;
    Ok((obtenir(pool, id).await?, intervention_id))
}

/// Change l'opération ou la périodicité d'une ligne du plan. La ligne reste sur son véhicule ; la
/// dernière fois, elle, se change dans l'entretien.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisiePlan,
    aujourd_hui: NaiveDate,
) -> Resultat<LignePlan> {
    let actuelle = obtenir(pool, id).await?;
    let mut saisie = saisie.clone();
    saisie.vehicule_id = actuelle.vehicule_id.to_string();
    let valide = valider(&saisie, false, aujourd_hui)?;
    if !plan_entretien::mettre_a_jour(pool, id, &valide.ligne)
        .await
        .map_err(|e| traduire(e, valide.ligne.operation))?
    {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

/// Ne plus suivre l'opération : la ligne du plan part, les interventions restent.
pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if plan_entretien::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

/// Les opérations que le formulaire propose, en deux groupes comme « Ce qui a été fait » : sans
/// « Autre », et sans celles déjà suivies sur le véhicule, sauf `courante` (la ligne modifiée).
pub fn operations_proposees(
    deja_suivies: &[LignePlan],
    courante: Option<&str>,
) -> [(&'static str, Vec<Choix>); 2] {
    let libres = |choix: Vec<Choix>| -> Vec<Choix> {
        choix
            .into_iter()
            .filter(|c| c.code != Operation::Autre.code())
            .filter(|c| {
                Some(c.code) == courante || !deja_suivies.iter().any(|l| l.operation == c.code)
            })
            .collect()
    };
    [
        ("Pièces et consommables", libres(Operation::pieces())),
        ("Contrôles et réglages", libres(Operation::controles())),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::lignes_intervention::LigneIntervention;
    use chrono::Utc;

    fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, j).unwrap()
    }

    fn vehicule(mise_en_service: NaiveDate, kilometrage: i32) -> Vehicule {
        Vehicule {
            id: 1,
            immatriculation: "PL-100-AN".into(),
            marque: "Renault".into(),
            modele: "Clio".into(),
            date_mise_en_service: mise_en_service,
            carburant: "essence".into(),
            kilometrage,
            statut: "en_service".into(),
            vin: String::new(),
            couleur: String::new(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }
    }

    fn intervention(
        id: i64,
        date: NaiveDate,
        kilometrage: i32,
        operations: &[&str],
    ) -> Intervention {
        Intervention {
            id,
            vehicule_id: 1,
            type_intervention: "revision".into(),
            resultat_controle: String::new(),
            date_intervention: date,
            kilometrage,
            cout_centimes: 0,
            prestataire: String::new(),
            notes: String::new(),
            cree_le: Utc::now(),
            immatriculation: "PL-100-AN".into(),
            couleur: String::new(),
            controle_initial: None,
            lignes: operations
                .iter()
                .enumerate()
                .map(|(n, o)| LigneIntervention {
                    id: id * 10 + n as i64,
                    intervention_id: id,
                    operation: (*o).into(),
                    libelle: String::new(),
                    reference_piece: String::new(),
                    quantite_centiemes: 100,
                    prix_centimes: 0,
                })
                .collect(),
            documents: Vec::new(),
        }
    }

    fn ligne(operation: &str, km: Option<i32>, mois: Option<i32>) -> LignePlan {
        LignePlan {
            id: 1,
            vehicule_id: 1,
            operation: operation.into(),
            periodicite_km: km,
            periodicite_mois: mois,
            cree_le: Utc::now(),
        }
    }

    #[test]
    fn la_derniere_fois_est_la_plus_recente_par_date_quel_que_soit_l_ordre() {
        let aujourd_hui = jour(2026, 10, 23);
        let i2023 = intervention(3, jour(2023, 5, 1), 20000, &["vidange"]);
        let i2024 = intervention(1, jour(2024, 3, 10), 30000, &["vidange", "filtre_huile"]);
        let i2025 = intervention(2, jour(2025, 4, 2), 42000, &["vidange"]);
        let attendu = Some(Realisation {
            date: jour(2025, 4, 2),
            kilometrage: 42000,
            intervention_id: 2,
        });
        // 2024, 2025, 2023 donne la même dernière fois que 2023, 2024, 2025.
        for ordre in [
            vec![i2024.clone(), i2025.clone(), i2023.clone()],
            vec![i2023.clone(), i2024.clone(), i2025.clone()],
            vec![i2025.clone(), i2023.clone(), i2024.clone()],
        ] {
            assert_eq!(
                derniere_realisation("vidange", &ordre, aujourd_hui),
                attendu
            );
        }
        // Le filtre à huile n'a été fait qu'en 2024.
        assert_eq!(
            derniere_realisation("filtre_huile", &[i2023, i2024, i2025], aujourd_hui)
                .map(|r| r.date),
            Some(jour(2024, 3, 10))
        );
    }

    #[test]
    fn une_intervention_datee_dans_le_futur_n_est_pas_une_realisation() {
        let aujourd_hui = jour(2026, 10, 23);
        let faite = intervention(1, jour(2026, 1, 15), 40000, &["vidange"]);
        let prevue = intervention(2, jour(2026, 12, 1), 45000, &["vidange"]);
        assert_eq!(
            derniere_realisation("vidange", &[faite, prevue], aujourd_hui).map(|r| r.date),
            Some(jour(2026, 1, 15))
        );
    }

    #[test]
    fn la_prochaine_se_compte_depuis_la_derniere_fois_et_la_premiere_borne_fait_foi() {
        let aujourd_hui = jour(2026, 10, 23);
        let v = vehicule(jour(2020, 1, 15), 41000);
        let faite = intervention(1, jour(2026, 3, 10), 30000, &["vidange"]);
        let s = suivre(
            ligne("vidange", Some(15000), Some(12)),
            &[faite],
            &v,
            aujourd_hui,
        );
        assert_eq!(s.prochaine_km, Some(45000));
        assert_eq!(s.prochaine_date, Some(jour(2027, 3, 10)));
        assert_eq!(s.reste_km, Some(4000));
        assert_eq!(s.reste_jours, Some(138));
        // 11 000 km sur 15 000 (73 %) pressent plus que 227 jours sur 365 (62 %).
        assert_eq!(s.borne, Some(Borne::Kilometrage));
        assert_eq!(s.avancement, 73);
        assert_eq!(s.avancement_dizaine(), 70);
        assert_eq!(s.etat, EtatPlan::AJour);
        assert_eq!(
            s.prochaine_km_fr().unwrap(),
            "45\u{202F}000 km · dans 4\u{202F}000 km"
        );
        assert_eq!(
            s.prochaine_date_fr().unwrap(),
            "10/03/2027 · dans 138 jours"
        );
        assert_eq!(s.derniere_fois_fr(), "10/03/2026 à 30\u{202F}000 km");
    }

    #[test]
    fn une_borne_passee_rend_l_operation_depassee() {
        let aujourd_hui = jour(2026, 10, 23);
        // La date est passée, pas les kilomètres : la date fait foi.
        let v = vehicule(jour(2020, 1, 15), 35000);
        let faite = intervention(1, jour(2025, 9, 1), 30000, &["vidange"]);
        let s = suivre(
            ligne("vidange", Some(15000), Some(12)),
            std::slice::from_ref(&faite),
            &v,
            aujourd_hui,
        );
        assert_eq!(s.etat, EtatPlan::Depassee);
        assert_eq!(s.borne, Some(Borne::Date));
        assert_eq!(s.reste_jours, Some(-52));
        assert_eq!(s.avancement, 100);
        // Les kilomètres seuls, dépassés de 500 km.
        let v = vehicule(jour(2020, 1, 15), 45500);
        let s = suivre(
            ligne("vidange", Some(15000), None),
            &[faite],
            &v,
            aujourd_hui,
        );
        assert_eq!(s.etat, EtatPlan::Depassee);
        assert_eq!(
            s.prochaine_km_fr().unwrap(),
            "45\u{202F}000 km · dépassée de 500 km"
        );
        assert_eq!(s.prochaine_date, None);
    }

    #[test]
    fn une_operation_jamais_notee_n_est_jamais_en_retard() {
        let aujourd_hui = jour(2026, 10, 23);
        // Un véhicule de 2020 : la vidange de 12 mois comptée depuis la mise en service est passée
        // depuis longtemps, mais on ne sait pas quand elle a été faite.
        let ancien = vehicule(jour(2020, 1, 15), 80000);
        let s = suivre(
            ligne("vidange", Some(15000), Some(12)),
            &[],
            &ancien,
            aujourd_hui,
        );
        assert_eq!(s.etat, EtatPlan::Inconnue);
        assert_eq!(s.prochaine_km, None);
        assert_eq!(s.prochaine_date, None);
        assert_eq!(s.derniere_fois_fr(), "Inconnue");
        // Un véhicule neuf : la borne comptée depuis la mise en service n'est pas atteinte.
        let neuf = vehicule(jour(2026, 6, 1), 4000);
        let s = suivre(
            ligne("vidange", Some(15000), Some(12)),
            &[],
            &neuf,
            aujourd_hui,
        );
        assert_eq!(s.etat, EtatPlan::AJour);
        assert_eq!(s.prochaine_km, Some(15000));
        assert_eq!(s.prochaine_date, Some(jour(2027, 6, 1)));
        assert_eq!(
            s.derniere_fois_fr(),
            "Jamais notée : compté depuis la mise en service (01/06/2026)"
        );
    }

    #[test]
    fn delais_en_kilometres() {
        assert_eq!(delai_km_fr(1800), "dans 1\u{202F}800 km");
        assert_eq!(delai_km_fr(0), "maintenant");
        assert_eq!(delai_km_fr(-500), "dépassée de 500 km");
    }

    fn saisie(operation: &str, km: &str, mois: &str) -> SaisiePlan {
        SaisiePlan {
            vehicule_id: "3".into(),
            operation: operation.into(),
            periodicite_km: km.into(),
            periodicite_mois: mois.into(),
            ..Default::default()
        }
    }

    fn refus(r: Resultat<PlanValide>) -> String {
        match r {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        }
    }

    #[test]
    fn validation_de_la_saisie() {
        let aujourd_hui = jour(2026, 10, 23);
        let v = valider(&saisie("vidange", "15 000", "12"), true, aujourd_hui).unwrap();
        assert_eq!(v.ligne.operation, Operation::Vidange);
        assert_eq!(v.ligne.periodicite_km, Some(15000));
        assert_eq!(v.ligne.periodicite_mois, Some(12));
        assert_eq!(v.derniere_fois, None);

        assert!(
            refus(valider(&saisie("vidange", "", ""), true, aujourd_hui))
                .contains("tous les combien")
        );
        assert!(
            refus(valider(&saisie("autre", "", "12"), true, aujourd_hui))
                .contains("« Autre » ne se suit pas")
        );
        assert!(refus(valider(&saisie("", "", "12"), true, aujourd_hui)).contains("Choisir"));
        assert!(refus(valider(&saisie("vidange", "0", ""), true, aujourd_hui)).contains("de 1 à"));
        assert!(refus(valider(&saisie("vidange", "", "241"), true, aujourd_hui)).contains("240"));

        let mut s = saisie("pression_pneus", "", "1");
        s.derniere_date = "2026-10-01".into();
        s.derniere_kilometrage = "41 000".into();
        let v = valider(&s, true, aujourd_hui).unwrap();
        assert_eq!(v.derniere_fois, Some((jour(2026, 10, 1), 41000)));
        // La modification ne lit pas la dernière fois.
        assert_eq!(valider(&s, false, aujourd_hui).unwrap().derniere_fois, None);

        s.derniere_date = "2026-11-01".into();
        assert!(refus(valider(&s, true, aujourd_hui)).contains("dans le futur"));
        s.derniere_date = String::new();
        assert!(refus(valider(&s, true, aujourd_hui)).contains("date de la dernière fois"));
        s.derniere_date = "2026-10-01".into();
        s.derniere_kilometrage = String::new();
        assert!(refus(valider(&s, true, aujourd_hui)).contains("obligatoire"));
    }

    #[test]
    fn la_derniere_fois_d_un_controle_est_un_entretien_courant() {
        assert_eq!(
            type_de_la_derniere_fois(Operation::PressionPneus),
            TypeIntervention::EntretienCourant
        );
        assert_eq!(
            type_de_la_derniere_fois(Operation::Vidange),
            TypeIntervention::Revision
        );
    }

    #[test]
    fn le_formulaire_ne_propose_ni_autre_ni_une_operation_deja_suivie() {
        let suivies = vec![ligne("vidange", Some(15000), None)];
        let [pieces, controles] = operations_proposees(&suivies, None);
        assert_eq!(pieces.0, "Pièces et consommables");
        assert!(!pieces.1.iter().any(|c| c.code == "vidange"));
        assert!(pieces.1.iter().any(|c| c.code == "filtre_huile"));
        assert!(!controles.1.iter().any(|c| c.code == "autre"));
        assert!(controles.1.iter().any(|c| c.code == "pression_pneus"));
        // En modification, l'opération de la ligne reste proposée.
        let [pieces, _] = operations_proposees(&suivies, Some("vidange"));
        assert!(pieces.1.iter().any(|c| c.code == "vidange"));
    }
}
