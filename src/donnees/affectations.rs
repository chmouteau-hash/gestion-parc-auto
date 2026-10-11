//! Table `affectations` : quel conducteur a quel véhicule, et depuis quand.
//!
//! Toutes les lectures joignent le véhicule et le conducteur : les listes ont besoin de
//! l'immatriculation et du nom, pas seulement des identifiants.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::date_fr;

/// Nom de l'index unique partiel qui tient la règle « une seule affectation active par véhicule ».
/// Sert à reconnaître la violation dans la couche métier.
pub const INDEX_UNE_SEULE_ACTIVE: &str = "affectations_une_seule_active_par_vehicule";

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Affectation {
    pub id: i64,
    pub vehicule_id: i64,
    pub conducteur_id: i64,
    pub date_debut: NaiveDate,
    /// `None` = affectation en cours.
    pub date_fin: Option<NaiveDate>,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
    pub conducteur_nom: String,
    pub conducteur_prenom: String,
}

impl Affectation {
    pub fn est_active(&self) -> bool {
        self.date_fin.is_none()
    }

    pub fn conducteur_nom_complet(&self) -> String {
        format!("{} {}", self.conducteur_prenom, self.conducteur_nom)
    }

    pub fn date_debut_fr(&self) -> String {
        date_fr(self.date_debut)
    }

    pub fn date_fin_fr(&self) -> String {
        self.date_fin
            .map_or_else(|| "en cours".to_string(), date_fr)
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
            v.immatriculation, c.nom AS conducteur_nom, c.prenom AS conducteur_prenom
     FROM affectations a
     JOIN vehicules v ON v.id = a.vehicule_id
     JOIN conducteurs c ON c.id = a.conducteur_id";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} ORDER BY a.date_fin IS NOT NULL, a.date_debut DESC, a.id DESC"
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

/// L'affectation en cours d'un véhicule, s'il y en a une (au plus une, par construction).
pub async fn active_pour_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Option<Affectation>> {
    sqlx::query_as::<_, Affectation>(&format!(
        "{SELECTION} WHERE a.vehicule_id = $1 AND a.date_fin IS NULL"
    ))
    .bind(vehicule_id)
    .fetch_optional(ex)
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

/// Clôture une affectation en cours à la date donnée.
pub async fn cloturer(ex: impl PgExecutor<'_>, id: i64, date_fin: NaiveDate) -> sqlx::Result<bool> {
    let resultat =
        sqlx::query("UPDATE affectations SET date_fin = $2 WHERE id = $1 AND date_fin IS NULL")
            .bind(id)
            .bind(date_fin)
            .execute(ex)
            .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM affectations WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
