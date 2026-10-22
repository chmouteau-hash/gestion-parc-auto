//! Ce que le véhicule coûte et consomme (#126, bloc 3, et #94, bloc 6) : la consommation de chaque
//! plein complet, les totaux de l'entretien, du carburant et des péages, le coût au kilomètre. Des
//! fonctions pures, sans base : tout se **calcule** à l'affichage et ne se stocke jamais, corriger
//! un plein ou une intervention corrige tout (même principe que les échéances, #94).
//!
//! ## La consommation
//!
//! Un plein **complet avec kilométrage** est une borne. Entre deux bornes consécutives, tout ce
//! qui a été mis dans le réservoir (les pleins partiels, ceux sans kilométrage, et le plein de
//! la borne d'arrivée, pas celui de la borne de départ) a été consommé sur les kilomètres qui
//! les séparent : `quantité / kilomètres × 100`. Les pleins sont rangés par date puis par
//! kilométrage, pas dans l'ordre où ils ont été saisis ; un intervalle dont le kilométrage ne
//! croît pas, ou dont une quantité n'est pas notée, n'a pas de consommation (on ne devine pas).
//! Les pleins ne se mélangent que s'ils ont la **même unité** : un hybride rechargeable a une
//! consommation en L/100 km et une autre en kWh/100 km, jamais une moyenne des deux.
//!
//! ## Le coût
//!
//! Le coût d'un véhicule est ce que coûtent son **entretien** (ses interventions), son
//! **carburant** et ses **péages**. Une intervention coûte ce que dit sa facture, main-d'œuvre
//! comprise ; si les prix saisis sur ses lignes « ce qui a été fait » font davantage (un coût
//! oublié), ils font foi : le détail ne coûte pas plus que le tout. Ces prix sont les **pièces** ;
//! le **reste** est la différence (main-d'œuvre, ce qui n'est pas détaillé).
//!
//! ## Le coût au kilomètre
//!
//! Sur les douze derniers mois : tout ce qui a été payé (entretien, carburant et péages) entre le
//! premier et le dernier kilométrage relevé (un plein ou une intervention), divisé par les
//! kilomètres parcourus entre les deux. Le premier relevé n'en fait pas partie, plein ou
//! intervention : il se rapporte à ce qui a été roulé avant.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;

use crate::commun::format::{centimes_fr, dixiemes_fr, euros_au_km_fr, nombre_fr};
use crate::commun::types::TypeDepense;
use crate::donnees::depenses::{Consommation, Depense};

/// La consommation de chaque plein complet qui clôt un intervalle mesurable, par identifiant de
/// dépense. `depenses` peut mêler plusieurs véhicules et plusieurs carburants : les pleins sont
/// regroupés par véhicule et par unité.
pub fn consommations(depenses: &[Depense]) -> HashMap<i64, Consommation> {
    let mut groupes: BTreeMap<(i64, &'static str), Vec<&Depense>> = BTreeMap::new();
    for d in depenses.iter().filter(|d| d.est_un_plein()) {
        groupes
            .entry((d.vehicule_id, d.unite()))
            .or_default()
            .push(d);
    }
    let mut resultat = HashMap::new();
    for ((_, unite), mut pleins) in groupes {
        // Chronologique, pas dans l'ordre de saisie. Le kilométrage départage deux pleins du même
        // jour ; sans kilométrage, il passe après.
        pleins.sort_by_key(|d| (d.date_depense, d.kilometrage.unwrap_or(i32::MAX), d.id));
        let mut borne: Option<i32> = None;
        // Ce qui a été mis depuis la borne de départ, sans le plein qui clôt, en centièmes.
        let mut entre: i64 = 0;
        // Une quantité non notée entre les deux : l'intervalle n'est pas mesurable.
        let mut inconnue = false;
        for d in pleins {
            let Some(km) = d.kilometrage.filter(|_| d.plein_complet) else {
                if borne.is_some() {
                    entre += i64::from(d.quantite_centiemes);
                    inconnue |= d.quantite_centiemes == 0;
                }
                continue;
            };
            if let Some(depart) = borne {
                let distance = i64::from(km) - i64::from(depart);
                let quantite = entre + i64::from(d.quantite_centiemes);
                if distance > 0 && !inconnue && d.quantite_centiemes > 0 {
                    // Q centièmes sur K km = Q/K unités aux 100 km, soit Q×10/K dixièmes.
                    let dixiemes = (quantite * 10 + distance / 2) / distance;
                    resultat.insert(
                        d.id,
                        Consommation {
                            dixiemes_aux_100_km: dixiemes as i32,
                            unite,
                            kilometres: distance as i32,
                            quantite_centiemes: quantite as i32,
                        },
                    );
                }
            }
            borne = Some(km);
            entre = 0;
            inconnue = false;
        }
    }
    resultat
}

/// La consommation moyenne d'un véhicule pour une unité : toutes les quantités sur tous les
/// kilomètres des intervalles mesurés (pondérée, pas une moyenne de moyennes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConsommationMoyenne {
    pub dixiemes_aux_100_km: i32,
    pub unite: &'static str,
    /// Combien d'intervalles mesurés, c'est-à-dire de pleins complets qui les ferment.
    pub intervalles: usize,
    pub kilometres: i32,
}

