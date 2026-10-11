//! Interventions d'entretien : validation et opérations.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::TypeIntervention;
use crate::donnees::interventions::{self, Intervention, NouvelleIntervention};
use crate::donnees::vehicules;
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::validation::{
    centimes_en_saisie, code_parmi, date_obligatoire, entier_positif, identifiant,
    montant_en_centimes, notes, texte_facultatif,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieIntervention {
    #[serde(default)]
    pub vehicule_id: String,
    #[serde(default)]
    pub type_intervention: String,
    #[serde(default)]
    pub date_intervention: String,
    #[serde(default)]
    pub kilometrage: String,
    /// Montant en euros (`149,90`), converti en centimes à la validation.
    #[serde(default)]
    pub cout: String,
    #[serde(default)]
    pub prestataire: String,
    #[serde(default)]
    pub notes: String,
}

impl SaisieIntervention {
    pub fn depuis(i: &Intervention) -> SaisieIntervention {
        SaisieIntervention {
            vehicule_id: i.vehicule_id.to_string(),
            type_intervention: i.type_intervention.clone(),
            date_intervention: i.date_intervention.to_string(),
            kilometrage: i.kilometrage.to_string(),
            cout: centimes_en_saisie(i.cout_centimes),
            prestataire: i.prestataire.clone(),
            notes: i.notes.clone(),
        }
    }
}

pub fn valider(s: &SaisieIntervention) -> Resultat<NouvelleIntervention> {
    Ok(NouvelleIntervention {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_intervention: code_parmi(&s.type_intervention, "type", TypeIntervention::depuis_code)?,
        date_intervention: date_obligatoire(&s.date_intervention, "date")?,
        kilometrage: entier_positif(&s.kilometrage, "kilométrage")?,
        cout_centimes: montant_en_centimes(&s.cout, "coût")?,
        prestataire: texte_facultatif(&s.prestataire, "prestataire")?,
        notes: notes(&s.notes, "notes")?,
    })
}

fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Intervention>> {
    Ok(interventions::lister(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Intervention>> {
    Ok(interventions::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Intervention> {
    interventions::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

/// Enregistre l'intervention et relève le kilométrage courant du véhicule si celui de
/// l'intervention est plus grand. Les deux écritures sont dans une même transaction : soit les
/// deux réussissent, soit aucune.
pub async fn creer(pool: &PgPool, saisie: &SaisieIntervention) -> Resultat<Intervention> {
    let nouvelle = valider(saisie)?;
    let mut transaction = pool.begin().await?;
    let id = interventions::inserer(&mut *transaction, &nouvelle)
        .await
        .map_err(traduire)?;
    vehicules::relever_kilometrage(
        &mut *transaction,
        nouvelle.vehicule_id,
        nouvelle.kilometrage,
    )
    .await?;
    transaction.commit().await?;
    obtenir(pool, id).await
}

pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieIntervention,
) -> Resultat<Intervention> {
    let nouvelle = valider(saisie)?;
    let mut transaction = pool.begin().await?;
    if !interventions::mettre_a_jour(&mut *transaction, id, &nouvelle)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    vehicules::relever_kilometrage(
        &mut *transaction,
        nouvelle.vehicule_id,
        nouvelle.kilometrage,
    )
    .await?;
    transaction.commit().await?;
    obtenir(pool, id).await
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if interventions::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cout_en_euros_devient_centimes() {
        let s = SaisieIntervention {
            vehicule_id: "3".into(),
            type_intervention: "revision".into(),
            date_intervention: "2026-04-02".into(),
            kilometrage: "61000".into(),
            cout: "249,90".into(),
            ..Default::default()
        };
        let i = valider(&s).unwrap();
        assert_eq!(i.cout_centimes, 24990);
        assert_eq!(i.type_intervention, TypeIntervention::Revision);
        assert_eq!(i.prestataire, "");
    }

    #[test]
    fn vehicule_non_choisi_refuse() {
        let s = SaisieIntervention {
            type_intervention: "revision".into(),
            date_intervention: "2026-04-02".into(),
            kilometrage: "61000".into(),
            ..Default::default()
        };
        assert!(valider(&s).is_err());
    }
}
