//! Table `releves_kilometrage` : les relevés du compteur faits à la main (#94, bloc 4). Avec les
//! interventions et les pleins, ils forment l'historique du compteur d'un véhicule.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{date_fr, nombre_fr};
use crate::commun::types::{TypeDepense, TypeIntervention};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Releve {
    pub id: i64,
    pub vehicule_id: i64,
    pub date_releve: NaiveDate,
    pub kilometrage: i32,
    pub cree_le: DateTime<Utc>,
}

/// Une ligne de l'historique du compteur : un relevé, une intervention, ou une dépense qui porte
/// un kilométrage (un plein). `origine` vaut `releve`, `intervention` ou `depense` ; `detail` est
/// le type de l'intervention ou de la dépense, vide pour un relevé. `id` est celui de la ligne
/// dans sa propre table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct LigneHistorique {
    pub origine: String,
    pub id: i64,
    pub date: NaiveDate,
    pub kilometrage: i32,
    pub detail: String,
}

impl LigneHistorique {
    pub fn est_un_releve(&self) -> bool {
        self.origine == "releve"
    }

    pub fn date_fr(&self) -> String {
        date_fr(self.date)
    }

    pub fn kilometrage_fr(&self) -> String {
        format!("{} km", nombre_fr(i64::from(self.kilometrage)))
    }

    /// « Relevé », « Intervention · Révision », « Plein ».
    pub fn origine_fr(&self) -> String {
        match self.origine.as_str() {
            "releve" => "Relevé".to_string(),
            "intervention" => format!(
                "Intervention · {}",
                TypeIntervention::depuis_code(&self.detail)
                    .map(TypeIntervention::libelle)
                    .unwrap_or(self.detail.as_str())
            ),
            _ => match TypeDepense::depuis_code(&self.detail) {
                Some(TypeDepense::Carburant) => "Plein".to_string(),
                Some(autre) => format!("Dépense · {}", autre.libelle()),
                None => self.detail.clone(),
            },
        }
    }

    /// « le relevé du 12/10/2026 », « l'intervention du 12/10/2026 », « le plein du 12/10/2026 » :
    /// d'où vient le compteur.
    pub fn d_ou_fr(&self) -> String {
        let quoi = match self.origine.as_str() {
            "releve" => "le relevé",
            "intervention" => "l'intervention",
            _ if self.detail == TypeDepense::Carburant.code() => "le plein",
            _ => "la dépense",
        };
        format!("{quoi} du {}", self.date_fr())
    }

    /// Là où la ligne se corrige : le relevé ici, l'intervention ou la dépense dans son formulaire.
    pub fn adresse_modification(&self) -> String {
        match self.origine.as_str() {
            "releve" => format!("/releves/{}/modifier", self.id),
            "intervention" => format!("/interventions/{}/modifier", self.id),
            _ => format!("/depenses/{}/modifier", self.id),
        }
    }
}

pub async fn inserer(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
    date_releve: NaiveDate,
    kilometrage: i32,
) -> sqlx::Result<Releve> {
    sqlx::query_as::<_, Releve>(
        "INSERT INTO releves_kilometrage (vehicule_id, date_releve, kilometrage)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(vehicule_id)
    .bind(date_releve)
    .bind(kilometrage)
    .fetch_one(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Releve>> {
    sqlx::query_as::<_, Releve>("SELECT * FROM releves_kilometrage WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await
}

/// Change la date et le kilométrage du relevé ; il reste sur son véhicule.
pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    date_releve: NaiveDate,
    kilometrage: i32,
) -> sqlx::Result<Option<Releve>> {
    sqlx::query_as::<_, Releve>(
        "UPDATE releves_kilometrage SET date_releve = $2, kilometrage = $3
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(date_releve)
    .bind(kilometrage)
    .fetch_optional(ex)
    .await
}

/// Supprime le relevé et rend son véhicule, dont le compteur est à recalculer ; `None` s'il
/// n'existe pas.
pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar::<_, i64>(
        "DELETE FROM releves_kilometrage WHERE id = $1 RETURNING vehicule_id",
    )
    .bind(id)
    .fetch_optional(ex)
    .await
}

/// L'historique du compteur d'un véhicule, le plus récent en premier : ses relevés, ses
/// interventions et ses dépenses qui portent un kilométrage. Lu dans les trois tables à chaque
/// fois : rien n'est recopié.
pub async fn historique(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<LigneHistorique>> {
    sqlx::query_as::<_, LigneHistorique>(
        "SELECT 'releve' AS origine, id, date_releve AS date, kilometrage, '' AS detail
           FROM releves_kilometrage WHERE vehicule_id = $1
         UNION ALL
         SELECT 'intervention', id, date_intervention, kilometrage, type_intervention
           FROM interventions WHERE vehicule_id = $1
         UNION ALL
         SELECT 'depense', id, date_depense, kilometrage, type_depense
           FROM depenses WHERE vehicule_id = $1 AND kilometrage IS NOT NULL
         ORDER BY date DESC, kilometrage DESC, id DESC",
    )
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}
