//! Table `echeances` : dates limites réglementaires ou d'entretien d'un véhicule.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::date_fr;
use crate::commun::types::{StatutEcheance, TypeEcheance};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Echeance {
    pub id: i64,
    pub vehicule_id: i64,
    pub type_echeance: String,
    pub date_limite: NaiveDate,
    pub statut: String,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
}

impl Echeance {
    pub fn type_libelle(&self) -> &str {
        match TypeEcheance::depuis_code(&self.type_echeance) {
            Some(valeur) => valeur.libelle(),
            None => &self.type_echeance,
        }
    }

    pub fn statut_libelle(&self) -> &str {
        match StatutEcheance::depuis_code(&self.statut) {
            Some(valeur) => valeur.libelle(),
            None => &self.statut,
        }
    }

    pub fn est_ouverte(&self) -> bool {
        self.statut == StatutEcheance::Ouverte.code()
    }

    pub fn date_limite_fr(&self) -> String {
        date_fr(self.date_limite)
    }
}

#[derive(Debug, Clone)]
pub struct NouvelleEcheance {
    pub vehicule_id: i64,
    pub type_echeance: TypeEcheance,
    pub date_limite: NaiveDate,
    pub statut: StatutEcheance,
}

const SELECTION: &str = "SELECT e.*, v.immatriculation
     FROM echeances e
     JOIN vehicules v ON v.id = e.vehicule_id";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Echeance>> {
    sqlx::query_as::<_, Echeance>(&format!(
        "{SELECTION} ORDER BY e.statut = 'traitee', e.date_limite, e.id"
    ))
    .fetch_all(ex)
    .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Echeance>> {
    sqlx::query_as::<_, Echeance>(&format!(
        "{SELECTION} WHERE e.vehicule_id = $1 ORDER BY e.statut = 'traitee', e.date_limite, e.id"
    ))
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

/// Échéances ouvertes dont la date limite est au plus tard `jusqu_au` (dépassées comprises).
pub async fn lister_ouvertes_jusqu_au(
    ex: impl PgExecutor<'_>,
    jusqu_au: NaiveDate,
) -> sqlx::Result<Vec<Echeance>> {
    sqlx::query_as::<_, Echeance>(&format!(
        "{SELECTION} WHERE e.statut = 'ouverte' AND e.date_limite <= $1 ORDER BY e.date_limite, e.id"
    ))
    .bind(jusqu_au)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Echeance>> {
    sqlx::query_as::<_, Echeance>(&format!("{SELECTION} WHERE e.id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, e: &NouvelleEcheance) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO echeances (vehicule_id, type_echeance, date_limite, statut)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(e.vehicule_id)
    .bind(e.type_echeance.code())
    .bind(e.date_limite)
    .bind(e.statut.code())
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    e: &NouvelleEcheance,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE echeances
         SET vehicule_id = $2, type_echeance = $3, date_limite = $4, statut = $5
         WHERE id = $1",
    )
    .bind(id)
    .bind(e.vehicule_id)
    .bind(e.type_echeance.code())
    .bind(e.date_limite)
    .bind(e.statut.code())
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn changer_statut(
    ex: impl PgExecutor<'_>,
    id: i64,
    statut: StatutEcheance,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query("UPDATE echeances SET statut = $2 WHERE id = $1")
        .bind(id)
        .bind(statut.code())
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM echeances WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
