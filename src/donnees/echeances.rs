//! Table `echeances` : dates limites réglementaires ou d'entretien d'un véhicule.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::date_fr;
use crate::commun::types::{Operation, StatutEcheance, TypeEcheance};

/// Nom de l'index unique partiel qui interdit deux échéances ouvertes de même véhicule, même type,
/// même date limite et même chose à faire. Sert à reconnaître la violation dans la couche métier.
pub const INDEX_PAS_DE_DOUBLON_OUVERTE: &str = "echeances_pas_de_doublon_ouverte";

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Echeance {
    pub id: i64,
    pub vehicule_id: i64,
    pub type_echeance: String,
    pub date_limite: NaiveDate,
    pub statut: String,
    pub cree_le: DateTime<Utc>,
    /// Pour un « à faire » : le code de l'opération (`Operation`) ; vide pour les autres types.
    pub operation: String,
    /// Pour un « à faire » d'opération « autre » : ce qu'il y a à faire ; vide sinon.
    pub libelle: String,
    /// Pour un « à faire » : pourquoi (« câble qui accroche ») ; vide sinon.
    pub constat: String,
    pub immatriculation: String,
    /// Couleur de repère du véhicule, pour sa plaque dans les listes.
    pub couleur: String,
}

/// Ce qu'il y a à faire, en toutes lettres : « Filtre à air », ou le libellé saisi pour « autre ».
pub fn quoi(operation: &str, libelle: &str) -> String {
    match Operation::depuis_code(operation) {
        Some(Operation::Autre) | None if !libelle.is_empty() => libelle.to_string(),
        Some(valeur) => valeur.libelle().to_string(),
        None => operation.to_string(),
    }
}

/// Le type d'une échéance tel que l'écran le dit : « Assurance », ou « À faire : Filtre à air ».
/// Même texte partout (fiche, liste, alertes, accueil, impression, export) : il n'est écrit qu'ici.
pub fn libelle_du_type(type_echeance: &str, operation: &str, libelle: &str) -> String {
    let type_lu = TypeEcheance::depuis_code(type_echeance);
    match type_lu {
        Some(TypeEcheance::AFaire) => format!(
            "{} : {}",
            TypeEcheance::AFaire.libelle(),
            quoi(operation, libelle)
        ),
        Some(valeur) => valeur.libelle().to_string(),
        None => type_echeance.to_string(),
    }
}

impl Echeance {
    /// « Contrôle technique », « À faire : Câble d'embrayage »…
    pub fn type_libelle(&self) -> String {
        libelle_du_type(&self.type_echeance, &self.operation, &self.libelle)
    }

    /// Vrai pour une échéance « à faire » : elle n'a pas de suivante.
    pub fn est_a_faire(&self) -> bool {
        self.type_echeance == TypeEcheance::AFaire.code()
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
    /// Code d'`Operation` pour un « à faire », vide sinon.
    pub operation: String,
    /// Ce qu'il y a à faire quand l'opération est « autre », vide sinon.
    pub libelle: String,
    /// Le constat d'un « à faire », vide sinon.
    pub constat: String,
}

impl NouvelleEcheance {
    /// Le type en toutes lettres, comme l'écran le dit (voir [`libelle_du_type`]).
    pub fn type_libelle(&self) -> String {
        libelle_du_type(self.type_echeance.code(), &self.operation, &self.libelle)
    }
}

const SELECTION: &str = "SELECT e.*, v.immatriculation, v.couleur
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

/// Toutes les échéances ouvertes, de la plus proche à la plus lointaine : l'accueil en tire la
/// prochaine de chaque véhicule.
pub async fn lister_ouvertes(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Echeance>> {
    sqlx::query_as::<_, Echeance>(&format!(
        "{SELECTION} WHERE e.statut = 'ouverte' ORDER BY e.date_limite, e.id"
    ))
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
        "INSERT INTO echeances
            (vehicule_id, type_echeance, date_limite, statut, operation, libelle, constat)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(e.vehicule_id)
    .bind(e.type_echeance.code())
    .bind(e.date_limite)
    .bind(e.statut.code())
    .bind(&e.operation)
    .bind(&e.libelle)
    .bind(&e.constat)
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
         SET vehicule_id = $2, type_echeance = $3, date_limite = $4, statut = $5,
             operation = $6, libelle = $7, constat = $8
         WHERE id = $1",
    )
    .bind(id)
    .bind(e.vehicule_id)
    .bind(e.type_echeance.code())
    .bind(e.date_limite)
    .bind(e.statut.code())
    .bind(&e.operation)
    .bind(&e.libelle)
    .bind(&e.constat)
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
