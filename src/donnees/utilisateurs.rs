//! Table `utilisateurs` : comptes de connexion à l'application.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Utilisateur {
    pub id: i64,
    pub login: String,
    /// Empreinte Argon2id, jamais le mot de passe lui-même.
    pub mot_de_passe_hash: String,
    pub role: String,
    pub cree_le: DateTime<Utc>,
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Utilisateur>> {
    sqlx::query_as::<_, Utilisateur>("SELECT * FROM utilisateurs WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn obtenir_par_login(
    ex: impl PgExecutor<'_>,
    login: &str,
) -> sqlx::Result<Option<Utilisateur>> {
    sqlx::query_as::<_, Utilisateur>("SELECT * FROM utilisateurs WHERE login = $1")
        .bind(login)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(
    ex: impl PgExecutor<'_>,
    login: &str,
    mot_de_passe_hash: &str,
    role: &str,
) -> sqlx::Result<Utilisateur> {
    sqlx::query_as::<_, Utilisateur>(
        "INSERT INTO utilisateurs (login, mot_de_passe_hash, role)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(login)
    .bind(mot_de_passe_hash)
    .bind(role)
    .fetch_one(ex)
    .await
}

pub async fn compter(ex: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM utilisateurs")
        .fetch_one(ex)
        .await
}
