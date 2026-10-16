//! Table `interventions` : entretien et réparations.

use chrono::{DateTime, Months, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{centimes_fr, date_fr, nombre_fr};
use crate::commun::types::{ResultatControle, TypeIntervention};
use crate::donnees::documents::Document;
use crate::donnees::lignes_intervention::LigneIntervention;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Intervention {
    pub id: i64,
    pub vehicule_id: i64,
    pub type_intervention: String,
    /// Pour un contrôle technique ou une contre-visite : `favorable`, `defavorable`, `critique`
    /// (#107). Vide pour les autres types, et pour les contrôles saisis avant que le résultat
    /// existe.
    pub resultat_controle: String,
    pub date_intervention: NaiveDate,
    pub kilometrage: i32,
    pub cout_centimes: i64,
    pub prestataire: String,
    pub notes: String,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
    /// Pour une contre-visite : la date du dernier contrôle technique défavorable du véhicule qui
    /// la précède, **calculée** par la couche métier, jamais stockée. Elle dit si la contre-visite
    /// est dans les deux mois.
    #[sqlx(skip)]
    #[serde(default)]
    pub controle_initial: Option<NaiveDate>,
    /// Ce qui a été fait (table `lignes_intervention`). La requête ne les lit pas : la couche
    /// métier les attache après coup, en une requête pour toute une liste.
    #[sqlx(skip)]
    #[serde(default)]
    pub lignes: Vec<LigneIntervention>,
    /// Les documents joints à l'intervention, sa facture le plus souvent (#100). Attachés après
    /// coup, comme les lignes.
    #[sqlx(skip)]
    #[serde(default)]
    pub documents: Vec<Document>,
}

impl Intervention {
    /// Les opérations en une ligne, « Vidange · Filtre à huile (réf. W712, 12,50 €) » ; vide
    /// quand rien n'a été détaillé.
    pub fn operations_fr(&self) -> String {
        self.lignes
            .iter()
            .map(LigneIntervention::resume)
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn type_libelle(&self) -> &str {
        match TypeIntervention::depuis_code(&self.type_intervention) {
            Some(valeur) => valeur.libelle(),
            None => &self.type_intervention,
        }
    }

    pub fn date_intervention_fr(&self) -> String {
        date_fr(self.date_intervention)
    }

    pub fn cout_fr(&self) -> String {
        centimes_fr(self.cout_centimes)
    }

    /// `45000` → `45 000 km`.
    pub fn kilometrage_fr(&self) -> String {
        format!("{} km", nombre_fr(i64::from(self.kilometrage)))
    }

    /// Le résultat en clair, avec ce qu'il implique : « Défavorable : contre-visite avant le
    /// 20/03/2026 ». Rien pour une intervention qui n'est pas un contrôle, ni pour un contrôle
    /// saisi sans résultat (#107).
    pub fn resultat_fr(&self) -> Option<String> {
        let resultat = ResultatControle::depuis_code(&self.resultat_controle)?;
        let limite = date_fr(date_limite_contre_visite(self.date_intervention));
        Some(
            match (
                TypeIntervention::depuis_code(&self.type_intervention)?,
                resultat,
            ) {
                (TypeIntervention::ControleTechnique, ResultatControle::Favorable) => {
                    "Favorable".to_string()
                }
                (TypeIntervention::ControleTechnique, ResultatControle::Defavorable) => {
                    format!("Défavorable : contre-visite avant le {limite}")
                }
                (TypeIntervention::ControleTechnique, ResultatControle::Critique) => {
                    format!("Défavorable, circulation interdite : contre-visite avant le {limite}")
                }
                (TypeIntervention::ContreVisite, ResultatControle::Favorable) => {
                    match self.controle_initial {
                        Some(initial)
                            if self.date_intervention > date_limite_contre_visite(initial) =>
                        {
                            format!(
                                "Favorable, mais hors délai (contrôle du {}) : un contrôle complet est à refaire",
                                date_fr(initial)
                            )
                        }
                        Some(initial) => format!("Favorable (contrôle du {})", date_fr(initial)),
                        None => "Favorable".to_string(),
                    }
                }
                (TypeIntervention::ContreVisite, _) => {
                    "Défavorable : un contrôle technique complet est à refaire".to_string()
                }
                _ => return None,
            },
        )
    }
}

/// Date avant laquelle la contre-visite d'un contrôle défavorable doit avoir lieu : deux mois
/// après le contrôle (règle française, validée le 16/10/2026, #107).
pub fn date_limite_contre_visite(controle: NaiveDate) -> NaiveDate {
    controle
        .checked_add_months(Months::new(2))
        .unwrap_or(controle)
}

#[derive(Debug, Clone)]
pub struct NouvelleIntervention {
    pub vehicule_id: i64,
    pub type_intervention: TypeIntervention,
    /// Obligatoire pour un contrôle technique ou une contre-visite, absent sinon.
    pub resultat_controle: Option<ResultatControle>,
    pub date_intervention: NaiveDate,
    pub kilometrage: i32,
    pub cout_centimes: i64,
    pub prestataire: String,
    pub notes: String,
}

const SELECTION: &str = "SELECT i.*, v.immatriculation
     FROM interventions i
     JOIN vehicules v ON v.id = i.vehicule_id";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} ORDER BY i.date_intervention DESC, i.id DESC"
    ))
    .fetch_all(ex)
    .await
}

