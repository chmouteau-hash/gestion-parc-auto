//! Table `vehicules`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{date_fr, nombre_fr};
use crate::commun::types::{Carburant, StatutVehicule};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Vehicule {
    pub id: i64,
    pub immatriculation: String,
    pub marque: String,
    pub modele: String,
    pub date_mise_en_service: NaiveDate,
    pub carburant: String,
    pub kilometrage: i32,
    pub statut: String,
    pub cree_le: DateTime<Utc>,
    pub modifie_le: DateTime<Utc>,
}

impl Vehicule {
    /// « AB-123-CD (Renault Clio) », pour les listes déroulantes et les titres.
    pub fn designation(&self) -> String {
        format!("{} ({} {})", self.immatriculation, self.marque, self.modele)
    }

    /// Un véhicule réformé est sorti du parc : il ne reçoit plus d'affectation.
    pub fn est_reforme(&self) -> bool {
        self.statut == StatutVehicule::Reforme.code()
    }

    pub fn statut_libelle(&self) -> &str {
        match StatutVehicule::depuis_code(&self.statut) {
            Some(valeur) => valeur.libelle(),
            None => &self.statut,
        }
    }

    pub fn carburant_libelle(&self) -> &str {
        match Carburant::depuis_code(&self.carburant) {
            Some(valeur) => valeur.libelle(),
            None => &self.carburant,
        }
    }

    pub fn date_mise_en_service_fr(&self) -> String {
        date_fr(self.date_mise_en_service)
    }

    /// `160000` → `160 000 km`.
    pub fn kilometrage_fr(&self) -> String {
        format!("{} km", nombre_fr(i64::from(self.kilometrage)))
    }

    /// Un véhicule en service est au parc, disponible ; en maintenance ou réformé, il ne l'est pas.
    pub fn est_en_service(&self) -> bool {
        self.statut == StatutVehicule::EnService.code()
    }

    pub fn est_en_maintenance(&self) -> bool {
        self.statut == StatutVehicule::EnMaintenance.code()
    }
}

/// Saisie validée par la couche métier, prête à écrire.
#[derive(Debug, Clone)]
pub struct NouveauVehicule {
    pub immatriculation: String,
    pub marque: String,
    pub modele: String,
    pub date_mise_en_service: NaiveDate,
    pub carburant: Carburant,
    pub kilometrage: i32,
    pub statut: StatutVehicule,
}

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Vehicule>> {
    sqlx::query_as::<_, Vehicule>("SELECT * FROM vehicules ORDER BY immatriculation")
        .fetch_all(ex)
        .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Vehicule>> {
    sqlx::query_as::<_, Vehicule>("SELECT * FROM vehicules WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, v: &NouveauVehicule) -> sqlx::Result<Vehicule> {
    sqlx::query_as::<_, Vehicule>(
        "INSERT INTO vehicules
            (immatriculation, marque, modele, date_mise_en_service, carburant, kilometrage, statut)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(&v.immatriculation)
    .bind(&v.marque)
    .bind(&v.modele)
    .bind(v.date_mise_en_service)
    .bind(v.carburant.code())
    .bind(v.kilometrage)
    .bind(v.statut.code())
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    v: &NouveauVehicule,
) -> sqlx::Result<Option<Vehicule>> {
    sqlx::query_as::<_, Vehicule>(
        "UPDATE vehicules
         SET immatriculation = $2, marque = $3, modele = $4, date_mise_en_service = $5,
             carburant = $6, kilometrage = $7, statut = $8, modifie_le = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&v.immatriculation)
    .bind(&v.marque)
    .bind(&v.modele)
    .bind(v.date_mise_en_service)
    .bind(v.carburant.code())
    .bind(v.kilometrage)
    .bind(v.statut.code())
    .fetch_optional(ex)
    .await
}

/// Relève le kilométrage courant s'il est inférieur à la valeur donnée (jamais de retour en
/// arrière : un compteur ne recule pas).
pub async fn relever_kilometrage(
    ex: impl PgExecutor<'_>,
    id: i64,
    kilometrage: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE vehicules
         SET kilometrage = GREATEST(kilometrage, $2), modifie_le = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(kilometrage)
    .execute(ex)
    .await?;
    Ok(())
}

/// Renvoie `true` si une ligne a été supprimée. L'historique lié part avec elle (ON DELETE CASCADE).
pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM vehicules WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
