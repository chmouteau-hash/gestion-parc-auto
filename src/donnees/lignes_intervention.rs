//! Table `lignes_intervention` : ce qui a été fait pendant une intervention, opération par
//! opération (vidange, filtre à huile, bougies…), avec la pièce, sa quantité et son prix.

use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{centimes_fr, quantite_fr};
use crate::commun::types::Operation;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct LigneIntervention {
    pub id: i64,
    pub intervention_id: i64,
    pub operation: String,
    /// Ce qu'est l'opération quand elle est « autre » ; vide sinon.
    pub libelle: String,
    pub reference_piece: String,
    /// En centièmes : `540` = 5,4 (litres d'huile, kilos de gaz…), comme le prix en centimes.
    pub quantite_centiemes: i32,
    pub prix_centimes: i64,
}

impl LigneIntervention {
    /// « Filtre à huile », ou le libellé saisi pour une opération « autre ».
    pub fn operation_libelle(&self) -> &str {
        match Operation::depuis_code(&self.operation) {
            Some(Operation::Autre) if !self.libelle.is_empty() => &self.libelle,
            Some(valeur) => valeur.libelle(),
            None => &self.operation,
        }
    }

    /// La ligne en un mot ou quelques-uns : « Filtre à huile (réf. W712, × 2, 25,00 €) ». Les
    /// précisions ne s'écrivent que si elles ont été saisies ; une quantité de 1 va sans dire,
    /// une quantité décimale s'écrit « × 5,4 ».
    pub fn resume(&self) -> String {
        let mut precisions = Vec::new();
        if !self.reference_piece.is_empty() {
            precisions.push(format!("réf. {}", self.reference_piece));
        }
        if self.quantite_centiemes != 100 {
            precisions.push(format!("× {}", quantite_fr(self.quantite_centiemes)));
        }
        if self.prix_centimes > 0 {
            precisions.push(centimes_fr(self.prix_centimes));
        }
        if precisions.is_empty() {
            self.operation_libelle().to_string()
        } else {
            format!("{} ({})", self.operation_libelle(), precisions.join(", "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NouvelleLigne {
    pub operation: Operation,
    pub libelle: String,
    pub reference_piece: String,
    /// En centièmes : `540` = 5,4 (litres d'huile, kilos de gaz…), comme le prix en centimes.
    pub quantite_centiemes: i32,
    pub prix_centimes: i64,
}

const SELECTION: &str =
    "SELECT id, intervention_id, operation, libelle, reference_piece, quantite_centiemes, prix_centimes
     FROM lignes_intervention";

/// Les lignes des interventions données, dans l'ordre du vocabulaire des opérations
/// (« autre » en dernier), puis de saisie. Un seul aller-retour pour toute une liste.
pub async fn lister_par_interventions(
    ex: impl PgExecutor<'_>,
    intervention_ids: &[i64],
) -> sqlx::Result<Vec<LigneIntervention>> {
    sqlx::query_as::<_, LigneIntervention>(&format!(
        "{SELECTION} WHERE intervention_id = ANY($1)
         ORDER BY intervention_id, array_position($2::text[], operation), id"
    ))
    .bind(intervention_ids)
    .bind(Operation::TOUS.iter().map(|o| o.code()).collect::<Vec<_>>())
    .fetch_all(ex)
    .await
}

pub async fn inserer(
    ex: impl PgExecutor<'_>,
    intervention_id: i64,
    l: &NouvelleLigne,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO lignes_intervention
            (intervention_id, operation, libelle, reference_piece, quantite_centiemes, prix_centimes)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(intervention_id)
    .bind(l.operation.code())
    .bind(&l.libelle)
    .bind(&l.reference_piece)
    .bind(l.quantite_centiemes)
    .bind(l.prix_centimes)
    .fetch_one(ex)
    .await
}

/// Retire toutes les lignes d'une intervention : la modification remplace l'ensemble.
pub async fn supprimer_par_intervention(
    ex: impl PgExecutor<'_>,
    intervention_id: i64,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM lignes_intervention WHERE intervention_id = $1")
        .bind(intervention_id)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(operation: &str) -> LigneIntervention {
        LigneIntervention {
            id: 1,
            intervention_id: 1,
            operation: operation.into(),
            libelle: String::new(),
            reference_piece: String::new(),
            quantite_centiemes: 100,
            prix_centimes: 0,
        }
    }

    #[test]
    fn le_resume_ne_dit_que_ce_qui_a_ete_saisi() {
        assert_eq!(ligne("vidange").resume(), "Vidange");

        let mut l = ligne("filtre_huile");
        l.reference_piece = "W712".into();
        l.quantite_centiemes = 200;
        l.prix_centimes = 2500;
        assert_eq!(
            l.resume(),
            "Filtre à huile (réf. W712, × 2, 25,00\u{202F}€)"
        );

        let mut l = ligne("pneus");
        l.quantite_centiemes = 400;
        assert_eq!(l.resume(), "Pneus (× 4)");

        let mut l = ligne("vidange");
        l.quantite_centiemes = 540;
        l.prix_centimes = 6750;
        assert_eq!(l.resume(), "Vidange (× 5,4, 67,50\u{202F}€)");
    }

    #[test]
    fn une_operation_autre_porte_son_libelle() {
        let mut l = ligne("autre");
        l.libelle = "Joint de culasse".into();
        l.prix_centimes = 4990;
        assert_eq!(l.resume(), "Joint de culasse (49,90\u{202F}€)");
    }
}
