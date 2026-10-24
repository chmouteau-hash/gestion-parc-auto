//! La base répond-elle ? Une requête sans table, pour la sonde de l'hébergement.

use sqlx::PgExecutor;

/// Un aller-retour avec PostgreSQL : `SELECT 1`. Réussit si la connexion s'obtient et répond.
pub async fn ping(ex: impl PgExecutor<'_>) -> sqlx::Result<()> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(ex)
        .await
        .map(|_| ())
}