/// Les `limite` interventions les plus récentes, pour le tableau de bord.
pub async fn lister_recentes(
    ex: impl PgExecutor<'_>,
    limite: i64,
) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} ORDER BY i.date_intervention DESC, i.id DESC LIMIT $1"
    ))
    .bind(limite)
    .fetch_all(ex)
    .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!(
        "{SELECTION} WHERE i.vehicule_id = $1 ORDER BY i.date_intervention DESC, i.id DESC"
    ))
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Intervention>> {
    sqlx::query_as::<_, Intervention>(&format!("{SELECTION} WHERE i.id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

/// La date du dernier contrôle technique défavorable (ou critique) du véhicule au plus tard à
/// `date` : le contrôle que suit une contre-visite faite ce jour-là.
pub async fn dernier_controle_defavorable_avant(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
    date: NaiveDate,
) -> sqlx::Result<Option<NaiveDate>> {
    sqlx::query_scalar::<_, NaiveDate>(
        "SELECT date_intervention FROM interventions
         WHERE vehicule_id = $1 AND type_intervention = 'controle_technique'
           AND resultat_controle IN ('defavorable', 'critique') AND date_intervention <= $2
         ORDER BY date_intervention DESC, id DESC LIMIT 1",
    )
    .bind(vehicule_id)
    .bind(date)
    .fetch_optional(ex)
    .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, i: &NouvelleIntervention) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO interventions
            (vehicule_id, type_intervention, resultat_controle, date_intervention, kilometrage,
             cout_centimes, prestataire, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(i.vehicule_id)
    .bind(i.type_intervention.code())
    .bind(i.resultat_controle.map(|r| r.code()).unwrap_or(""))
    .bind(i.date_intervention)
    .bind(i.kilometrage)
    .bind(i.cout_centimes)
    .bind(&i.prestataire)
    .bind(&i.notes)
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    i: &NouvelleIntervention,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE interventions
         SET vehicule_id = $2, type_intervention = $3, resultat_controle = $4,
             date_intervention = $5, kilometrage = $6, cout_centimes = $7, prestataire = $8,
             notes = $9
         WHERE id = $1",
    )
    .bind(id)
    .bind(i.vehicule_id)
    .bind(i.type_intervention.code())
    .bind(i.resultat_controle.map(|r| r.code()).unwrap_or(""))
    .bind(i.date_intervention)
    .bind(i.kilometrage)
    .bind(i.cout_centimes)
    .bind(&i.prestataire)
    .bind(&i.notes)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM interventions WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
