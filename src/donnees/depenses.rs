//! Table `depenses` : les pleins de carburant et les tickets de péage (#126).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{centimes_fr, date_fr, nombre_fr, quantite_fr};
use crate::commun::types::{Carburant, TypeDepense};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Depense {
    pub id: i64,
    pub vehicule_id: i64,
    /// Code du vocabulaire [`TypeDepense`] : `carburant` ou `peage`.
    pub type_depense: String,
    pub date_depense: NaiveDate,
    pub montant_centimes: i64,
    /// Centièmes de litre, ou de kWh pour un véhicule électrique ; `0` quand la quantité n'est
    /// pas notée, et toujours pour un péage.
    pub quantite_centiemes: i32,
    /// Le compteur au moment du plein ; `None` quand on ne l'a pas regardé, et toujours pour un
    /// péage.
    pub kilometrage: Option<i32>,
    /// Un plein complet sert de borne au calcul de la consommation ; un plein partiel compte
    /// dans les litres sans faire borne. Toujours faux pour un péage.
    pub plein_complet: bool,
    /// La station, ou le trajet (« Lyon – Grenoble »).
    pub libelle: String,
    pub notes: String,
    pub cree_le: DateTime<Utc>,
    pub immatriculation: String,
    /// Couleur de repère du véhicule, pour sa plaque dans les listes.
    pub couleur: String,
    /// Le carburant du véhicule : il dit l'unité de la quantité (litres ou kWh).
    pub carburant: String,
}

impl Depense {
    pub fn est_un_plein(&self) -> bool {
        TypeDepense::depuis_code(&self.type_depense) == Some(TypeDepense::Carburant)
    }

    pub fn type_libelle(&self) -> &str {
        match TypeDepense::depuis_code(&self.type_depense) {
            Some(valeur) => valeur.libelle(),
            None => &self.type_depense,
        }
    }

    pub fn date_depense_fr(&self) -> String {
        date_fr(self.date_depense)
    }

    pub fn montant_fr(&self) -> String {
        centimes_fr(self.montant_centimes)
    }

    /// « L », ou « kWh » pour un véhicule électrique.
    pub fn unite(&self) -> &'static str {
        Carburant::depuis_code(&self.carburant)
            .map(Carburant::unite_de_plein)
            .unwrap_or("L")
    }

    /// `4230` → `42,3 L` ; vide quand la quantité n'est pas notée.
    pub fn quantite_fr(&self) -> String {
        if self.quantite_centiemes == 0 {
            String::new()
        } else {
            format!("{} {}", quantite_fr(self.quantite_centiemes), self.unite())
        }
    }

    /// `61200` → `61 200 km` ; vide quand le compteur n'a pas été relevé.
    pub fn kilometrage_fr(&self) -> String {
        match self.kilometrage {
            Some(km) => format!("{} km", nombre_fr(i64::from(km))),
            None => String::new(),
        }
    }

    /// Le détail en une ligne, « 42,3 L · 61 200 km · Station fictive » ; « plein partiel » s'y
    /// ajoute quand la case n'est pas cochée. Pour un péage, le trajet, ou rien.
    pub fn detail_fr(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();
        if self.est_un_plein() {
            let quantite = self.quantite_fr();
            if !quantite.is_empty() {
                morceaux.push(quantite);
            }
            let kilometrage = self.kilometrage_fr();
            if !kilometrage.is_empty() {
                morceaux.push(kilometrage);
            }
            if !self.plein_complet {
                morceaux.push("plein partiel".to_string());
            }
        }
        if !self.libelle.is_empty() {
            morceaux.push(self.libelle.clone());
        }
        morceaux.join(" · ")
    }
}

/// Une dépense validée, prête à écrire.
#[derive(Debug, Clone)]
pub struct NouvelleDepense {
    pub vehicule_id: i64,
    pub type_depense: TypeDepense,
    pub date_depense: NaiveDate,
    pub montant_centimes: i64,
    pub quantite_centiemes: i32,
    pub kilometrage: Option<i32>,
    pub plein_complet: bool,
    pub libelle: String,
    pub notes: String,
}

const SELECTION: &str = "SELECT d.*, v.immatriculation, v.couleur, v.carburant
     FROM depenses d
     JOIN vehicules v ON v.id = d.vehicule_id";

pub async fn lister(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Depense>> {
    sqlx::query_as::<_, Depense>(&format!(
        "{SELECTION} ORDER BY d.date_depense DESC, d.id DESC"
    ))
    .fetch_all(ex)
    .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Depense>> {
    sqlx::query_as::<_, Depense>(&format!(
        "{SELECTION} WHERE d.vehicule_id = $1 ORDER BY d.date_depense DESC, d.id DESC"
    ))
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Depense>> {
    sqlx::query_as::<_, Depense>(&format!("{SELECTION} WHERE d.id = $1"))
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, d: &NouvelleDepense) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO depenses
            (vehicule_id, type_depense, date_depense, montant_centimes, quantite_centiemes,
             kilometrage, plein_complet, libelle, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(d.vehicule_id)
    .bind(d.type_depense.code())
    .bind(d.date_depense)
    .bind(d.montant_centimes)
    .bind(d.quantite_centiemes)
    .bind(d.kilometrage)
    .bind(d.plein_complet)
    .bind(&d.libelle)
    .bind(&d.notes)
    .fetch_one(ex)
    .await
}

pub async fn mettre_a_jour(
    ex: impl PgExecutor<'_>,
    id: i64,
    d: &NouvelleDepense,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE depenses
         SET vehicule_id = $2, type_depense = $3, date_depense = $4, montant_centimes = $5,
             quantite_centiemes = $6, kilometrage = $7, plein_complet = $8, libelle = $9,
             notes = $10
         WHERE id = $1",
    )
    .bind(id)
    .bind(d.vehicule_id)
    .bind(d.type_depense.code())
    .bind(d.date_depense)
    .bind(d.montant_centimes)
    .bind(d.quantite_centiemes)
    .bind(d.kilometrage)
    .bind(d.plein_complet)
    .bind(&d.libelle)
    .bind(&d.notes)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM depenses WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}
