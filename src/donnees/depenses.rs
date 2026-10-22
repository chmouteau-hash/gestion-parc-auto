//! Table `depenses` : les pleins de carburant et les tickets de péage (#126).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{centimes_fr, date_fr, dixiemes_fr, nombre_fr, quantite_fr};
use crate::commun::types::{CarburantPlein, TypeDepense};
use crate::donnees::documents::Document;

/// Ce qu'un plein complet a consommé depuis le plein complet précédent qui avait aussi un
/// kilométrage (#126, bloc 3). **Calculée** par `metier::consommation`, jamais stockée : corriger
/// un plein corrige tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Consommation {
    /// Dixièmes d'unité aux 100 km : `62` = 6,2 L/100 km.
    pub dixiemes_aux_100_km: i32,
    /// « L », « kWh » ou « kg », celle du carburant des pleins comptés.
    pub unite: &'static str,
    /// Les kilomètres parcourus entre les deux pleins complets.
    pub kilometres: i32,
    /// Tout ce qui a été mis entre les deux, le plein complet compris, en centièmes d'unité.
    pub quantite_centiemes: i32,
}

impl Consommation {
    /// `6,2 L/100 km`, avec une espace insécable avant « km ».
    pub fn fr(&self) -> String {
        format!(
            "{} {}/100\u{202F}km",
            dixiemes_fr(self.dixiemes_aux_100_km),
            self.unite
        )
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Depense {
    pub id: i64,
    pub vehicule_id: i64,
    /// Code du vocabulaire [`TypeDepense`] : `carburant` ou `peage`.
    pub type_depense: String,
    pub date_depense: NaiveDate,
    pub montant_centimes: i64,
    /// Le carburant pris ce jour-là (code du vocabulaire [`CarburantPlein`]) : il fixe l'unité
    /// de la quantité. Vide pour un péage.
    pub carburant: String,
    /// Centièmes de litre, de kWh ou de kg selon le carburant ; `0` quand la quantité n'est pas
    /// notée, et toujours pour un péage.
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
    /// Les documents joints à la dépense, son ticket le plus souvent (#126, bloc 2). La requête
    /// ne les lit pas : la couche métier les attache après coup, en une requête pour toute une
    /// liste.
    #[sqlx(skip)]
    #[serde(default)]
    pub documents: Vec<Document>,
    /// Pour un plein complet qui clôt un intervalle mesurable : ce qu'il a consommé (#126,
    /// bloc 3). Attachée après coup par la couche métier, comme les documents.
    #[sqlx(skip)]
    #[serde(default)]
    pub consommation: Option<Consommation>,
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

    /// « SP98 », « Gazole (B7) »… ; vide pour un péage.
    pub fn carburant_libelle(&self) -> &str {
        match CarburantPlein::depuis_code(&self.carburant) {
            Some(valeur) => valeur.libelle(),
            None => &self.carburant,
        }
    }

    pub fn date_depense_fr(&self) -> String {
        date_fr(self.date_depense)
    }

    /// « Plein du 12/10/2026 », « Péage du 12/10/2026 » : pour nommer la dépense depuis la page
    /// de son ticket.
    pub fn nom_fr(&self) -> String {
        let quoi = if self.est_un_plein() {
            "Plein"
        } else {
            "Péage"
        };
        format!("{quoi} du {}", self.date_depense_fr())
    }

    pub fn montant_fr(&self) -> String {
        centimes_fr(self.montant_centimes)
    }

    /// « L », « kWh » pour l'électricité, « kg » pour le GNV.
    pub fn unite(&self) -> &'static str {
        CarburantPlein::depuis_code(&self.carburant)
            .map(CarburantPlein::unite)
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

    /// Le détail en une ligne, « 42,3 L SP98 · 61 200 km · Station fictive » ; « plein
    /// partiel » s'y ajoute quand la case n'est pas cochée. Pour un péage, le trajet, ou rien.
    pub fn detail_fr(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();
        if self.est_un_plein() {
            let quantite = self.quantite_fr();
            let carburant = self.carburant_libelle();
            morceaux.push(if quantite.is_empty() {
                carburant.to_string()
            } else {
                format!("{quantite} {carburant}")
            });
            let kilometrage = self.kilometrage_fr();
            if !kilometrage.is_empty() {
                morceaux.push(kilometrage);
            }
            if let Some(c) = &self.consommation {
                morceaux.push(c.fr());
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
    /// Le carburant du plein ; `None` pour un péage.
    pub carburant: Option<CarburantPlein>,
    pub quantite_centiemes: i32,
    pub kilometrage: Option<i32>,
    pub plein_complet: bool,
    pub libelle: String,
    pub notes: String,
}

const SELECTION: &str = "SELECT d.*, v.immatriculation, v.couleur
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

/// Ce qui a été dépensé (carburant et péages) entre deux dates comprises, en centimes : le
/// chiffre « Dépenses du mois » du tableau de bord (#126, bloc 3).
pub async fn total_entre(
    ex: impl PgExecutor<'_>,
    du: NaiveDate,
    au: NaiveDate,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(montant_centimes), 0)::BIGINT FROM depenses
         WHERE date_depense BETWEEN $1 AND $2",
    )
    .bind(du)
    .bind(au)
    .fetch_one(ex)
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
            (vehicule_id, type_depense, date_depense, montant_centimes, carburant,
             quantite_centiemes, kilometrage, plein_complet, libelle, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(d.vehicule_id)
    .bind(d.type_depense.code())
    .bind(d.date_depense)
    .bind(d.montant_centimes)
    .bind(d.carburant.map(|c| c.code()).unwrap_or(""))
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
             carburant = $6, quantite_centiemes = $7, kilometrage = $8, plein_complet = $9,
             libelle = $10, notes = $11
         WHERE id = $1",
    )
    .bind(id)
    .bind(d.vehicule_id)
    .bind(d.type_depense.code())
    .bind(d.date_depense)
    .bind(d.montant_centimes)
    .bind(d.carburant.map(|c| c.code()).unwrap_or(""))
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
