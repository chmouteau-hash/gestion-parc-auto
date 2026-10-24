//! Table `vehicules`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{date_fr, nombre_fr};
use crate::commun::types::{Carburant, CarburantPlein, Couleur, CritAir, StatutVehicule};

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
    /// Numéro d'identification du véhicule (case E de la carte grise), 17 caractères ; vide tant
    /// qu'il n'est pas renseigné.
    pub vin: String,
    /// Couleur de repère (`commun::types::Couleur`) ; vide tant qu'aucune n'est choisie.
    pub couleur: String,
    /// Vignette Crit'Air (`commun::types::CritAir`), choisie d'après le certificat ; vide tant
    /// qu'aucune n'est choisie (#165).
    pub crit_air: String,
    pub cree_le: DateTime<Utc>,
    pub modifie_le: DateTime<Utc>,
}

impl Vehicule {
    /// « Rouge », ou vide quand aucune couleur n'est choisie.
    pub fn couleur_libelle(&self) -> &str {
        match Couleur::depuis_code(&self.couleur) {
            Some(valeur) => valeur.libelle(),
            None => "",
        }
    }

    /// La vignette Crit'Air choisie ; `None` tant qu'aucune ne l'est.
    pub fn crit_air(&self) -> Option<CritAir> {
        CritAir::depuis_code(&self.crit_air)
    }

    /// La vignette que l'application propose quand aucune n'est choisie, d'après le carburant et
    /// la date de mise en service (`CritAir::propose_pour`). `None` dès qu'une est choisie : le
    /// choix prime toujours.
    pub fn crit_air_propose(&self) -> Option<CritAir> {
        if !self.crit_air.is_empty() {
            return None;
        }
        CritAir::propose_pour(
            Carburant::depuis_code(&self.carburant)?,
            self.date_mise_en_service,
        )
    }

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

    /// Le code du carburant proposé d'office pour un plein de ce véhicule (#126) : gazole pour
    /// un diesel, SP95-E10 pour une essence… ; « autre » si le carburant du véhicule est inconnu.
    pub fn carburant_de_plein(&self) -> &'static str {
        CarburantPlein::propose_pour(
            Carburant::depuis_code(&self.carburant).unwrap_or(Carburant::Autre),
        )
        .code()
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
    /// Validé par `metier::validation::vin` : vide, ou 17 caractères en majuscules.
    pub vin: String,
    /// Vide, ou un code de `Couleur`.
    pub couleur: String,
    /// Vide, ou un code de `CritAir`.
    pub crit_air: String,
}

/// Nom de l'index unique sur le VIN : sert à reconnaître un doublon dans la couche métier.
pub const INDEX_VIN_UNIQUE: &str = "vehicules_vin_unique";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Vehicule>> {
    sqlx::query_as::<_, Vehicule>("SELECT * FROM vehicules ORDER BY immatriculation")
        .fetch_all(ex)
        .await
}

/// Véhicules dont l'immatriculation, la marque, le modèle ou le VIN contient le motif, sans tenir
/// compte de la casse. Le motif est un motif `ILIKE` déjà encadré de `%` et échappé par la
/// couche métier.
pub async fn rechercher(
    ex: impl PgExecutor<'_>,
    motif: &str,
    limite: i64,
) -> sqlx::Result<Vec<Vehicule>> {
    sqlx::query_as::<_, Vehicule>(
        "SELECT * FROM vehicules
         WHERE immatriculation ILIKE $1 OR marque ILIKE $1 OR modele ILIKE $1 OR vin ILIKE $1
         ORDER BY immatriculation LIMIT $2",
    )
    .bind(motif)
    .bind(limite)
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
            (immatriculation, marque, modele, date_mise_en_service, carburant, kilometrage, statut,
             vin, couleur, crit_air)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING *",
    )
    .bind(&v.immatriculation)
    .bind(&v.marque)
    .bind(&v.modele)
    .bind(v.date_mise_en_service)
    .bind(v.carburant.code())
    .bind(v.kilometrage)
    .bind(v.statut.code())
    .bind(&v.vin)
    .bind(&v.couleur)
    .bind(&v.crit_air)
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
             carburant = $6, statut = $7, vin = $8, couleur = $9, crit_air = $10,
             modifie_le = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&v.immatriculation)
    .bind(&v.marque)
    .bind(&v.modele)
    .bind(v.date_mise_en_service)
    .bind(v.carburant.code())
    .bind(v.statut.code())
    .bind(&v.vin)
    .bind(&v.couleur)
    .bind(&v.crit_air)
    .fetch_optional(ex)
    .await
}

/// Repasse « en service » un véhicule réformé. Sans effet sur un véhicule qui ne l'est pas : la
/// condition est dans la requête, pas dans une lecture d'avant, donc deux administrateurs qui
/// cliquent en même temps ne s'écrasent pas. `None` : pas de véhicule, ou pas réformé.
pub async fn remettre_en_service(
    ex: impl PgExecutor<'_>,
    id: i64,
) -> sqlx::Result<Option<Vehicule>> {
    sqlx::query_as::<_, Vehicule>(
        "UPDATE vehicules
         SET statut = $2, modifie_le = now()
         WHERE id = $1 AND statut = $3
         RETURNING *",
    )
    .bind(id)
    .bind(StatutVehicule::EnService.code())
    .bind(StatutVehicule::Reforme.code())
    .fetch_optional(ex)
    .await
}

/// Le compteur du véhicule devient le plus haut kilométrage de son historique : ses relevés, ses
/// interventions et ses dépenses (#94, bloc 4). À appeler dans la transaction de chaque écriture
/// de l'une de ces sources : corriger ou supprimer la saisie fautive fait redescendre le compteur
/// (#4). Sans aucun historique, il vaut 0.
pub async fn recalculer_kilometrage(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE vehicules
         SET kilometrage = h.kilometrage, modifie_le = now()
         FROM (SELECT COALESCE(GREATEST(
                  (SELECT MAX(kilometrage) FROM releves_kilometrage WHERE vehicule_id = $1),
                  (SELECT MAX(kilometrage) FROM interventions WHERE vehicule_id = $1),
                  (SELECT MAX(kilometrage) FROM depenses WHERE vehicule_id = $1)
               ), 0) AS kilometrage) h
         WHERE vehicules.id = $1 AND vehicules.kilometrage <> h.kilometrage",
    )
    .bind(id)
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

/// Ce qu'une suppression emporte avec le véhicule (#60) : compté juste avant, pour le journal.
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct Dependances {
    pub affectations: i64,
    pub interventions: i64,
    pub echeances: i64,
    pub depenses: i64,
    pub documents: i64,
    pub releves: i64,
    pub plan: i64,
}

pub async fn compter_dependances(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Dependances> {
    sqlx::query_as::<_, Dependances>(
        "SELECT (SELECT count(*) FROM affectations WHERE vehicule_id = $1) AS affectations,
                (SELECT count(*) FROM interventions WHERE vehicule_id = $1) AS interventions,
                (SELECT count(*) FROM echeances WHERE vehicule_id = $1) AS echeances,
                (SELECT count(*) FROM depenses WHERE vehicule_id = $1) AS depenses,
                (SELECT count(*) FROM documents WHERE vehicule_id = $1) AS documents,
                (SELECT count(*) FROM releves_kilometrage WHERE vehicule_id = $1) AS releves,
                (SELECT count(*) FROM plan_entretien WHERE vehicule_id = $1) AS plan",
    )
    .bind(id)
    .fetch_one(ex)
    .await
}
