//! État de santé du serveur, pour la sonde de l'hébergement (#63, bloc 2).

use std::time::Duration;

use sqlx::PgPool;

use crate::donnees::sante;

/// Au-delà, la base est considérée injoignable : une sonde qui attend trente secondes ne sert
/// à rien, et la page « mise à jour en cours » de l'hôte doit s'afficher vite.
pub const DELAI_BASE: Duration = Duration::from_secs(2);

/// Ce que la sonde constate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sante {
    /// Le serveur tourne et la base a répondu dans le délai.
    Ok,
    /// La base a refusé, coupé, ou n'a pas répondu dans le délai.
    BaseInjoignable,
}

/// Interroge la base, sans jamais attendre plus de [`DELAI_BASE`].
pub async fn verifier(pool: &PgPool) -> Sante {
    match tokio::time::timeout(DELAI_BASE, sante::ping(pool)).await {
        Ok(Ok(())) => Sante::Ok,
        Ok(Err(erreur)) => {
            tracing::warn!("sonde /health : la base ne répond pas ({erreur})");
            Sante::BaseInjoignable
        }
        Err(_) => {
            tracing::warn!(
                "sonde /health : la base n'a pas répondu en {} s",
                DELAI_BASE.as_secs()
            );
            Sante::BaseInjoignable
        }
    }
}
