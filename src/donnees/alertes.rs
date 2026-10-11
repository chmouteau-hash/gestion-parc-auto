//! Requêtes de la vue alertes : ce qui arrive à échéance avant une date donnée.

use chrono::NaiveDate;
use sqlx::PgExecutor;

use super::conducteurs::Conducteur;
use super::echeances::Echeance;

/// Échéances ouvertes (dépassées ou à venir) jusqu'à la date donnée incluse.
pub async fn echeances_a_traiter(
    ex: impl PgExecutor<'_>,
    jusqu_au: NaiveDate,
) -> sqlx::Result<Vec<Echeance>> {
    super::echeances::lister_ouvertes_jusqu_au(ex, jusqu_au).await
}

/// Conducteurs actifs dont le permis expire au plus tard à la date donnée (expirés compris).
pub async fn conducteurs_permis_expirant(
    ex: impl PgExecutor<'_>,
    jusqu_au: NaiveDate,
) -> sqlx::Result<Vec<Conducteur>> {
    sqlx::query_as::<_, Conducteur>(
        "SELECT * FROM conducteurs
         WHERE statut = 'actif' AND date_expiration_permis <= $1
         ORDER BY date_expiration_permis, nom, prenom",
    )
    .bind(jusqu_au)
    .fetch_all(ex)
    .await
}
