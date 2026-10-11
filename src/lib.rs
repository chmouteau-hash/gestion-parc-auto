//! Gestion du parc automobile : bibliothèque applicative.
//!
//! Le code est organisé en trois couches qui ne dépendent que dans un sens,
//! `web → metier → donnees` (voir docs/architecture.md) :
//!
//! - [`web`]     : routes HTTP, session, rendu des pages HTML et de l'API JSON ;
//! - [`metier`]  : règles de gestion et validation des saisies ;
//! - [`donnees`] : requêtes SQL paramétrées vers PostgreSQL.
//!
//! Le programme `src/main.rs` ne fait que lire la configuration et démarrer le serveur ; tout le
//! reste est ici, ce qui permet aux tests d'intégration (`tests/`) d'utiliser le code réel.

pub mod commun;
pub mod config;
pub mod donnees;
pub mod erreurs;
pub mod etat;
pub mod metier;
pub mod web;

/// Applique les migrations manquantes du dossier `migrations/`.
///
/// Les fichiers SQL sont embarqués dans l'exécutable à la compilation (`sqlx::migrate!`), donc le
/// programme n'a pas besoin du dossier à côté de lui pour démarrer. Une migration déjà appliquée
/// est ignorée : la fonction est rejouable sans effet.
pub async fn migrer(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
