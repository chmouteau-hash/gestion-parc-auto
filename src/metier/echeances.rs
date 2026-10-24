//! Échéances réglementaires et d'entretien : validation et opérations.
//!
//! Un véhicule n'a pas deux échéances **ouvertes** de même type, de même date limite et de même
//! chose à faire : ce serait deux lignes indiscernables, presque toujours une double saisie. La règle est tenue par l'index
//! unique partiel `echeances_pas_de_doublon_ouverte`, que [`traduire`] rend en message lisible. Elle
//! ne regarde pas les échéances traitées : une échéance traitée ne bloque pas une nouvelle saisie.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::date_fr;
use crate::commun::types::{
    Operation, ResultatControle, StatutEcheance, TypeEcheance, TypeIntervention,
};
use crate::donnees::echeances::{
    self, Echeance, NouvelleEcheance, INDEX_PAS_DE_DOUBLON_OUVERTE, INDEX_UNE_CONTRE_VISITE_OUVERTE,
};
use crate::donnees::interventions::{
    self as donnees_interventions, date_limite_contre_visite, Intervention, NouvelleIntervention,
};
use crate::donnees::lignes_intervention::NouvelleLigne;
use crate::donnees::plan_entretien as donnees_plan;
use crate::erreurs::{
    a_le_code, contrainte_violee, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE,
    CODE_VIOLATION_UNICITE,
};
use crate::metier::alertes::{jusqu_au, niveau, Niveau};
use crate::metier::validation::{
    code_parmi, date_obligatoire, identifiant, texte_facultatif, texte_obligatoire,
};

/// Ce que l'écran dit d'une échéance : son statut, précisé par la date du jour.
///
/// Il se calcule à l'affichage, il n'est pas stocké : une échéance « à traiter » devient « à venir »
/// quand sa date entre dans l'horizon des alertes, puis « dépassée » le lendemain de sa date
/// limite, sans que personne n'y touche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatEcheance {
    /// Ouverte, et sa date limite est passée.
    Depassee,
    /// Ouverte, pas encore dépassée (le jour même, elle ne l'est pas), et sa date limite tombe
    /// dans l'horizon des alertes : la vue alertes la montre.
    AVenir,
    /// Ouverte, et sa date limite est au-delà de l'horizon des alertes.
    ATraiter,
    Traitee,
    /// Un « Entretien périodique » ouvert d'un véhicule qui a un plan d'entretien : le plan le
    /// remplace, il ne compte ni dans les alertes ni sur l'accueil (#94, bloc 2b).
    RemplaceeParLePlan,
    /// Un « Contrôle technique » ouvert d'un véhicule qui a une contre-visite à faire : elle le
    /// remplace jusqu'à ce qu'elle soit faite (#107, décidé le 23/10/2026).
    RemplaceeParLaContreVisite,
}

impl EtatEcheance {
    pub fn libelle(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => Niveau::Depassee.libelle(),
            EtatEcheance::AVenir => Niveau::AVenir.libelle(),
            EtatEcheance::ATraiter => StatutEcheance::Ouverte.libelle(),
            EtatEcheance::Traitee => StatutEcheance::Traitee.libelle(),
            EtatEcheance::RemplaceeParLePlan => "Remplacée par le plan d'entretien",
            EtatEcheance::RemplaceeParLaContreVisite => "Remplacée par la contre-visite",
        }
    }

    /// Classe CSS de la pastille.
    pub fn classe(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => "echeance-depassee",
            EtatEcheance::AVenir => "echeance-a_venir",
            EtatEcheance::ATraiter => "echeance-a_traiter",
            EtatEcheance::Traitee => "echeance-traitee",
            EtatEcheance::RemplaceeParLePlan | EtatEcheance::RemplaceeParLaContreVisite => {
                "echeance-remplacee"
            }
        }
    }
}