impl ConsommationMoyenne {
    pub fn fr(&self) -> String {
        format!(
            "{} {}/100\u{202F}km",
            dixiemes_fr(self.dixiemes_aux_100_km),
            self.unite
        )
    }

    /// `1650` → `1 650 km`.
    pub fn kilometres_fr(&self) -> String {
        format!("{}\u{202F}km", nombre_fr(i64::from(self.kilometres)))
    }
}

/// Les moyennes par unité (litres, puis kWh, puis kg), à partir de ce que [`consommations`] a
/// calculé pour un véhicule.
pub fn moyennes<'a>(
    consommations: impl IntoIterator<Item = &'a Consommation>,
) -> Vec<ConsommationMoyenne> {
    let mut par_unite: BTreeMap<usize, (&'static str, i64, i64, usize)> = BTreeMap::new();
    for c in consommations {
        let rang = ["L", "kWh", "kg"]
            .iter()
            .position(|u| *u == c.unite)
            .unwrap_or(3);
        let ligne = par_unite.entry(rang).or_insert((c.unite, 0, 0, 0));
        ligne.1 += i64::from(c.quantite_centiemes);
        ligne.2 += i64::from(c.kilometres);
        ligne.3 += 1;
    }
    par_unite
        .into_values()
        .filter(|(_, _, km, _)| *km > 0)
        .map(|(unite, quantite, km, intervalles)| ConsommationMoyenne {
            dixiemes_aux_100_km: ((quantite * 10 + km / 2) / km) as i32,
            unite,
            intervalles,
            kilometres: km as i32,
        })
        .collect()
}

/// Ce qui a été dépensé, carburant et péages séparés, en centimes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Montants {
    pub carburant_centimes: i64,
    pub peage_centimes: i64,
}

impl Montants {
    pub fn de<'a>(depenses: impl IntoIterator<Item = &'a Depense>) -> Montants {
        let mut m = Montants::default();
        for d in depenses {
            match TypeDepense::depuis_code(&d.type_depense) {
                Some(TypeDepense::Carburant) => m.carburant_centimes += d.montant_centimes,
                Some(TypeDepense::Peage) => m.peage_centimes += d.montant_centimes,
                None => {}
            }
        }
        m
    }

    pub fn total_centimes(&self) -> i64 {
        self.carburant_centimes + self.peage_centimes
    }

    pub fn carburant_fr(&self) -> String {
        centimes_fr(self.carburant_centimes)
    }

    pub fn peage_fr(&self) -> String {
        centimes_fr(self.peage_centimes)
    }

    pub fn total_fr(&self) -> String {
        centimes_fr(self.total_centimes())
    }
}

/// Une intervention d'entretien, vue par le calcul du coût.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entretien {
    pub id: i64,
    pub date: NaiveDate,
    pub kilometrage: i32,
    /// Le coût de la facture, main-d'œuvre comprise.
    pub cout_centimes: i64,
    /// La somme des prix saisis sur ses lignes « ce qui a été fait » (zéro sans prix saisi).
    pub pieces_centimes: i64,
}

impl Entretien {
    /// Le coût retenu : celui de la facture, ou la somme des prix de ses lignes si elle est plus
    /// grande. Le détail ne coûte pas plus que le tout, et un coût oublié ne fait pas disparaître
    /// des prix saisis.
    pub fn total_centimes(&self) -> i64 {
        self.cout_centimes.max(self.pieces_centimes)
    }
}

/// Ce que le véhicule a coûté sur une période, en centimes : l'entretien (dont les pièces), le
/// carburant et les péages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Cout {
    pub entretien_centimes: i64,
    /// Dans l'entretien, ce que détaillent les lignes « ce qui a été fait » : les pièces.
    pub pieces_centimes: i64,
    pub carburant_centimes: i64,
    pub peage_centimes: i64,
}

impl Cout {
    pub fn de<'a>(
        entretiens: impl IntoIterator<Item = &'a Entretien>,
        depenses: impl IntoIterator<Item = &'a Depense>,
    ) -> Cout {
        let mut c = Cout::default();
        for e in entretiens {
            c.entretien_centimes += e.total_centimes();
            c.pieces_centimes += e.pieces_centimes;
        }
        let m = Montants::de(depenses);
        c.carburant_centimes = m.carburant_centimes;
        c.peage_centimes = m.peage_centimes;
        c
    }

    /// Dans l'entretien, ce que les pièces n'expliquent pas : main-d'œuvre, fournitures non
    /// détaillées.
    pub fn reste_centimes(&self) -> i64 {
        self.entretien_centimes - self.pieces_centimes
    }

    pub fn total_centimes(&self) -> i64 {
        self.entretien_centimes + self.carburant_centimes + self.peage_centimes
    }

    /// Des prix de pièces ont été saisis : la page peut séparer pièces et reste.
    pub fn a_des_pieces(&self) -> bool {
        self.pieces_centimes > 0
    }

    pub fn entretien_fr(&self) -> String {
        centimes_fr(self.entretien_centimes)
    }

    pub fn pieces_fr(&self) -> String {
        centimes_fr(self.pieces_centimes)
    }

    pub fn reste_fr(&self) -> String {
        centimes_fr(self.reste_centimes())
    }

    pub fn carburant_fr(&self) -> String {
        centimes_fr(self.carburant_centimes)
    }

    pub fn peage_fr(&self) -> String {
        centimes_fr(self.peage_centimes)
    }

    pub fn total_fr(&self) -> String {
        centimes_fr(self.total_centimes())
    }
}

