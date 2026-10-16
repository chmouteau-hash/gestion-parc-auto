//! Table `affectations` : quel conducteur a quel véhicule, et sur quelle période.
//!
//! Toutes les lectures joignent le véhicule et le conducteur : les listes ont besoin de
//! l'immatriculation et du nom, pas seulement des identifiants.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::date_fr;

/// Nom de l'index unique partiel qui interdit d'avoir deux affectations sans date de fin pour le
/// même conducteur sur le même véhicule. Sert à reconnaître la violation dans la couche métier.
pub const INDEX_PAS_DE_DOUBLON_EN_COURS: &str = "affectations_pas_de_doublon_en_cours";

/// État d'une affectation à une date donnée.
///
/// Il se calcule à partir de la période, il n'est pas stocké : une affectation datée passe
/// d'« à venir » à « en cours », puis à « terminée », sans que personne n'y touche. L'ordre des
/// variantes est l'ordre d'affichage des listes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Etat {
    /// La date tombe dans la période : à partir du jour de début, et avant le jour de fin.
    EnCours,
    /// La période n'a pas encore commencé.
    AVenir,
    /// Le jour de fin est atteint ou passé.
    Terminee,
}

impl Etat {
    pub fn libelle(self) -> &'static str {
        match self {
            Etat::EnCours => "En cours",
            Etat::AVenir => "À venir",
            Etat::Terminee => "Terminée",
        }
    }

    /// Classe CSS de la pastille.
    pub fn classe(self) -> &'static str {
        match self {
            Etat::EnCours => "etat-en_cours",
            Etat::AVenir => "etat-a_venir",
            Etat::Terminee => "etat-terminee",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Affectation {
    pub id: i64,
    pub vehicule_id: i64,
    pub conducteur_id: i64,
    pub date_debut: NaiveDate,
    /// Le jour où l'affectation s'arrête : ce jour-là, elle est déjà terminée.
    /// `None` = pas de date de fin prévue.
    pub date_fin: Option<NaiveDate>,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
    /// Couleur de repère du véhicule, pour sa plaque dans les listes.
    pub couleur: String,
    pub conducteur_nom: String,
    pub conducteur_prenom: String,
    /// Vide quand le service n'est pas renseigné.
    pub conducteur_service: String,
}

impl Affectation {
    /// État à la date donnée.
    ///
    /// L'affectation est en cours à partir de son jour de début, et jusqu'à la veille de son jour
    /// de fin. Le jour de fin est celui où elle s'arrête : c'est ce qui rend immédiat l'effet de
    /// « terminer aujourd'hui », d'une réforme ou d'une désactivation, qui posent la date du jour.
    pub fn etat(&self, aujourd_hui: NaiveDate) -> Etat {
        if aujourd_hui < self.date_debut {
            Etat::AVenir
        } else if self.date_fin.is_some_and(|fin| fin <= aujourd_hui) {
            Etat::Terminee
        } else {
            Etat::EnCours
        }
    }

    pub fn est_en_cours(&self, aujourd_hui: NaiveDate) -> bool {
        self.etat(aujourd_hui) == Etat::EnCours
    }

    /// Vrai si la période donnée a au moins un jour en commun avec celle de cette affectation.
    /// Une fin absente vaut « sans limite ». Deux périodes qui se suivent (l'une s'arrête le jour
    /// où l'autre commence) ne se chevauchent pas.
    pub fn chevauche(&self, debut: NaiveDate, fin: Option<NaiveDate>) -> bool {
        let commence_avant_la_fin = fin.is_none_or(|fin| self.date_debut < fin);
        let finit_apres_le_debut = self.date_fin.is_none_or(|fin| fin > debut);
        commence_avant_la_fin && finit_apres_le_debut
    }

    pub fn conducteur_nom_complet(&self) -> String {
        format!("{} {}", self.conducteur_prenom, self.conducteur_nom)
    }

    /// « , Service » à écrire à la suite du nom, ou rien : deux conducteurs de même nom se
    /// distinguent ainsi, comme dans la liste déroulante du formulaire. Le service est facultatif,
    /// et il n'existe pas dans le profil particulier (`avec_service` faux, voir
    /// `Contexte::montre_service`).
    pub fn conducteur_service_apres_le_nom(&self, avec_service: bool) -> String {
        if avec_service && !self.conducteur_service.is_empty() {
            format!(", {}", self.conducteur_service)
        } else {
            String::new()
        }
    }

    pub fn date_debut_fr(&self) -> String {
        date_fr(self.date_debut)
    }

    /// La date de fin, ou un tiret quand aucune n'est prévue.
    pub fn date_fin_fr(&self) -> String {
        self.date_fin.map_or_else(|| "—".to_string(), date_fr)
    }

    /// « depuis le 10/01/2026 » ou « du 10/01/2026 au 01/04/2026 ».
    pub fn periode_fr(&self) -> String {
        match self.date_fin {
            None => format!("depuis le {}", self.date_debut_fr()),
            Some(fin) => format!("du {} au {}", self.date_debut_fr(), date_fr(fin)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NouvelleAffectation {
    pub vehicule_id: i64,
    pub conducteur_id: i64,
    pub date_debut: NaiveDate,
    pub date_fin: Option<NaiveDate>,
}

const SELECTION: &str =
    "SELECT a.id, a.vehicule_id, a.conducteur_id, a.date_debut, a.date_fin, a.cree_le,
            v.immatriculation, v.couleur, c.nom AS conducteur_nom, c.prenom AS conducteur_prenom,
            c.service AS conducteur_service
     FROM affectations a
     JOIN vehicules v ON v.id = a.vehicule_id
     JOIN conducteurs c ON c.id = a.conducteur_id";

/// Toutes les affectations, de la plus récente à la plus ancienne. Le classement par état
/// (en cours d'abord) dépend de la date du jour : il est fait par la couche métier.
pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} ORDER BY a.date_debut DESC, a.id DESC"
    ))
    .fetch_all(ex)
    .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} WHERE a.vehicule_id = $1 ORDER BY a.date_debut DESC, a.id DESC"
    ))
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

