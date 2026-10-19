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
use crate::commun::types::{Operation, StatutEcheance, TypeEcheance};
use crate::donnees::echeances::{self, Echeance, NouvelleEcheance, INDEX_PAS_DE_DOUBLON_OUVERTE};
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
}

impl EtatEcheance {
    pub fn libelle(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => Niveau::Depassee.libelle(),
            EtatEcheance::AVenir => Niveau::AVenir.libelle(),
            EtatEcheance::ATraiter => StatutEcheance::Ouverte.libelle(),
            EtatEcheance::Traitee => StatutEcheance::Traitee.libelle(),
        }
    }

    /// Classe CSS de la pastille.
    pub fn classe(self) -> &'static str {
        match self {
            EtatEcheance::Depassee => "echeance-depassee",
            EtatEcheance::AVenir => "echeance-a_venir",
            EtatEcheance::ATraiter => "echeance-a_traiter",
            EtatEcheance::Traitee => "echeance-traitee",
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
/// le 14/10/2026, issue #39, bloc 5). Un « à faire » se fait une fois : pas de suivante.
pub fn periode_mois(type_echeance: TypeEcheance) -> Option<u32> {
    match type_echeance {
        TypeEcheance::ControleTechnique => Some(24),
        TypeEcheance::Assurance | TypeEcheance::EntretienPeriodique => Some(12),
        TypeEcheance::AFaire => None,
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
/// jour du clic. La vraie date de réalisation viendra avec les interventions (#94, bloc 3).
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
}