/// État d'une échéance à la date donnée. « Dépassée » se décide par [`niveau`] et « à venir » par
/// [`jusqu_au`], les règles de la vue alertes : la liste, la fiche et les alertes ne peuvent pas
/// se contredire.
pub fn etat(echeance: &Echeance, aujourd_hui: NaiveDate, horizon_jours: i64) -> EtatEcheance {
    if !echeance.est_ouverte() {
        EtatEcheance::Traitee
    } else if niveau(echeance.date_limite, aujourd_hui) == Niveau::Depassee {
        EtatEcheance::Depassee
    } else if echeance.date_limite <= jusqu_au(aujourd_hui, horizon_jours) {
        EtatEcheance::AVenir
    } else {
        EtatEcheance::ATraiter
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieEcheance {
    #[serde(default)]
    pub vehicule_id: String,
    #[serde(default)]
    pub type_echeance: String,
    #[serde(default)]
    pub date_limite: String,
    /// Vide = « ouverte ».
    #[serde(default)]
    pub statut: String,
    /// Pour un « à faire » : code d'`Operation`. Vide pour les autres types.
    #[serde(default)]
    pub operation: String,
    /// Pour un « à faire » d'opération « autre » : ce qu'il y a à faire.
    #[serde(default)]
    pub libelle: String,
    /// Pour un « à faire » : pourquoi (facultatif).
    #[serde(default)]
    pub constat: String,
}

impl SaisieEcheance {
    pub fn depuis(e: &Echeance) -> SaisieEcheance {
        SaisieEcheance {
            vehicule_id: e.vehicule_id.to_string(),
            type_echeance: e.type_echeance.clone(),
            date_limite: e.date_limite.to_string(),
            statut: e.statut.clone(),
            operation: e.operation.clone(),
            libelle: e.libelle.clone(),
            constat: e.constat.clone(),
        }
    }
}

pub fn valider(s: &SaisieEcheance) -> Resultat<NouvelleEcheance> {
    let statut = if s.statut.trim().is_empty() {
        StatutEcheance::Ouverte
    } else {
        code_parmi(&s.statut, "statut", StatutEcheance::depuis_code)?
    };
    let type_echeance = code_parmi(&s.type_echeance, "type", TypeEcheance::depuis_code)?;
    let (operation, libelle, constat) = a_faire(type_echeance, s)?;
    Ok(NouvelleEcheance {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_echeance,
        date_limite: date_obligatoire(&s.date_limite, "date limite")?,
        statut,
        operation,
        libelle,
        constat,
    })
}

/// Ce qu'une échéance dit de la chose à faire : (opération, libellé, constat). Un « à faire »
/// dit quoi (une opération du vocabulaire, ou « autre » avec son libellé) et, s'il veut, pourquoi.
/// Les autres types ne portent rien de tout cela : une opération, un libellé ou un constat sur un
/// contrôle technique est refusé plutôt qu'ignoré, pour qu'une saisie ne perde rien en silence.
fn a_faire(type_echeance: TypeEcheance, s: &SaisieEcheance) -> Resultat<(String, String, String)> {
    let operation = s.operation.trim();
    if type_echeance != TypeEcheance::AFaire {
        if !operation.is_empty() || !s.libelle.trim().is_empty() || !s.constat.trim().is_empty() {
            return Err(ErreurApp::metier(format!(
                "Ce qu'il y a à faire et le constat ne vont qu'avec le type « {} ».",
                TypeEcheance::AFaire.libelle()
            )));
        }
        return Ok((String::new(), String::new(), String::new()));
    }
    if operation.is_empty() {
        return Err(ErreurApp::metier(
            "Choisissez ce qu'il y a à faire (ou « Autre » pour le préciser).",
        ));
    }
    let operation = code_parmi(operation, "ce qu'il y a à faire", Operation::depuis_code)?;
    let libelle = if operation == Operation::Autre {
        texte_obligatoire(&s.libelle, "préciser ce qu'il y a à faire")?
    } else if s.libelle.trim().is_empty() {
        String::new()
    } else {
        return Err(ErreurApp::metier(
            "Le libellé ne va qu'avec « Autre » : choisissez « Autre » pour préciser autre chose.",
        ));
    };
    let constat = texte_facultatif(&s.constat, "constat")?;
    Ok((operation.code().to_string(), libelle, constat))
}

/// Traduit les refus de la base en messages lisibles. `nouvelle` est la saisie refusée : le message
/// y reprend le type et la date, ce que l'utilisateur vient de taper.
///
/// Le même message sert à la création, à la modification, et à la réouverture d'une échéance
/// traitée (son statut repasse à « ouverte » : elle peut alors doubler une autre).
fn traduire(erreur: sqlx::Error, nouvelle: &NouvelleEcheance) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_UNICITE)
        && contrainte_violee(&erreur).as_deref() == Some(INDEX_PAS_DE_DOUBLON_OUVERTE)
    {
        ErreurApp::metier(format!(
            "Ce véhicule a déjà une échéance « {} » au {} qui n'est pas encore traitée.",
            nouvelle.type_libelle(),
            date_fr(nouvelle.date_limite)
        ))
    } else if a_le_code(&erreur, CODE_VIOLATION_UNICITE)
        && contrainte_violee(&erreur).as_deref() == Some(INDEX_UNE_CONTRE_VISITE_OUVERTE)
    {
        ErreurApp::metier(
            "Ce véhicule a déjà une contre-visite à faire : modifiez-la plutôt que d'en ajouter une.",
        )
    } else if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Echeance>> {
    Ok(echeances::lister(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Echeance>> {
    Ok(echeances::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Echeance> {
    echeances::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn creer(pool: &PgPool, saisie: &SaisieEcheance) -> Resultat<Echeance> {
    let nouvelle = valider(saisie)?;
    if nouvelle.type_echeance == TypeEcheance::EntretienPeriodique
        && !crate::metier::plan_entretien::lister_par_vehicule(pool, nouvelle.vehicule_id)
            .await?
            .is_empty()
    {
        return Err(ErreurApp::metier(
            "Ce véhicule a un plan d'entretien : il remplace l'échéance « Entretien périodique ». Suivez plutôt l'opération dans le plan, sur la fiche du véhicule.",
        ));
    }
    let id = echeances::inserer(pool, &nouvelle)
        .await
        .map_err(|e| traduire(e, &nouvelle))?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieEcheance) -> Resultat<Echeance> {
    let nouvelle = valider(saisie)?;
    if !echeances::mettre_a_jour(pool, id, &nouvelle)
        .await
        .map_err(|e| traduire(e, &nouvelle))?
    {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

/// Combien de mois après une échéance tombe la suivante du même type : deux ans pour un contrôle
/// technique, un an pour l'assurance et l'entretien périodique (durées confirmées par Christophe
/// le 14/10/2026, issue #39, bloc 5). Un « à faire » se fait une fois : pas de suivante. Une
/// contre-visite non plus : c'est le contrôle technique qui a sa suivante (#107).
pub fn periode_mois(type_echeance: TypeEcheance) -> Option<u32> {
    match type_echeance {
        TypeEcheance::ControleTechnique => Some(24),
        TypeEcheance::Assurance | TypeEcheance::EntretienPeriodique => Some(12),
        TypeEcheance::AFaire | TypeEcheance::ContreVisite => None,
    }
}

/// Combien de mois avant sa date limite une intervention peut remplir l'échéance ([`remplit`]) :
/// sa période, et deux mois pour une contre-visite, le délai qui suit le contrôle défavorable.
pub fn fenetre_mois(type_echeance: TypeEcheance) -> Option<u32> {
    match type_echeance {
        TypeEcheance::ContreVisite => Some(2),
        autre => periode_mois(autre),
    }
}

/// La prochaine échéance, pré-remplie après « Marquer traitée » : même véhicule, même type, la
/// date limite décalée d'une période. Elle n'est pas enregistrée : c'est l'utilisateur qui décide.
/// `None` pour un « à faire » : fait, il est fait.
///
/// D'où part la période (#95) : pour l'**assurance**, de l'ancienne date limite, c'est
/// l'anniversaire du contrat, qui ne dépend pas du jour où l'on clique. Pour le **contrôle
/// technique** et l'**entretien périodique**, de la plus tardive de l'ancienne date limite et du
/// jour : le contrôle suivant est dû deux ans après la visite, et une échéance traitée en retard,
/// ou saisie pour mémoire, ne doit pas en proposer une déjà passée (une cascade de « dépassée »).
/// La date reste modifiable dans le formulaire : on la corrige si la visite n'a pas eu lieu le
/// jour du clic. Quand c'est une intervention qui remplit l'échéance (#94, bloc 3), la suivante
/// part de la date de la visite : voir [`suivante_apres`].
pub fn prochaine(echeance: &Echeance, aujourd_hui: NaiveDate) -> Option<SaisieEcheance> {
    let type_echeance = TypeEcheance::depuis_code(&echeance.type_echeance);
    let mois = match type_echeance {
        Some(type_connu) => periode_mois(type_connu)?,
        None => 12,
    };
    let depart = match type_echeance {
        Some(TypeEcheance::Assurance) => echeance.date_limite,
        _ => echeance.date_limite.max(aujourd_hui),
    };
    let date_limite = depart
        .checked_add_months(chrono::Months::new(mois))
        .unwrap_or(depart);
    Some(SaisieEcheance {
        vehicule_id: echeance.vehicule_id.to_string(),
        type_echeance: echeance.type_echeance.clone(),
        date_limite: date_limite.to_string(),
        statut: String::new(),
        ..Default::default()
    })
}

/// Le type d'échéance qu'une intervention remplit (#94, bloc 3) : un contrôle technique
/// **favorable** remplit le « Contrôle technique », une **révision** l'« Entretien périodique ».
/// Un contrôle défavorable ne remplit rien : le véhicule n'est pas en règle, l'échéance reste
/// ouverte (la contre-visite viendra avec #107). L'« Entretien courant » non plus : le gonflage du
/// mois n'est pas l'entretien de l'année.
pub fn type_rempli(
    type_intervention: TypeIntervention,
    resultat: Option<ResultatControle>,
) -> Option<TypeEcheance> {
    match (type_intervention, resultat) {
        (TypeIntervention::ControleTechnique, Some(ResultatControle::Favorable)) => {
            Some(TypeEcheance::ControleTechnique)
        }
        (TypeIntervention::Revision, _) => Some(TypeEcheance::EntretienPeriodique),
        _ => None,
    }
}

/// Vrai si une intervention faite le `date` remplit l'échéance (#94, bloc 3) : l'échéance est
/// ouverte, et la visite tombe dans la période qui précède sa date limite (deux ans pour un
/// contrôle technique, un an pour l'entretien périodique), ou après. La visite d'où part
/// l'échéance, ou une plus ancienne, ne la remplit pas : un contrôle de 2022 saisi pour mémoire
/// ne solde pas l'échéance de 2026, et la saisie du passé ne solde rien en cascade (#95). Une
/// intervention datée dans le futur n'est pas faite (#94, bloc 2) : elle ne remplit rien. Pour une
/// contre-visite, la fenêtre est de deux mois ([`fenetre_mois`]).
pub fn remplit(echeance: &Echeance, date: NaiveDate, aujourd_hui: NaiveDate) -> bool {
    let Some(mois) = TypeEcheance::depuis_code(&echeance.type_echeance).and_then(fenetre_mois)
    else {
        return false;
    };
    let debut = echeance
        .date_limite
        .checked_sub_months(chrono::Months::new(mois))
        .unwrap_or(NaiveDate::MIN);
    echeance.est_ouverte() && date > debut && date <= aujourd_hui
}

/// L'échéance du type donné que l'intervention faite le `date` remplit, parmi celles du véhicule
/// ([`remplit`]) : la première due, s'il y en a plusieurs.
pub fn echeance_remplie(
    echeances: &[Echeance],
    type_echeance: TypeEcheance,
    date: NaiveDate,
    aujourd_hui: NaiveDate,
) -> Option<&Echeance> {
    echeances
        .iter()
        .filter(|e| e.type_echeance == type_echeance.code())
        .filter(|e| remplit(e, date, aujourd_hui))
        .min_by_key(|e| (e.date_limite, e.id))
}

/// La suivante d'une échéance remplie le `date` (#94, bloc 3) : même véhicule, même type, une
/// période après la visite (deux ans pour le contrôle technique, un an pour l'entretien
/// périodique). Comme après « Marquer traitée », elle est proposée, pas enregistrée.
pub fn suivante_apres(echeance: &Echeance, date: NaiveDate) -> Option<SaisieEcheance> {
    let mois = TypeEcheance::depuis_code(&echeance.type_echeance).and_then(periode_mois)?;
    let date_limite = date
        .checked_add_months(chrono::Months::new(mois))
        .unwrap_or(date);
    Some(SaisieEcheance {
        vehicule_id: echeance.vehicule_id.to_string(),
        type_echeance: echeance.type_echeance.clone(),
        date_limite: date_limite.to_string(),
        statut: String::new(),
        ..Default::default()
    })
}

/// Ce que l'enregistrement d'une intervention a soldé (#94, bloc 3 ; #107, bloc 3).
#[derive(Debug, Clone)]
pub struct Solde {
    /// « Contrôle technique », « Entretien périodique » ou « Contre-visite ».
    pub type_echeance: TypeEcheance,
    /// L'échéance, désormais « traitée ».
    pub echeance: Echeance,
    /// La suivante à proposer : datée de la visite ([`suivante_apres`]) ; après une contre-visite,
    /// le prochain contrôle technique, deux ans après le contrôle initial.
    pub suivante: Option<SaisieEcheance>,
}

/// Ce qu'une intervention dit de la contre-visite (#107).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteContreVisite {
    /// Contrôle défavorable : l'échéance « Contre-visite » est créée, à faire avant cette date.
    AFaire(NaiveDate),
    /// Contre-visite défavorable, ou faite après les deux mois : un contrôle technique complet est
    /// à refaire. L'application le dit, sans l'imposer.
    ControleARefaire,
}

/// Ce que l'enregistrement d'une intervention a fait aux échéances, dans sa transaction.
#[derive(Debug, Clone, Default)]
pub struct Gestes {
    /// L'échéance passée à « traitée » et son type.
    pub soldee: Option<(i64, TypeEcheance)>,
    /// La suivante à proposer.
    pub suivante: Option<SaisieEcheance>,
    pub contre_visite: Option<SuiteContreVisite>,
}

/// Le prochain contrôle technique après une contre-visite favorable : deux ans après le contrôle
/// **initial**, d'où court la validité (hypothèse validée par Christophe le 16/10/2026, #107).
pub fn controle_apres_contre_visite(vehicule_id: i64, initial: NaiveDate) -> SaisieEcheance {
    let date_limite = initial
        .checked_add_months(chrono::Months::new(24))
        .unwrap_or(initial);
    SaisieEcheance {
        vehicule_id: vehicule_id.to_string(),
        type_echeance: TypeEcheance::ControleTechnique.code().to_string(),
        date_limite: date_limite.to_string(),
        statut: String::new(),
        ..Default::default()
    }
}

/// Dans la transaction qui enregistre l'intervention, ce qu'elle fait aux échéances du véhicule.
/// Une intervention datée dans le futur n'est pas faite : elle ne fait rien.
///
/// - **Contrôle technique défavorable ou critique** (#107, bloc 2) : crée l'échéance
///   « Contre-visite », deux mois après le contrôle, si ce délai n'est pas déjà passé (la saisie du
///   passé ne crée rien) ; une contre-visite encore ouverte est remplacée. L'échéance « Contrôle
///   technique » reste ouverte, masquée par le calcul tant que la contre-visite est à faire.
/// - **Contre-visite** (#107, bloc 3) : solde la contre-visite ouverte qu'elle remplit. Favorable
///   et dans les deux mois, elle solde aussi le « Contrôle technique » que le contrôle initial
///   remplissait, et propose le suivant deux ans après ce contrôle initial. Défavorable ou trop
///   tard, un contrôle complet est à refaire.
/// - **Contrôle technique favorable, révision** (#94, bloc 3) : solde l'échéance ouverte qu'elle
///   remplit ([`type_rempli`], [`echeance_remplie`]) et propose la suivante, datée de la visite ;
///   un contrôle favorable solde aussi une contre-visite encore ouverte. Sur un véhicule qui a un
///   plan d'entretien, le plan remplace l'« Entretien périodique » (#94, bloc 2b) : une révision
///   ne le touche pas, il reste masqué et revient si l'on retire le plan.
pub async fn solder_dans(
    connexion: &mut sqlx::PgConnection,
    intervention: &NouvelleIntervention,
    aujourd_hui: NaiveDate,
) -> Resultat<Gestes> {
    let mut gestes = Gestes::default();
    let date = intervention.date_intervention;
    if date > aujourd_hui {
        return Ok(gestes);
    }
    let vehicule_id = intervention.vehicule_id;
    let du_vehicule = echeances::lister_par_vehicule(&mut *connexion, vehicule_id).await?;
    let contre_visite_ouverte = du_vehicule
        .iter()
        .find(|e| e.est_ouverte() && e.type_echeance == TypeEcheance::ContreVisite.code());
    match (
        intervention.type_intervention,
        intervention.resultat_controle,
    ) {
        (
            TypeIntervention::ControleTechnique,
            Some(ResultatControle::Defavorable | ResultatControle::Critique),
        ) => {
            let date_limite = date_limite_contre_visite(date);
            if date_limite < aujourd_hui {
                return Ok(gestes);
            }
            if let Some(ancienne) = contre_visite_ouverte {
                echeances::changer_statut(&mut *connexion, ancienne.id, StatutEcheance::Traitee)
                    .await?;
            }
            echeances::inserer(
                &mut *connexion,
                &NouvelleEcheance {
                    vehicule_id,
                    type_echeance: TypeEcheance::ContreVisite,
                    date_limite,
                    statut: StatutEcheance::Ouverte,
                    operation: String::new(),
                    libelle: String::new(),
                    constat: String::new(),
                },
            )
            .await?;
            gestes.contre_visite = Some(SuiteContreVisite::AFaire(date_limite));
            return Ok(gestes);
        }
        (TypeIntervention::ContreVisite, resultat) => {
            let Some(contre_visite) =
                contre_visite_ouverte.filter(|e| remplit(e, date, aujourd_hui))
            else {
                return Ok(gestes);
            };
            echeances::changer_statut(&mut *connexion, contre_visite.id, StatutEcheance::Traitee)
                .await?;
            if resultat != Some(ResultatControle::Favorable) || date > contre_visite.date_limite {
                gestes.contre_visite = Some(SuiteContreVisite::ControleARefaire);
                return Ok(gestes);
            }
            let initial = donnees_interventions::dernier_controle_defavorable_avant(
                &mut *connexion,
                vehicule_id,
                date,
            )
            .await?
            .unwrap_or_else(|| {
                contre_visite
                    .date_limite
                    .checked_sub_months(chrono::Months::new(2))
                    .unwrap_or(contre_visite.date_limite)
            });
            if let Some(controle) = echeance_remplie(
                &du_vehicule,
                TypeEcheance::ControleTechnique,
                initial,
                aujourd_hui,
            ) {
                echeances::changer_statut(&mut *connexion, controle.id, StatutEcheance::Traitee)
                    .await?;
            }
            gestes.soldee = Some((contre_visite.id, TypeEcheance::ContreVisite));
            gestes.suivante = Some(controle_apres_contre_visite(vehicule_id, initial));
            return Ok(gestes);
        }
        _ => {}
    }
    let Some(type_echeance) = type_rempli(
        intervention.type_intervention,
        intervention.resultat_controle,
    ) else {
        return Ok(gestes);
    };
    if type_echeance == TypeEcheance::EntretienPeriodique
        && !donnees_plan::lister_par_vehicule(&mut *connexion, vehicule_id)
            .await?
            .is_empty()
    {
        return Ok(gestes);
    }
    if type_echeance == TypeEcheance::ControleTechnique {
        if let Some(contre_visite) = contre_visite_ouverte {
            echeances::changer_statut(&mut *connexion, contre_visite.id, StatutEcheance::Traitee)
                .await?;
        }
    }
    let Some(echeance) = echeance_remplie(&du_vehicule, type_echeance, date, aujourd_hui) else {
        return Ok(gestes);
    };
    echeances::changer_statut(&mut *connexion, echeance.id, StatutEcheance::Traitee).await?;
    gestes.soldee = Some((echeance.id, type_echeance));
    gestes.suivante = suivante_apres(echeance, date);
    Ok(gestes)
}

/// Vrai si une intervention faite le `date`, qui contient l'opération `operation`, remplit la
/// chose à faire (#107, bloc 4 ; #98) : l'échéance « À faire » est ouverte, porte la même
/// opération, et l'intervention est faite entre le jour où la chose a été notée et aujourd'hui.
/// Une intervention plus ancienne ne la remplit pas : des plaquettes changées l'an dernier ne
/// soldent pas celles qu'on vient de noter, et la saisie du passé ne solde rien. Une intervention
/// datée dans le futur n'est pas faite. « Autre » ne se reconnaît pas : son libellé est libre.
pub fn chose_faite(
    echeance: &Echeance,
    operation: Operation,
    date: NaiveDate,
    aujourd_hui: NaiveDate,
) -> bool {
    let notee_le = echeance.cree_le.with_timezone(&chrono::Local).date_naive();
    echeance.est_ouverte()
        && echeance.type_echeance == TypeEcheance::AFaire.code()
        && operation != Operation::Autre
        && echeance.operation == operation.code()
        && notee_le <= date
        && date <= aujourd_hui
}

/// Dans la transaction qui enregistre l'intervention, solde les choses à faire du véhicule
/// qu'elle remplit ([`chose_faite`]) : les plaquettes notées à changer après un contrôle
/// défavorable se soldent quand on enregistre l'intervention qui les change. Rend leur nombre.
pub async fn solder_choses_a_faire(
    connexion: &mut sqlx::PgConnection,
    intervention: &NouvelleIntervention,
    lignes: &[NouvelleLigne],
    aujourd_hui: NaiveDate,
) -> Resultat<usize> {
    let date = intervention.date_intervention;
    if lignes.is_empty() || date > aujourd_hui {
        return Ok(0);
    }
    let du_vehicule =
        echeances::lister_par_vehicule(&mut *connexion, intervention.vehicule_id).await?;
    let mut soldees = 0;
    for echeance in &du_vehicule {
        if lignes
            .iter()
            .any(|l| chose_faite(echeance, l.operation, date, aujourd_hui))
        {
            echeances::changer_statut(&mut *connexion, echeance.id, StatutEcheance::Traitee)
                .await?;
            soldees += 1;
        }
    }
    Ok(soldees)
}

/// Un défaut relevé au contrôle technique, à corriger avant la contre-visite (#107, bloc 4) : la
/// chose à faire que propose, sur la fiche, la ligne de la contre-visite ouverte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefautACorriger {
    /// La contre-visite ouverte : le lien se place sur sa ligne.
    pub echeance_id: i64,
    /// Celle de la contre-visite : le défaut se corrige avant d'y repasser.
    pub date_limite: NaiveDate,
    /// « Défaut relevé au contrôle technique du 21/10/2026 ».
    pub constat: String,
}

/// Ce que propose « Noter un défaut à corriger » : rien sans contre-visite ouverte. Le contrôle
/// nommé dans le constat est le dernier contrôle défavorable du véhicule avant la date limite de
/// la contre-visite, celui qui l'a créée ; sans lui (contre-visite saisie à la main), le constat
/// ne donne pas de date.
pub fn defaut_a_corriger(
    echeances: &[Echeance],
    interventions: &[Intervention],
) -> Option<DefautACorriger> {
    let contre_visite = echeances
        .iter()
        .find(|e| e.est_ouverte() && e.type_echeance == TypeEcheance::ContreVisite.code())?;
    let controle = interventions
        .iter()
        .filter(|i| {
            i.type_intervention == TypeIntervention::ControleTechnique.code()
                && matches!(
                    ResultatControle::depuis_code(&i.resultat_controle),
                    Some(ResultatControle::Defavorable | ResultatControle::Critique)
                )
                && i.date_intervention <= contre_visite.date_limite
        })
        .map(|i| i.date_intervention)
        .max();
    Some(DefautACorriger {
        echeance_id: contre_visite.id,
        date_limite: contre_visite.date_limite,
        constat: match controle {
            Some(date) => format!("Défaut relevé au contrôle technique du {}", date_fr(date)),
            None => "Défaut relevé au contrôle technique".to_string(),
        },
    })
}

/// Marque l'échéance comme traitée : elle disparaît des alertes.
pub async fn traiter(pool: &PgPool, id: i64) -> Resultat<Echeance> {
    if !echeances::changer_statut(pool, id, StatutEcheance::Traitee).await? {
        return Err(ErreurApp::Introuvable);
    }
    obtenir(pool, id).await
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if echeances::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statut_vide_vaut_ouverte() {
        let s = SaisieEcheance {
            vehicule_id: "1".into(),
            type_echeance: "assurance".into(),
            date_limite: "2027-01-01".into(),
            statut: "".into(),
            ..Default::default()
        };
        assert_eq!(valider(&s).unwrap().statut, StatutEcheance::Ouverte);
    }

    #[test]
    fn un_a_faire_n_a_pas_de_suivante_les_autres_si() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let mut e = echeance(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), "ouverte");
        e.type_echeance = "a_faire".into();
        e.operation = "balais".into();
        assert!(prochaine(&e, aujourd_hui).is_none());
        assert_eq!(periode_mois(TypeEcheance::AFaire), None);
        assert_eq!(periode_mois(TypeEcheance::ControleTechnique), Some(24));
        e.type_echeance = "assurance".into();
        e.operation.clear();
        assert!(prochaine(&e, aujourd_hui).is_some());
    }

    #[test]
    fn le_type_se_dit_en_toutes_lettres() {
        use crate::donnees::echeances::libelle_du_type;
        assert_eq!(libelle_du_type("assurance", "", ""), "Assurance");
        assert_eq!(
            libelle_du_type("a_faire", "filtre_air", ""),
            "À faire : Filtre à air"
        );
        assert_eq!(
            libelle_du_type("a_faire", "autre", "Câble d'embrayage"),
            "À faire : Câble d'embrayage"
        );
        // Un type que l'application ne connaît plus s'affiche tel quel, sans paniquer.
        assert_eq!(libelle_du_type("vignette", "", ""), "vignette");
    }

    #[test]
    fn la_saisie_d_un_a_faire_est_normalisee() {
        let mut s = SaisieEcheance {
            vehicule_id: "1".into(),
            type_echeance: "a_faire".into(),
            date_limite: "2027-01-01".into(),
            operation: " autre ".into(),
            libelle: "  Câble d'embrayage ".into(),
            constat: " câble qui accroche ".into(),
            ..Default::default()
        };
        let n = valider(&s).unwrap();
        assert_eq!(n.operation, "autre");
        assert_eq!(n.libelle, "Câble d'embrayage");
        assert_eq!(n.constat, "câble qui accroche");
        assert_eq!(n.type_libelle(), "À faire : Câble d'embrayage");

        // Une pièce du vocabulaire n'a pas besoin de libellé.
        s.operation = "vidange".into();
        s.libelle.clear();
        assert_eq!(valider(&s).unwrap().type_libelle(), "À faire : Vidange");
    }

    #[test]
    fn type_inconnu_refuse() {
        let s = SaisieEcheance {
            vehicule_id: "1".into(),
            type_echeance: "vignette".into(),
            date_limite: "2027-01-01".into(),
            statut: "".into(),
            ..Default::default()
        };
        assert!(valider(&s).is_err());
    }

    fn echeance(date_limite: NaiveDate, statut: &str) -> Echeance {
        Echeance {
            id: 1,
            vehicule_id: 1,
            type_echeance: "assurance".into(),
            date_limite,
            statut: statut.into(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AA-001-AA".into(),
            couleur: String::new(),
            operation: String::new(),
            libelle: String::new(),
            constat: String::new(),
        }
    }

    #[test]
    fn un_controle_favorable_et_une_revision_remplissent_leur_echeance() {
        use crate::commun::types::{ResultatControle as R, TypeIntervention as T};
        assert_eq!(
            type_rempli(T::ControleTechnique, Some(R::Favorable)),
            Some(TypeEcheance::ControleTechnique)
        );
        assert_eq!(
            type_rempli(T::ControleTechnique, Some(R::Defavorable)),
            None
        );
        assert_eq!(type_rempli(T::ControleTechnique, Some(R::Critique)), None);
        assert_eq!(type_rempli(T::ContreVisite, Some(R::Favorable)), None);
        assert_eq!(
            type_rempli(T::Revision, None),
            Some(TypeEcheance::EntretienPeriodique)
        );
        assert_eq!(type_rempli(T::EntretienCourant, None), None);
        assert_eq!(type_rempli(T::Reparation, None), None);
    }

    #[test]
    fn une_intervention_ne_remplit_que_l_echeance_de_sa_periode() {
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        let aujourd_hui = d(2026, 10, 23);
        let mut ct = echeance(d(2026, 11, 10), "ouverte");
        ct.type_echeance = "controle_technique".into();
        // Dans les deux ans qui précèdent la date limite : remplie.
        assert!(remplit(&ct, d(2026, 10, 20), aujourd_hui));
        assert!(remplit(&ct, d(2024, 11, 11), aujourd_hui));
        // La visite d'où part l'échéance, ou une plus ancienne : non.
        assert!(!remplit(&ct, d(2024, 11, 10), aujourd_hui));
        assert!(!remplit(&ct, d(2022, 5, 3), aujourd_hui));
        // En retard, après la date limite : remplie. Dans le futur : pas encore faite.
        assert!(remplit(&ct, d(2026, 12, 1), d(2026, 12, 1)));
        assert!(!remplit(&ct, d(2026, 10, 24), aujourd_hui));
        // Une échéance traitée ne se remplit plus.
        let mut traitee = ct.clone();
        traitee.statut = "traitee".into();
        assert!(!remplit(&traitee, d(2026, 10, 20), aujourd_hui));
        // L'entretien périodique compte un an ; un « à faire » n'a pas de période.
        let mut ep = ct.clone();
        ep.type_echeance = "entretien_periodique".into();
        assert!(remplit(&ep, d(2025, 11, 11), aujourd_hui));
        assert!(!remplit(&ep, d(2025, 11, 10), aujourd_hui));
        let mut a_faire = ct.clone();
        a_faire.type_echeance = "a_faire".into();
        assert!(!remplit(&a_faire, d(2026, 10, 20), aujourd_hui));
    }

    #[test]
    fn la_premiere_due_du_bon_type_est_remplie_et_la_suivante_part_de_la_visite() {
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        let aujourd_hui = d(2026, 10, 23);
        let une = |id, type_echeance: &str, date_limite| Echeance {
            id,
            type_echeance: type_echeance.into(),
            ..echeance(date_limite, "ouverte")
        };
        let du_vehicule = vec![
            une(1, "controle_technique", d(2026, 11, 20)),
            une(2, "entretien_periodique", d(2026, 11, 1)),
            une(3, "controle_technique", d(2026, 11, 10)),
        ];
        let remplie = echeance_remplie(
            &du_vehicule,
            TypeEcheance::ControleTechnique,
            d(2026, 10, 20),
            aujourd_hui,
        )
        .unwrap();
        assert_eq!(remplie.id, 3);
        let suivante = suivante_apres(remplie, d(2026, 10, 20)).unwrap();
        assert_eq!(suivante.type_echeance, "controle_technique");
        assert_eq!(suivante.date_limite, "2028-10-20");
        let ep = echeance_remplie(
            &du_vehicule,
            TypeEcheance::EntretienPeriodique,
            d(2026, 10, 20),
            aujourd_hui,
        )
        .unwrap();
        assert_eq!(
            suivante_apres(ep, d(2026, 10, 20)).unwrap().date_limite,
            "2027-10-20"
        );
        assert!(echeance_remplie(
            &du_vehicule,
            TypeEcheance::ControleTechnique,
            d(2023, 1, 5),
            aujourd_hui
        )
        .is_none());
    }

    #[test]
    fn la_contre_visite_a_deux_mois_pas_de_suivante_et_le_controle_part_de_l_initial() {
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        assert_eq!(periode_mois(TypeEcheance::ContreVisite), None);
        assert_eq!(fenetre_mois(TypeEcheance::ContreVisite), Some(2));
        let mut cv = echeance(d(2026, 12, 21), "ouverte");
        cv.type_echeance = "contre_visite".into();
        assert!(prochaine(&cv, d(2026, 10, 23)).is_none());
        // Remplie par une visite dans les deux mois qui précèdent sa limite, ou après ; pas par le
        // contrôle défavorable d'où elle part.
        assert!(remplit(&cv, d(2026, 11, 2), d(2026, 11, 2)));
        assert!(remplit(&cv, d(2026, 12, 28), d(2026, 12, 28)));
        assert!(!remplit(&cv, d(2026, 10, 21), d(2026, 11, 2)));
        let suivant = controle_apres_contre_visite(1, d(2026, 10, 21));
        assert_eq!(suivant.type_echeance, "controle_technique");
        assert_eq!(suivant.date_limite, "2028-10-21");
    }

    #[test]
    fn la_prochaine_echeance_garde_le_vehicule_et_le_type_et_decale_la_date() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let mut e = echeance(NaiveDate::from_ymd_opt(2027, 1, 31).unwrap(), "traitee");
        let suivante = prochaine(&e, aujourd_hui).unwrap();
        assert_eq!(suivante.vehicule_id, "1");
        assert_eq!(suivante.type_echeance, "assurance");
        assert_eq!(suivante.date_limite, "2028-01-31");
        assert_eq!(suivante.statut, "");

        // Deux ans pour un contrôle technique ; une fin de mois reste une fin de mois.
        e.type_echeance = "controle_technique".into();
        e.date_limite = NaiveDate::from_ymd_opt(2028, 2, 29).unwrap();
        assert_eq!(
            prochaine(&e, aujourd_hui).unwrap().date_limite,
            "2030-02-28"
        );
    }

    #[test]
    fn traitee_en_retard_la_prochaine_part_du_jour_sauf_pour_l_assurance() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let passee = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();

        // Contrôle technique passé avec du retard : deux ans après la visite, pas après
        // l'ancienne limite ; jamais une date déjà passée.
        let mut e = echeance(passee, "traitee");
        e.type_echeance = "controle_technique".into();
        assert_eq!(
            prochaine(&e, aujourd_hui).unwrap().date_limite,
            "2028-10-16"
        );
        e.type_echeance = "entretien_periodique".into();
        assert_eq!(
            prochaine(&e, aujourd_hui).unwrap().date_limite,
            "2027-10-16"
        );

        // L'assurance garde l'anniversaire du contrat, même traitée en retard.
        e.type_echeance = "assurance".into();
        assert_eq!(
            prochaine(&e, aujourd_hui).unwrap().date_limite,
            "2025-03-01"
        );

        // Le jour même de la limite, rien ne change.
        e.type_echeance = "controle_technique".into();
        e.date_limite = aujourd_hui;
        assert_eq!(
            prochaine(&e, aujourd_hui).unwrap().date_limite,
            "2028-10-16"
        );
    }

    #[test]
    fn etat_selon_la_date_et_le_statut() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let hier = aujourd_hui.pred_opt().unwrap();
        let demain = aujourd_hui.succ_opt().unwrap();

        // Comme sur la vue alertes, une échéance n'est dépassée que le lendemain de sa date, et
        // « à venir » jusqu'au dernier jour de l'horizon compris.
        let ouverte = |date| etat(&echeance(date, "ouverte"), aujourd_hui, 30);
        assert_eq!(ouverte(hier), EtatEcheance::Depassee);
        assert_eq!(ouverte(aujourd_hui), EtatEcheance::AVenir);
        assert_eq!(ouverte(demain), EtatEcheance::AVenir);
        let dernier_jour = jusqu_au(aujourd_hui, 30);
        assert_eq!(ouverte(dernier_jour), EtatEcheance::AVenir);
        assert_eq!(
            ouverte(dernier_jour.succ_opt().unwrap()),
            EtatEcheance::ATraiter
        );

        // Traitée, elle le reste quelle que soit sa date.
        let traitee = etat(&echeance(hier, "traitee"), aujourd_hui, 30);
        assert_eq!(traitee, EtatEcheance::Traitee);

        assert_eq!(EtatEcheance::Depassee.libelle(), "Dépassée");
        assert_eq!(EtatEcheance::AVenir.libelle(), "À venir");
        assert_eq!(EtatEcheance::ATraiter.libelle(), "À traiter");
        assert_eq!(EtatEcheance::Traitee.libelle(), "Traitée");
    }

    #[test]
    fn une_intervention_fait_la_chose_a_faire_de_meme_operation_notee_avant() {
        let aujourd_hui = crate::commun::dates::aujourd_hui();
        let hier = aujourd_hui.pred_opt().unwrap();
        let demain = aujourd_hui.succ_opt().unwrap();
        let mut e = echeance(demain, "ouverte");
        e.type_echeance = "a_faire".into();
        e.operation = "plaquettes".into();
        // Notée aujourd'hui (`cree_le` vaut maintenant) : faite par une intervention d'aujourd'hui.
        assert!(chose_faite(
            &e,
            Operation::Plaquettes,
            aujourd_hui,
            aujourd_hui
        ));
        // Une autre opération, une intervention d'avant la note, ou datée dans le futur : non.
        assert!(!chose_faite(
            &e,
            Operation::Disques,
            aujourd_hui,
            aujourd_hui
        ));
        assert!(!chose_faite(&e, Operation::Plaquettes, hier, aujourd_hui));
        assert!(!chose_faite(&e, Operation::Plaquettes, demain, aujourd_hui));
        // Déjà traitée, ou d'un autre type : non.
        e.statut = "traitee".into();
        assert!(!chose_faite(
            &e,
            Operation::Plaquettes,
            aujourd_hui,
            aujourd_hui
        ));
        e.statut = "ouverte".into();
        e.type_echeance = "assurance".into();
        assert!(!chose_faite(
            &e,
            Operation::Plaquettes,
            aujourd_hui,
            aujourd_hui
        ));
        // « Autre » ne se reconnaît pas, même libellé compris.
        e.type_echeance = "a_faire".into();
        e.operation = "autre".into();
        e.libelle = "Rétroviseur".into();
        assert!(!chose_faite(&e, Operation::Autre, aujourd_hui, aujourd_hui));
    }

    fn controle(id: i64, date: NaiveDate, resultat: &str) -> Intervention {
        Intervention {
            id,
            vehicule_id: 1,
            type_intervention: "controle_technique".into(),
            resultat_controle: resultat.into(),
            date_intervention: date,
            kilometrage: 0,
            cout_centimes: 0,
            prestataire: String::new(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: String::new(),
            couleur: String::new(),
            controle_initial: None,
            lignes: Vec::new(),
            documents: Vec::new(),
        }
    }

    #[test]
    fn le_defaut_a_corriger_se_propose_sur_la_contre_visite_ouverte() {
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        let mut cv = echeance(d(2026, 12, 21), "ouverte");
        cv.id = 7;
        cv.type_echeance = "contre_visite".into();
        let interventions = vec![
            controle(1, d(2024, 10, 2), "defavorable"),
            controle(2, d(2026, 10, 21), "defavorable"),
            controle(3, d(2026, 10, 1), "favorable"),
        ];
        assert_eq!(
            defaut_a_corriger(&[cv.clone()], &interventions),
            Some(DefautACorriger {
                echeance_id: 7,
                date_limite: d(2026, 12, 21),
                constat: "Défaut relevé au contrôle technique du 21/10/2026".into(),
            })
        );
        // Une contre-visite saisie à la main, sans contrôle défavorable : le constat sans date.
        assert_eq!(
            defaut_a_corriger(&[cv.clone()], &[]).unwrap().constat,
            "Défaut relevé au contrôle technique"
        );
        // Sans contre-visite ouverte : rien.
        cv.statut = "traitee".into();
        assert_eq!(defaut_a_corriger(&[cv], &interventions), None);
        assert_eq!(
            defaut_a_corriger(&[echeance(d(2026, 12, 21), "ouverte")], &interventions),
            None
        );
    }
}