pub async fn lister_par_conducteur(
    ex: impl PgExecutor<'_>,
    conducteur_id: i64,
) -> sqlx::Result<Vec<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} WHERE a.conducteur_id = $1 ORDER BY a.date_debut DESC, a.id DESC"
    ))
    .bind(conducteur_id)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!("{SELECTION} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

/// Toutes les affectations de ce conducteur sur ce véhicule, passées, en cours et à venir. Sert
/// à vérifier qu'une nouvelle période n'en chevauche aucune.
pub async fn lister_pour_couple(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
    conducteur_id: i64,
) -> sqlx::Result<Vec<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} WHERE a.vehicule_id = $1 AND a.conducteur_id = $2
         ORDER BY a.date_debut, a.id"
    ))
    .bind(vehicule_id)
    .bind(conducteur_id)
    .fetch_all(ex)
    .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, a: &NouvelleAffectation) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO affectations (vehicule_id, conducteur_id, date_debut, date_fin)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(a.vehicule_id)
    .bind(a.conducteur_id)
    .bind(a.date_debut)
    .bind(a.date_fin)
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    a: &NouvelleAffectation,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE affectations
         SET vehicule_id = $2, conducteur_id = $3, date_debut = $4, date_fin = $5
         WHERE id = $1",
    )
    .bind(id)
    .bind(a.vehicule_id)
    .bind(a.conducteur_id)
    .bind(a.date_debut)
    .bind(a.date_fin)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

/// Met fin à une affectation à la date donnée, si elle n'a pas de date de fin ou si sa fin
/// prévue est postérieure.
pub async fn cloturer(ex: impl PgExecutor<'_>, id: i64, date_fin: NaiveDate) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE affectations SET date_fin = $2
         WHERE id = $1 AND (date_fin IS NULL OR date_fin > $2)",
    )
    .bind(id)
    .bind(date_fin)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

/// Les affectations « encore ouvertes » à une date : sans date de fin, ou dont la fin prévue est
/// postérieure. La nouvelle date de fin ne précède jamais le début (la base le refuserait) : une
/// affectation qui n'a pas encore commencé se termine le jour de son début.
const CLOTURE_EN_LOT: &str = "UPDATE affectations
     SET date_fin = GREATEST(date_debut, $2)
     WHERE (date_fin IS NULL OR date_fin > $2) AND ";

/// Met fin, à la date donnée, à toutes les affectations encore ouvertes d'un véhicule.
/// Renvoie le nombre d'affectations clôturées.
pub async fn cloturer_pour_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
    date_fin: NaiveDate,
) -> sqlx::Result<u64> {
    let resultat = sqlx::query(&format!("{CLOTURE_EN_LOT} vehicule_id = $1"))
        .bind(vehicule_id)
        .bind(date_fin)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected())
}

/// Met fin, à la date donnée, à toutes les affectations encore ouvertes d'un conducteur.
/// Renvoie le nombre d'affectations clôturées.
pub async fn cloturer_pour_conducteur(
    ex: impl PgExecutor<'_>,
    conducteur_id: i64,
    date_fin: NaiveDate,
) -> sqlx::Result<u64> {
    let resultat = sqlx::query(&format!("{CLOTURE_EN_LOT} conducteur_id = $1"))
        .bind(conducteur_id)
        .bind(date_fin)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected())
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM affectations WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
