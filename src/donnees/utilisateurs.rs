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
    /// Faux : le compte ne peut plus se connecter, sa session s'arrête ; il n'est pas supprimé.
    pub actif: bool,
    /// L'adresse de courriel du compte, vide tant qu'il n'en a pas donné (#182).
    pub courriel: String,
    /// Vrai : le compte reçoit le rappel du matin à son adresse.
    pub rappels: bool,
}

/// Règle l'adresse de courriel d'un compte et s'il reçoit le rappel. Renvoie `true` si le
/// compte existe.
pub async fn definir_courriel(
    ex: impl PgExecutor<'_>,
    id: i64,
    courriel: &str,
    rappels: bool,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query("UPDATE utilisateurs SET courriel = $2, rappels = $3 WHERE id = $1")
        .bind(id)
        .bind(courriel)
        .bind(rappels)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

/// Les adresses des comptes actifs qui ont demandé le rappel du matin (#182), par identifiant.
pub async fn destinataires_rappels(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT courriel FROM utilisateurs
         WHERE actif AND rappels AND courriel <> ''
         ORDER BY login",
    )
    .fetch_all(ex)
    .await
}

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Utilisateur>> {
    sqlx::query_as::<_, Utilisateur>("SELECT * FROM utilisateurs ORDER BY login")
        .fetch_all(ex)
        .await
}

/// Active ou désactive un compte. Renvoie `true` si le compte existe.
pub async fn definir_actif(ex: impl PgExecutor<'_>, id: i64, actif: bool) -> sqlx::Result<bool> {
    let resultat = sqlx::query("UPDATE utilisateurs SET actif = $2 WHERE id = $1")
        .bind(id)
        .bind(actif)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn compter_administrateurs_actifs(ex: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM utilisateurs WHERE role = 'administrateur' AND actif",
    )
    .fetch_one(ex)
    .await
}

/// Remplace l'empreinte du mot de passe d'un compte désigné par son numéro.
pub async fn changer_mot_de_passe_par_id(
    ex: impl PgExecutor<'_>,
    id: i64,
    mot_de_passe_hash: &str,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query("UPDATE utilisateurs SET mot_de_passe_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(mot_de_passe_hash)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
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

/// Remplace l'empreinte du mot de passe d'un compte. Renvoie `true` si le compte existe.
pub async fn changer_mot_de_passe(
    ex: impl PgExecutor<'_>,
    login: &str,
    mot_de_passe_hash: &str,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query("UPDATE utilisateurs SET mot_de_passe_hash = $2 WHERE login = $1")
        .bind(login)
        .bind(mot_de_passe_hash)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn compter(ex: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM utilisateurs")
        .fetch_one(ex)
        .await
}