/// Le coût d'un kilomètre roulé, sur la période.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CoutAuKm {
    /// Centimes aux 100 km : assez fin pour un véhicule électrique, qui coûte quelques centimes
    /// au kilomètre.
    pub centimes_aux_100_km: i64,
    /// Les kilomètres parcourus entre le premier et le dernier relevé de la période.
    pub kilometres: i32,
    /// Ce que le carburant et les péages ont coûté entre les deux relevés.
    pub depenses_centimes: i64,
    /// Ce que l'entretien a coûté entre les deux relevés.
    pub entretien_centimes: i64,
}

impl CoutAuKm {
    pub fn fr(&self) -> String {
        euros_au_km_fr(self.centimes_aux_100_km)
    }

    /// `1000` → `1 000 km`.
    pub fn kilometres_fr(&self) -> String {
        format!("{}\u{202F}km", nombre_fr(i64::from(self.kilometres)))
    }
}

/// Un relevé du kilométrage : un plein ou une intervention.
struct Releve {
    date: NaiveDate,
    kilometrage: i32,
    /// La dépense, quand le relevé est un plein.
    depense_id: Option<i64>,
    /// L'intervention, quand le relevé en est une.
    entretien_id: Option<i64>,
}

/// Le coût au kilomètre des douze derniers mois (voir l'en-tête). `entretiens` : les interventions
/// du véhicule, qui sont des relevés et des coûts. `None` quand il y a moins de deux relevés dans
/// la période, que le kilométrage n'avance pas, ou que rien n'a été payé entre les deux.
pub fn cout_au_km(
    depenses: &[Depense],
    entretiens: &[Entretien],
    aujourd_hui: NaiveDate,
) -> Option<CoutAuKm> {
    let debut = aujourd_hui.checked_sub_months(Months::new(12))?;
    let dans_la_periode = |d: NaiveDate| d > debut && d <= aujourd_hui;
    let mut releves: Vec<Releve> = depenses
        .iter()
        .filter(|d| d.est_un_plein() && dans_la_periode(d.date_depense))
        .filter_map(|d| {
            d.kilometrage.map(|km| Releve {
                date: d.date_depense,
                kilometrage: km,
                depense_id: Some(d.id),
                entretien_id: None,
            })
        })
        .chain(
            entretiens
                .iter()
                .filter(|e| dans_la_periode(e.date))
                .map(|e| Releve {
                    date: e.date,
                    kilometrage: e.kilometrage,
                    depense_id: None,
                    entretien_id: Some(e.id),
                }),
        )
        .collect();
    releves.sort_by_key(|r| (r.date, r.kilometrage));
    let (premier, dernier) = (releves.first()?, releves.last()?);
    let kilometres = i64::from(dernier.kilometrage) - i64::from(premier.kilometrage);
    if kilometres <= 0 {
        return None;
    }
    let entre = |date: NaiveDate| date >= premier.date && date <= dernier.date;
    let depenses_centimes: i64 = depenses
        .iter()
        .filter(|d| entre(d.date_depense))
        .filter(|d| Some(d.id) != premier.depense_id)
        .map(|d| d.montant_centimes)
        .sum();
    let entretien_centimes: i64 = entretiens
        .iter()
        .filter(|e| entre(e.date))
        .filter(|e| Some(e.id) != premier.entretien_id)
        .map(Entretien::total_centimes)
        .sum();
    let cout = depenses_centimes + entretien_centimes;
    if cout <= 0 {
        return None;
    }
    Some(CoutAuKm {
        centimes_aux_100_km: (cout * 100 + kilometres / 2) / kilometres,
        kilometres: kilometres as i32,
        depenses_centimes,
        entretien_centimes,
    })
}

/// Ce que le bloc « Coût » de la fiche d'un véhicule résume : l'entretien, le carburant et les
/// péages de l'année et des douze derniers mois, la consommation, le coût au kilomètre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resume {
    /// L'année civile en cours, pour l'intitulé.
    pub annee: i32,
    pub cette_annee: Cout,
    pub douze_derniers_mois: Cout,
    pub consommations: Vec<ConsommationMoyenne>,
    pub cout_au_km: Option<CoutAuKm>,
    pub nombre_de_depenses: usize,
    pub nombre_d_interventions: usize,
}

