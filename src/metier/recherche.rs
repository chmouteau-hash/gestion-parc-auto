//! Recherche libre : une immatriculation, une marque, un modèle, un VIN, un nom de conducteur.
//!
//! Le motif est cherché « contenu dans », sans tenir compte de la casse, dans les colonnes de
//! texte des véhicules et des conducteurs. Les résultats sont plafonnés : une recherche sert à
//! retrouver une fiche, pas à lister le parc.

use sqlx::PgPool;

use crate::donnees::conducteurs::{self, Conducteur};
use crate::donnees::vehicules::{self, Vehicule};
use crate::erreurs::Resultat;

/// Au-delà, c'est la liste qu'il faut ouvrir.
pub const LIMITE: i64 = 50;

#[derive(Debug, Default)]
pub struct Resultats {
    /// La question, nettoyée de ses espaces autour.
    pub question: String,
    pub vehicules: Vec<Vehicule>,
    pub conducteurs: Vec<Conducteur>,
}

/// Le motif `ILIKE` : la question encadrée de `%`, ses caractères spéciaux (`%`, `_`, `\`)
/// échappés pour être cherchés tels quels.
pub fn motif(question: &str) -> String {
    let mut motif = String::with_capacity(question.len() + 2);
    motif.push('%');
    for c in question.trim().chars() {
        if matches!(c, '%' | '_' | '\\') {
            motif.push('\\');
        }
        motif.push(c);
    }
    motif.push('%');
    motif
}

pub async fn chercher(pool: &PgPool, question: &str) -> Resultat<Resultats> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Ok(Resultats::default());
    }
    let motif = motif(&question);
    let vehicules = vehicules::rechercher(pool, &motif, LIMITE).await?;
    let conducteurs = conducteurs::rechercher(pool, &motif, LIMITE).await?;
    Ok(Resultats {
        question,
        vehicules,
        conducteurs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_motif_encadre_et_echappe() {
        assert_eq!(motif("  clio "), "%clio%");
        assert_eq!(motif("100%"), "%100\\%%");
        assert_eq!(motif("a_b\\c"), "%a\\_b\\\\c%");
    }
}
