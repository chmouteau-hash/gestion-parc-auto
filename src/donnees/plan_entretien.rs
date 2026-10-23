//! Table `plan_entretien` : les opérations suivies d'un véhicule et leur périodicité (#94,
//! bloc 2). La dernière fois et la prochaine n'y sont pas : elles se calculent
//! (`metier::plan_entretien`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::nombre_fr;
use crate::commun::types::Operation;

/// L'index qui interdit de suivre deux fois la même opération sur un véhicule.
pub const INDEX_UNE_LIGNE_PAR_OPERATION: &str = "plan_entretien_une_ligne_par_operation";

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct LignePlan {
    pub id: i64,
    pub vehicule_id: i64,
    /// Code du vocabulaire [`Operation`], jamais « autre ».
    pub operation: String,
    /// Tous les combien de kilomètres ; absent = pas de borne en kilomètres.
    pub periodicite_km: Option<i32>,
    /// Tous les combien de mois ; absent = pas de borne en date.
    pub periodicite_mois: Option<i32>,
    pub cree_le: DateTime<Utc>,
}

impl LignePlan {
    pub fn operation_libelle(&self) -> &str {
        match Operation::depuis_code(&self.operation) {
            Some(valeur) => valeur.libelle(),
            None => &self.operation,
        }
    }

    /// « tous les 15 000 km ou tous les 12 mois », « tous les mois »…
    pub fn periodicite_fr(&self) -> String {
        periodicite_fr(self.periodicite_km, self.periodicite_mois)
    }
}

/// La périodicité en clair : « tous les 15 000 km ou tous les 12 mois », « tous les 10 000 km »,
/// « tous les mois ».
pub fn periodicite_fr(km: Option<i32>, mois: Option<i32>) -> String {
    let km = km.map(|k| format!("tous les {} km", nombre_fr(i64::from(k))));
    let mois = mois.map(|m| match m {
        1 => "tous les mois".to_string(),
        m => format!("tous les {m} mois"),
    });
    match (km, mois) {
        (Some(k), Some(m)) => format!("{k} ou {m}"),
        (Some(k), None) => k,
        (None, Some(m)) => m,
        (None, None) => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NouvelleLignePlan {
    pub vehicule_id: i64,
    pub operation: Operation,
    pub periodicite_km: Option<i32>,
    pub periodicite_mois: Option<i32>,
}

const SELECTION: &str =
    "SELECT id, vehicule_id, operation, periodicite_km, periodicite_mois, cree_le
     FROM plan_entretien";

/// Les opérations suivies du véhicule, dans l'ordre du vocabulaire des opérations : celui du
/// formulaire d'intervention.
pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<LignePlan>> {
    sqlx::query_as::<_, LignePlan>(&format!(
        "{SELECTION} WHERE vehicule_id = $1 ORDER BY array_position($2::text[], operation), id"
    ))
    .bind(vehicule_id)
    .bind(Operation::TOUS.iter().map(|o| o.code()).collect::<Vec<_>>())
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<LignePlan>> {
    sqlx::query_as::<_, LignePlan>(&format!("{SELECTION} WHERE id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, l: &NouvelleLignePlan) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO plan_entretien (vehicule_id, operation, periodicite_km, periodicite_mois)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(l.vehicule_id)
    .bind(l.operation.code())
    .bind(l.periodicite_km)
    .bind(l.periodicite_mois)
    .fetch_one(ex)
    .await
}

/// Change l'opération et la périodicité ; la ligne reste sur son véhicule.
pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    l: &NouvelleLignePlan,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE plan_entretien
         SET operation = $2, periodicite_km = $3, periodicite_mois = $4
         WHERE id = $1",
    )
    .bind(id)
    .bind(l.operation.code())
    .bind(l.periodicite_km)
    .bind(l.periodicite_mois)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM plan_entretien WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_periodicite_se_lit_en_clair() {
        assert_eq!(
            periodicite_fr(Some(15000), Some(12)),
            "tous les 15\u{202F}000 km ou tous les 12 mois"
        );
        assert_eq!(
            periodicite_fr(Some(10000), None),
            "tous les 10\u{202F}000 km"
        );
        assert_eq!(periodicite_fr(None, Some(1)), "tous les mois");
        assert_eq!(periodicite_fr(None, Some(24)), "tous les 24 mois");
    }
}