/// Le résumé du coût **d'un** véhicule. `depenses` : ses pleins et ses péages (la consommation de
/// chaque plein est celle de [`consommations`]) ; `entretiens` : ses interventions.
pub fn resume(depenses: &[Depense], entretiens: &[Entretien], aujourd_hui: NaiveDate) -> Resume {
    let calculees = consommations(depenses);
    let debut = aujourd_hui.checked_sub_months(Months::new(12));
    let douze_mois =
        |date: NaiveDate| date <= aujourd_hui && debut.is_none_or(|debut| date > debut);
    Resume {
        annee: aujourd_hui.year(),
        cette_annee: Cout::de(
            entretiens
                .iter()
                .filter(|e| e.date.year() == aujourd_hui.year()),
            depenses
                .iter()
                .filter(|d| d.date_depense.year() == aujourd_hui.year()),
        ),
        douze_derniers_mois: Cout::de(
            entretiens.iter().filter(|e| douze_mois(e.date)),
            depenses.iter().filter(|d| douze_mois(d.date_depense)),
        ),
        consommations: moyennes(calculees.values()),
        cout_au_km: cout_au_km(depenses, entretiens, aujourd_hui),
        nombre_de_depenses: depenses.len(),
        nombre_d_interventions: entretiens.len(),
    }
}

/// Une période : du premier au dernier jour compris.
pub type Periode = (NaiveDate, NaiveDate);

/// Le mois de `aujourd_hui`, du premier au dernier jour.
pub fn mois_en_cours(aujourd_hui: NaiveDate) -> Periode {
    let premier = aujourd_hui.with_day(1).unwrap_or(aujourd_hui);
    let dernier = premier
        .checked_add_months(Months::new(1))
        .and_then(|d| d.pred_opt())
        .unwrap_or(aujourd_hui);
    (premier, dernier)
}

/// L'année civile de `aujourd_hui`, du 1er janvier au 31 décembre.
pub fn annee_en_cours(aujourd_hui: NaiveDate) -> Periode {
    let premier = NaiveDate::from_ymd_opt(aujourd_hui.year(), 1, 1).unwrap_or(aujourd_hui);
    let dernier = NaiveDate::from_ymd_opt(aujourd_hui.year(), 12, 31).unwrap_or(aujourd_hui);
    (premier, dernier)
}

