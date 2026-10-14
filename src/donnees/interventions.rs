//! Table `interventions` : entretien et réparations.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{centimes_fr, date_fr, nombre_fr};
use crate::commun::types::TypeIntervention;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Intervention {
    pub id: i64,
    pub vehicule_id: i64,
    pub type_intervention: String,
    pub date_intervention: NaiveDate,
    pub kilometrage: i32,
    pub cout_centimes: i64,
    pub prestataire: String,
    pub notes: String,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
}

impl Intervention {
    pub fn type_libelle(&self) -> &str {
        match TypeIntervention::depuis_code(&self.type_intervention) {
            Some(valeur) => valeur.libelle(),
            None => &self.type_intervention,
        }
    }

    pub fn date_intervention_fr(&self) -> String {
        date_fr(self.date_intervention)
    }

    pub fn cout_fr(&self) -> String {
        centimes_fr(self.cout_centimes)
    }

    /// `45000` → `45 000 km`.
    pub fn kilometrage_fr(&self) -> String {
        format!("{} km", nombre_fr(i64::from(self.kilometrage)))
    }
}

#[derive(Debug, Clone)]
pub struct NouvelleIntervention {
    pub vehicule_id: i64,
    pub type_intervention: TypeIntervention,
    pub date_intervention: NaiveDate,
    pub kilometrage: i32,
    pub cout_centimes: i64,
    pub prestataire: String,
    pub notes: String,
}

const SELECTION: &str = "SELECT i.*, v.immatriculation
     FROM interventions i
     JOIN vehicules v ON v.id = i.vehicule_id";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} ORDER BY i.date_intervention DESC, i.id DESC"
    ))
    .fetch_all(ex)
    .await
}

/// Les `limite` interventions les plus récentes, pour le tableau de bord.
pub async fn lister_recentes(
    ex: impl PgExecutor<'_>,
    limite: i64,
) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} ORDER BY i.date_intervention DESC, i.id DESC LIMIT $1"
    ))
    .bind(limite)
    .fetch_all(ex)
    .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} WHERE i.vehicule_id = $1 ORDER BY i.date_intervention DESC, i.id DESC"
    ))
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!("{SELECTION} WHERE i.id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, i: &NouvelleIntervention) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO interventions
            (vehicule_id, type_intervention, date_intervention, kilometrage, cout_centimes,
             prestataire, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(i.vehicule_id)
    .bind(i.type_intervention.code())
    .bind(i.date_intervention)
    .bind(i.kilometrage)
    .bind(i.cout_centimes)
    .bind(&i.prestataire)
    .bind(&i.notes)
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    i: &NouvelleIntervention,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE interventions
         SET vehicule_id = $2, type_intervention = $3, date_intervention = $4, kilometrage = $5,
             cout_centimes = $6, prestataire = $7, notes = $8
         WHERE id = $1",
    )
    .bind(id)
    .bind(i.vehicule_id)
    .bind(i.type_intervention.code())
    .bind(i.date_intervention)
    .bind(i.kilometrage)
    .bind(i.cout_centimes)
    .bind(&i.prestataire)
    .bind(&i.notes)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM interventions WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
