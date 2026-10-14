//! Table `conducteurs` : personnes physiques, données minimisées (voir docs/exploitation.md).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::date_fr;
use crate::commun::types::StatutConducteur;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Conducteur {
    pub id: i64,
    pub nom: String,
    pub prenom: String,
    pub service: String,
    pub date_expiration_permis: NaiveDate,
    pub statut: String,
    pub cree_le: DateTime<Utc>,
    pub modifie_le: DateTime<Utc>,
}

impl Conducteur {
    pub fn nom_complet(&self) -> String {
        format!("{} {}", self.prenom, self.nom)
    }

    /// « CM » pour Claire Martin : la première lettre du prénom puis du nom, en capitales.
    pub fn initiales(&self) -> String {
        [&self.prenom, &self.nom]
            .iter()
            .filter_map(|mot| mot.trim().chars().next())
            .flat_map(char::to_uppercase)
            .collect()
    }

    pub fn est_actif(&self) -> bool {
        self.statut == StatutConducteur::Actif.code()
    }

    /// Le permis est expiré quand sa date d'expiration est passée. Il reste valable le jour même,
    /// comme une échéance ne devient « dépassée » que le lendemain de sa date limite.
    pub fn permis_expire(&self, aujourd_hui: NaiveDate) -> bool {
        self.date_expiration_permis < aujourd_hui
    }

    pub fn statut_libelle(&self) -> &str {
        match StatutConducteur::depuis_code(&self.statut) {
            Some(valeur) => valeur.libelle(),
            None => &self.statut,
        }
    }

    pub fn date_expiration_permis_fr(&self) -> String {
        date_fr(self.date_expiration_permis)
    }
}

#[derive(Debug, Clone)]
pub struct NouveauConducteur {
    pub nom: String,
    pub prenom: String,
    pub service: String,
    pub date_expiration_permis: NaiveDate,
    pub statut: StatutConducteur,
}

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Conducteur>> {
    sqlx::query_as::<_, Conducteur>("SELECT * FROM conducteurs ORDER BY nom, prenom")
        .fetch_all(ex)
        .await
}

/// Conducteurs dont le nom, le prénom ou le service contient le motif (`ILIKE`, déjà encadré de
/// `%` et échappé par la couche métier).
pub async fn rechercher(
    ex: impl PgExecutor<'_>,
    motif: &str,
    limite: i64,
) -> sqlx::Result<Vec<Conducteur>> {
    sqlx::query_as::<_, Conducteur>(
        "SELECT * FROM conducteurs
         WHERE nom ILIKE $1 OR prenom ILIKE $1 OR service ILIKE $1
         ORDER BY nom, prenom LIMIT $2",
    )
    .bind(motif)
    .bind(limite)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Conducteur>> {
    sqlx::query_as::<_, Conducteur>("SELECT * FROM conducteurs WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, c: &NouveauConducteur) -> sqlx::Result<Conducteur> {
    sqlx::query_as::<_, Conducteur>(
        "INSERT INTO conducteurs (nom, prenom, service, date_expiration_permis, statut)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(&c.nom)
    .bind(&c.prenom)
    .bind(&c.service)
    .bind(c.date_expiration_permis)
    .bind(c.statut.code())
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    c: &NouveauConducteur,
) -> sqlx::Result<Option<Conducteur>> {
    sqlx::query_as::<_, Conducteur>(
        "UPDATE conducteurs
         SET nom = $2, prenom = $3, service = $4, date_expiration_permis = $5, statut = $6,
             modifie_le = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&c.nom)
    .bind(&c.prenom)
    .bind(&c.service)
    .bind(c.date_expiration_permis)
    .bind(c.statut.code())
    .fetch_optional(ex)
    .await
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM conducteurs WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

/// Conducteurs inactifs dont la dernière modification est antérieure à la date donnée
/// (candidats à la purge RGPD).
pub async fn lister_inactifs_avant(
    ex: impl PgExecutor<'_>,
    modifie_avant: DateTime<Utc>,
) -> sqlx::Result<Vec<Conducteur>> {
    sqlx::query_as::<_, Conducteur>(
        "SELECT * FROM conducteurs
         WHERE statut = 'inactif' AND modifie_le < $1
         ORDER BY id",
    )
    .bind(modifie_avant)
    .fetch_all(ex)
    .await
}