/// Les douze derniers mois, jusqu'à aujourd'hui compris : du lendemain d'il y a un an à
/// aujourd'hui (comme [`resume`]).
pub fn douze_derniers_mois(aujourd_hui: NaiveDate) -> Periode {
    let debut = aujourd_hui
        .checked_sub_months(Months::new(12))
        .and_then(|d| d.succ_opt())
        .unwrap_or(aujourd_hui);
    (debut, aujourd_hui)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn jour(m: u32, j: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, j).unwrap()
    }

    /// Un plein (unité litres, carburant gazole), ou kWh si `carburant` = « electricite ».
    #[allow(clippy::too_many_arguments)]
    fn plein(
        id: i64,
        vehicule: i64,
        date: NaiveDate,
        quantite_centiemes: i32,
        km: Option<i32>,
        complet: bool,
        carburant: &str,
        montant: i64,
    ) -> Depense {
        Depense {
            id,
            vehicule_id: vehicule,
            type_depense: "carburant".into(),
            date_depense: date,
            montant_centimes: montant,
            carburant: carburant.into(),
            quantite_centiemes,
            kilometrage: km,
            plein_complet: complet,
            libelle: String::new(),
            notes: String::new(),
            cree_le: Utc::now(),
            immatriculation: "AB-123-CD".into(),
            couleur: String::new(),
            documents: Vec::new(),
            consommation: None,
        }
    }

    fn gazole(id: i64, date: NaiveDate, q: i32, km: Option<i32>, complet: bool) -> Depense {
        plein(id, 1, date, q, km, complet, "gazole", 6000)
    }

    fn peage(id: i64, date: NaiveDate, montant: i64) -> Depense {
        Depense {
            type_depense: "peage".into(),
            carburant: String::new(),
            quantite_centiemes: 0,
            kilometrage: None,
            plein_complet: false,
            ..plein(id, 1, date, 0, None, false, "", montant)
        }
    }

    fn entretien(id: i64, date: NaiveDate, km: i32, cout: i64, pieces: i64) -> Entretien {
        Entretien {
            id,
            date,
            kilometrage: km,
            cout_centimes: cout,
            pieces_centimes: pieces,
        }
    }

    #[test]
    fn la_consommation_se_mesure_entre_deux_pleins_complets() {
        let pleins = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 4000, Some(10_650), true),
        ];
        let c = consommations(&pleins);
        // Le premier plein n'a pas de départ ; le second : 40 L sur 650 km = 6,15 → 6,2.
        assert!(!c.contains_key(&1));
        let deuxieme = c[&2];
        assert_eq!(deuxieme.dixiemes_aux_100_km, 62);
        assert_eq!(deuxieme.kilometres, 650);
        assert_eq!(deuxieme.quantite_centiemes, 4000);
        assert_eq!(deuxieme.unite, "L");
        assert_eq!(deuxieme.fr(), "6,2 L/100\u{202F}km");
    }

    #[test]
    fn un_plein_partiel_entre_les_deux_compte_dans_les_litres_sans_faire_borne() {
        let pleins = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            // Un plein partiel, avec son kilométrage : il ne sert pas de borne.
            gazole(2, jour(1, 12), 2000, Some(10_300), false),
            gazole(3, jour(1, 20), 3000, Some(10_650), true),
        ];
        let c = consommations(&pleins);
        assert!(
            !c.contains_key(&2),
            "un plein partiel n'a pas de consommation"
        );
        // (20 + 30) L sur 650 km = 7,69 → 7,7.
        assert_eq!(c[&3].dixiemes_aux_100_km, 77);
        assert_eq!(c[&3].quantite_centiemes, 5000);
        assert_eq!(c[&3].kilometres, 650);
    }

    #[test]
    fn un_plein_complet_sans_kilometrage_compte_dans_les_litres_sans_faire_borne() {
        let pleins = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 12), 2500, None, true),
            gazole(3, jour(1, 20), 2500, Some(10_650), true),
        ];
        let c = consommations(&pleins);
        assert!(!c.contains_key(&2));
        assert_eq!(c[&3].quantite_centiemes, 5000);
        assert_eq!(c[&3].dixiemes_aux_100_km, 77);
    }

    #[test]
    fn les_pleins_se_rangent_par_date_pas_par_ordre_de_saisie() {
        // Saisis dans le désordre : l'identifiant le plus petit est le plus récent.
        let pleins = [
            gazole(1, jour(2, 10), 4000, Some(11_300), true),
            gazole(2, jour(1, 5), 5000, Some(10_000), true),
            gazole(3, jour(1, 20), 4000, Some(10_650), true),
        ];
        let c = consommations(&pleins);
        assert_eq!(c.len(), 2);
        assert_eq!(c[&3].kilometres, 650);
        assert_eq!(c[&1].kilometres, 650);
        assert_eq!(c[&1].dixiemes_aux_100_km, 62);
        // Le même jeu rangé autrement donne le même résultat : l'ordre du tableau ne compte pas.
        let inverse: Vec<Depense> = pleins.iter().rev().cloned().collect();
        assert_eq!(consommations(&inverse), c);
    }

    #[test]
    fn deux_pleins_du_meme_jour_se_rangent_par_kilometrage() {
        let pleins = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 4000, Some(10_650), true),
            gazole(3, jour(1, 20), 100, Some(10_700), true),
        ];
        let c = consommations(&pleins);
        assert_eq!(c[&2].kilometres, 650);
        assert_eq!(c[&3].kilometres, 50);
    }

    #[test]
    fn un_intervalle_qui_ne_se_mesure_pas_n_a_pas_de_consommation() {
        // Kilométrage qui recule (une faute de saisie) : pas de consommation pour ce plein, et le
        // suivant repart de lui.
        let recul = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 4000, Some(9_900), true),
            gazole(3, jour(2, 5), 4000, Some(10_500), true),
        ];
        let c = consommations(&recul);
        assert!(!c.contains_key(&2));
        assert_eq!(c[&3].kilometres, 600);
        // Une quantité non notée entre les deux : on ne devine pas.
        let inconnue = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 12), 0, Some(10_300), false),
            gazole(3, jour(1, 20), 4000, Some(10_650), true),
        ];
        assert!(consommations(&inconnue).is_empty());
        // La quantité du plein qui clôt n'est pas notée.
        let sans_quantite = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 0, Some(10_650), true),
            gazole(3, jour(2, 5), 4000, Some(11_300), true),
        ];
        let c = consommations(&sans_quantite);
        assert!(!c.contains_key(&2));
        // Le suivant repart du plein sans quantité (sa borne de kilométrage compte).
        assert_eq!(c[&3].kilometres, 650);
        // Mêmes kilomètres : distance nulle.
        let nul = [
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 4000, Some(10_000), true),
        ];
        assert!(consommations(&nul).is_empty());
    }

    #[test]
    fn les_vehicules_et_les_unites_ne_se_melangent_pas() {
        let pleins = [
            // Véhicule 1, litres.
            gazole(1, jour(1, 5), 5000, Some(10_000), true),
            gazole(2, jour(1, 20), 4000, Some(10_650), true),
            // Un autre véhicule : ses pleins ne bornent pas ceux du premier.
            plein(3, 2, jour(1, 10), 3000, Some(500), true, "sp95", 5000),
            plein(4, 2, jour(1, 25), 3000, Some(900), true, "sp95", 5000),
            // Une recharge, en kWh, sur le premier véhicule (un hybride rechargeable).
            plein(
                5,
                1,
                jour(1, 8),
                2000,
                Some(10_100),
                true,
                "electricite",
                400,
            ),
            plein(
                6,
                1,
                jour(1, 22),
                3000,
                Some(10_700),
                true,
                "electricite",
                600,
            ),
        ];
        let c = consommations(&pleins);
        // Les litres du véhicule 1 : 10 000 → 10 650, sans les recharges ni l'autre véhicule.
        assert_eq!(c[&2].kilometres, 650);
        assert_eq!(c[&2].unite, "L");
        // Le véhicule 2 : 30 L sur 400 km = 7,5.
        assert_eq!(c[&4].dixiemes_aux_100_km, 75);
        // Les kWh du véhicule 1 : 30 kWh sur 600 km = 5,0 kWh/100 km.
        assert_eq!(c[&6].unite, "kWh");
        assert_eq!(c[&6].dixiemes_aux_100_km, 50);
        assert_eq!(c[&6].kilometres, 600);
        // Une moyenne par unité, litres d'abord.
        let par_unite = moyennes(c.values().filter(|x| x.kilometres >= 600));
        assert_eq!(
            par_unite.iter().map(|m| m.unite).collect::<Vec<_>>(),
            ["L", "kWh"]
        );
    }

    #[test]
    fn la_moyenne_est_ponderee_par_les_kilometres() {
        let intervalles = [
            // 40 L sur 400 km = 10,0 ; 30 L sur 600 km = 5,0 ; ensemble : 70 L sur 1 000 km = 7,0.
            Consommation {
                dixiemes_aux_100_km: 100,
                unite: "L",
                kilometres: 400,
                quantite_centiemes: 4000,
            },
            Consommation {
                dixiemes_aux_100_km: 50,
                unite: "L",
                kilometres: 600,
                quantite_centiemes: 3000,
            },
        ];
        let m = moyennes(&intervalles);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].dixiemes_aux_100_km, 70);
        assert_eq!(m[0].kilometres, 1000);
        assert_eq!(m[0].intervalles, 2);
        assert_eq!(m[0].fr(), "7,0 L/100\u{202F}km");
        assert!(moyennes(std::iter::empty()).is_empty());
    }

    #[test]
    fn les_montants_separent_le_carburant_et_les_peages() {
        let liste = [
            gazole(1, jour(1, 5), 5000, None, true),
            peage(2, jour(1, 6), 1230),
            gazole(3, jour(1, 7), 4000, None, true),
        ];
        let m = Montants::de(&liste);
        assert_eq!(m.carburant_centimes, 12_000);
        assert_eq!(m.peage_centimes, 1230);
        assert_eq!(m.total_centimes(), 13_230);
        assert_eq!(m.total_fr(), "132,30\u{202F}€");
        assert_eq!(Montants::de(&[]), Montants::default());
    }

    #[test]
    fn le_cout_au_kilometre_ne_compte_pas_le_plein_du_premier_releve() {
        let aujourd_hui = jour(10, 21);
        let depenses = [
            // Le premier relevé : son plein remplit ce qui a été roulé avant, hors période.
            plein(1, 1, jour(6, 1), 4000, Some(10_000), true, "gazole", 7000),
            plein(2, 1, jour(7, 1), 4000, Some(10_600), true, "gazole", 6000),
            peage(3, jour(7, 15), 1000),
            plein(4, 1, jour(8, 1), 4000, Some(11_000), true, "gazole", 5000),
        ];
        let c = cout_au_km(&depenses, &[], aujourd_hui).expect("un coût");
        assert_eq!(c.kilometres, 1000);
        // 60 + 10 + 50 = 120 € sur 1 000 km = 12 centimes au km = 1 200 centimes aux 100 km.
        assert_eq!(c.depenses_centimes, 12_000);
        assert_eq!(c.centimes_aux_100_km, 1200);
        assert_eq!(c.fr(), "0,12\u{202F}€/km");
    }

    #[test]
    fn le_cout_au_kilometre_prend_aussi_les_interventions_comme_releves() {
        let aujourd_hui = jour(10, 21);
        let depenses = [
            plein(1, 1, jour(7, 1), 4000, Some(10_600), true, "gazole", 6000),
            plein(2, 1, jour(8, 1), 4000, Some(11_000), true, "gazole", 5000),
        ];
        // Une intervention antérieure à 10 000 km : le premier relevé est l'intervention, donc le
        // plein du 1er juillet compte.
        let c = cout_au_km(
            &depenses,
            &[entretien(1, jour(6, 1), 10_000, 0, 0)],
            aujourd_hui,
        )
        .unwrap();
        assert_eq!(c.kilometres, 1000);
        assert_eq!(c.depenses_centimes, 11_000);
    }

    #[test]
    fn pas_de_cout_au_kilometre_sans_deux_releves_ni_depense() {
        let aujourd_hui = jour(10, 21);
        let un_seul = [plein(
            1,
            1,
            jour(7, 1),
            4000,
            Some(10_600),
            true,
            "gazole",
            6000,
        )];
        assert_eq!(cout_au_km(&un_seul, &[], aujourd_hui), None);
        // Un relevé hors des douze derniers mois ne compte pas.
        let vieux = [
            plein(1, 1, jour(1, 1), 4000, Some(10_000), true, "gazole", 6000),
            plein(2, 1, jour(7, 1), 4000, Some(10_600), true, "gazole", 6000),
        ];
        let mut vieux = vieux.to_vec();
        vieux[0].date_depense = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        assert_eq!(cout_au_km(&vieux, &[], aujourd_hui), None);
        // Le kilométrage n'avance pas.
        let fixe = [
            plein(1, 1, jour(7, 1), 4000, Some(10_000), true, "gazole", 6000),
            plein(2, 1, jour(8, 1), 4000, Some(10_000), true, "gazole", 6000),
        ];
        assert_eq!(cout_au_km(&fixe, &[], aujourd_hui), None);
        // Aucun montant noté : pas de coût.
        let gratuit = [
            plein(1, 1, jour(7, 1), 4000, Some(10_000), true, "gazole", 0),
            plein(2, 1, jour(8, 1), 4000, Some(10_500), true, "gazole", 0),
        ];
        assert_eq!(cout_au_km(&gratuit, &[], aujourd_hui), None);
    }

    #[test]
    fn le_cout_au_kilometre_ajoute_l_entretien_sauf_celui_du_premier_releve() {
        let aujourd_hui = jour(10, 21);
        let depenses = [
            plein(1, 1, jour(6, 1), 4000, Some(10_000), true, "gazole", 6000),
            plein(2, 1, jour(7, 1), 4000, Some(11_000), true, "gazole", 6000),
        ];
        let entretiens = [
            // Le premier relevé : il se rapporte à ce qui a été roulé avant, il n'en est pas.
            entretien(1, jour(5, 1), 9_000, 20_000, 0),
            // Une facture de 120 € dont 40 € de pièces : c'est la facture qui compte.
            entretien(2, jour(6, 15), 10_400, 12_000, 4_000),
        ];
        let c = cout_au_km(&depenses, &entretiens, aujourd_hui).expect("un coût");
        assert_eq!(c.kilometres, 2_000);
        assert_eq!(c.depenses_centimes, 12_000);
        assert_eq!(c.entretien_centimes, 12_000);
        // 120 + 120 = 240 € sur 2 000 km = 12 centimes au km.
        assert_eq!(c.centimes_aux_100_km, 1_200);
    }

    #[test]
    fn un_vehicule_sans_depense_a_un_cout_au_kilometre_par_ses_interventions() {
        let entretiens = [
            entretien(1, jour(3, 1), 10_000, 10_000, 0),
            entretien(2, jour(6, 1), 12_000, 30_000, 9_000),
        ];
        let c = cout_au_km(&[], &entretiens, jour(10, 21)).expect("un coût");
        assert_eq!(
            (c.kilometres, c.depenses_centimes, c.entretien_centimes),
            (2_000, 0, 30_000)
        );
        // 300 € sur 2 000 km (la première intervention n'en est pas) = 15 centimes au km.
        assert_eq!(c.fr(), "0,15\u{202F}€/km");
    }

    #[test]
    fn une_intervention_coute_sa_facture_ou_ses_pieces_si_elles_font_davantage() {
        // La facture fait foi quand elle couvre les pièces.
        let facture = entretien(1, jour(3, 1), 10_000, 18_000, 6_500);
        assert_eq!(facture.total_centimes(), 18_000);
        // Un coût oublié : les prix saisis sur les lignes font foi.
        let oublie = entretien(2, jour(3, 1), 10_000, 0, 4_000);
        assert_eq!(oublie.total_centimes(), 4_000);
        // Les pièces ne s'ajoutent pas à la facture : le détail ne coûte pas plus que le tout.
        let incoherent = entretien(3, jour(3, 1), 10_000, 3_000, 4_000);
        assert_eq!(incoherent.total_centimes(), 4_000);
    }

    #[test]
    fn le_cout_separe_l_entretien_dont_les_pieces_le_carburant_et_les_peages() {
        let entretiens = [
            entretien(1, jour(3, 1), 10_000, 18_000, 6_500),
            entretien(2, jour(4, 1), 10_400, 0, 4_000),
        ];
        let depenses = [
            gazole(1, jour(5, 1), 4000, Some(10_800), true),
            peage(2, jour(5, 2), 1_000),
        ];
        let c = Cout::de(&entretiens, &depenses);
        assert_eq!(c.entretien_centimes, 22_000);
        assert_eq!(c.pieces_centimes, 10_500);
        assert_eq!(c.reste_centimes(), 11_500);
        assert_eq!(c.carburant_centimes, 6_000);
        assert_eq!(c.peage_centimes, 1_000);
        assert_eq!(c.total_centimes(), 29_000);
        assert!(c.a_des_pieces());
        assert_eq!(c.entretien_fr(), "220,00\u{202F}€");
        assert_eq!(c.pieces_fr(), "105,00\u{202F}€");
        assert_eq!(c.reste_fr(), "115,00\u{202F}€");
        assert_eq!(c.total_fr(), "290,00\u{202F}€");
        // Sans prix de pièce saisi, il n'y a pas de séparation à montrer.
        let sans = Cout::de(&[entretien(3, jour(3, 1), 10_000, 5_000, 0)], &[]);
        assert!(!sans.a_des_pieces());
        assert_eq!(sans.reste_centimes(), 5_000);
        assert_eq!(Cout::de(&[], &[]), Cout::default());
    }

    #[test]
    fn le_resume_compte_l_entretien_de_l_annee_et_des_douze_derniers_mois() {
        let aujourd_hui = jour(10, 21);
        let le = |a: i32, m: u32, j: u32| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        let entretiens = [
            // Trop ancienne : ni cette année, ni dans les douze derniers mois.
            entretien(1, le(2025, 9, 1), 8_000, 50_000, 0),
            // Dans les douze derniers mois, pas cette année.
            entretien(2, le(2025, 11, 15), 9_000, 20_000, 8_000),
            // Cette année.
            entretien(3, jour(3, 1), 10_000, 30_000, 12_000),
        ];
        let r = resume(&[], &entretiens, aujourd_hui);
        assert_eq!(r.nombre_d_interventions, 3);
        assert_eq!(r.cette_annee.entretien_centimes, 30_000);
        assert_eq!(r.cette_annee.pieces_centimes, 12_000);
        assert_eq!(r.douze_derniers_mois.entretien_centimes, 50_000);
        assert_eq!(r.douze_derniers_mois.pieces_centimes, 20_000);
        assert_eq!(r.douze_derniers_mois.reste_centimes(), 30_000);
        // Ni carburant ni péage : le total est celui de l'entretien.
        assert_eq!(r.douze_derniers_mois.total_centimes(), 50_000);
        // Coût au kilomètre : de novembre 2025 (9 000 km, relevé de départ, hors compte) à mars
        // 2026 (10 000 km) : 300 € sur 1 000 km.
        let cout = r.cout_au_km.expect("un coût");
        assert_eq!((cout.kilometres, cout.entretien_centimes), (1_000, 30_000));
        assert_eq!(cout.centimes_aux_100_km, 3_000);
    }

    #[test]
    fn le_resume_donne_l_annee_les_douze_mois_la_consommation_et_le_cout() {
        let aujourd_hui = jour(10, 21);
        let mut vieux = gazole(1, jour(1, 1), 5000, Some(9_000), true);
        vieux.date_depense = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap();
        vieux.montant_centimes = 9000;
        let depenses = [
            vieux,
            plein(2, 1, jour(7, 1), 4000, Some(10_000), true, "gazole", 6000),
            peage(3, jour(7, 15), 1000),
            plein(4, 1, jour(8, 1), 4000, Some(10_650), true, "gazole", 5000),
        ];
        let r = resume(&depenses, &[], aujourd_hui);
        assert_eq!(r.annee, 2026);
        assert_eq!(r.nombre_de_depenses, 4);
        // Cette année : les trois dépenses de 2026 ; la de mars 2025 en est exclue.
        assert_eq!(r.cette_annee.carburant_centimes, 11_000);
        assert_eq!(r.cette_annee.peage_centimes, 1000);
        // Les douze derniers mois (du 22/10/2025 au 21/10/2026) : pareil, mars 2025 est trop vieux.
        assert_eq!(r.douze_derniers_mois, r.cette_annee);
        // Consommation : 2025-03 (9 000 km) → 2026-07 (10 000 km) mesure 1 000 km pour 40 L, puis
        // 650 km pour 40 L ; ensemble 80 L sur 1 650 km = 4,8.
        assert_eq!(r.consommations.len(), 1);
        assert_eq!(r.consommations[0].dixiemes_aux_100_km, 48);
        assert_eq!(r.consommations[0].intervalles, 2);
        let cout = r.cout_au_km.expect("un coût");
        assert_eq!(cout.kilometres, 650);
        assert_eq!(cout.depenses_centimes, 6000);
        // Un véhicule sans dépense.
        let vide = resume(&[], &[], aujourd_hui);
        assert_eq!(vide.nombre_de_depenses, 0);
        assert!(vide.consommations.is_empty() && vide.cout_au_km.is_none());
    }

    #[test]
    fn les_periodes_courantes() {
        let aujourd_hui = jour(10, 21);
        assert_eq!(mois_en_cours(aujourd_hui), (jour(10, 1), jour(10, 31)));
        assert_eq!(
            mois_en_cours(NaiveDate::from_ymd_opt(2026, 2, 10).unwrap()),
            (
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
            )
        );
        assert_eq!(
            mois_en_cours(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            (
                NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
            )
        );
        assert_eq!(
            annee_en_cours(aujourd_hui),
            (jour(1, 1), NaiveDate::from_ymd_opt(2026, 12, 31).unwrap())
        );
        assert_eq!(
            douze_derniers_mois(aujourd_hui),
            (NaiveDate::from_ymd_opt(2025, 10, 22).unwrap(), aujourd_hui)
        );
    }
}
