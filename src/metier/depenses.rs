//! Les dépenses pour rouler (#126) : validation et opérations.
//!
//! Un seul écran pour les pleins de carburant et les tickets de péage, distingués par un type
//! d'un vocabulaire fermé ([`TypeDepense`]). Seul un plein porte le carburant pris ce jour-là
//! ([`CarburantPlein`] : SP95, SP98, E85, gazole, GPL, GNV, électricité…), une quantité, le
//! compteur et la case « plein complet ». Sur un péage, la quantité et le kilométrage sont
//! refusés quand ils sont renseignés, parce qu'un formulaire sans script les envoie quand même
//! (vides) ; le carburant et la case, qui ont une valeur d'office, sont ignorés.
//!
//! Le carburant d'un plein est **proposé d'après celui du véhicule** (gazole pour un diesel,
//! électricité pour un électrique…) quand la saisie ne le dit pas : le formulaire le pré-remplit,
//! et l'API peut l'omettre. C'est lui, pas le véhicule, qui fixe l'unité de la quantité : un
//! hybride rechargeable fait un plein d'essence en litres et une recharge en kWh.
//!
//! Un plein noté avec son kilométrage **relève le compteur du véhicule** s'il est plus haut, comme
//! une intervention ; un plein ancien ne le fait pas reculer. Le kilométrage est facultatif : sans
//! lui, pas de consommation pour ce plein, mais on ne bloque pas qui n'a pas regardé le compteur.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::quantite_fr;
use crate::commun::types::{Carburant, CarburantPlein, TypeDepense};
use crate::donnees::depenses::{self, Depense, NouvelleDepense};
use crate::donnees::vehicules;
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::validation::{
    case_a_cocher, centimes_en_saisie, code_parmi, date_obligatoire, entier_positif, identifiant,
    montant_en_centimes, notes, quantite_en_centiemes, texte_facultatif,
};

/// Ce que le formulaire ou l'API envoie : tout en chaînes, une seule validation.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieDepense {
    #[serde(default)]
    pub vehicule_id: String,
    /// Code du vocabulaire [`TypeDepense`].
    #[serde(default)]
    pub type_depense: String,
    #[serde(default)]
    pub date_depense: String,
    /// Montant en euros (`68,40`), converti en centimes à la validation ; vide = 0.
    #[serde(default)]
    pub montant: String,
    /// Code du vocabulaire [`CarburantPlein`] ; vide = celui proposé d'après le véhicule.
    /// Carburant seulement : ignoré pour un péage.
    #[serde(default)]
    pub carburant: String,
    /// Quantité en litres, en kWh ou en kg (`42,3`) ; vide = non notée. Carburant seulement.
    #[serde(default)]
    pub quantite: String,
    /// Le compteur au moment du plein ; vide = non relevé. Carburant seulement.
    #[serde(default)]
    pub kilometrage: String,
    /// La case : `oui`/`on`/`true`/`1` cochée, vide ou `non`/`false`/`0` décochée. Carburant
    /// seulement : ignorée pour un péage, parce que le formulaire la coche d'office.
    #[serde(default)]
    pub plein_complet: String,
    /// La station, ou le trajet.
    #[serde(default)]
    pub libelle: String,
    #[serde(default)]
    pub notes: String,
}

impl SaisieDepense {
    pub fn depuis(d: &Depense) -> SaisieDepense {
        SaisieDepense {
            vehicule_id: d.vehicule_id.to_string(),
            type_depense: d.type_depense.clone(),
            date_depense: d.date_depense.to_string(),
            montant: centimes_en_saisie(d.montant_centimes),
            carburant: d.carburant.clone(),
            quantite: if d.quantite_centiemes == 0 {
                String::new()
            } else {
                quantite_fr(d.quantite_centiemes)
            },
            kilometrage: d.kilometrage.map(|km| km.to_string()).unwrap_or_default(),
            plein_complet: if d.plein_complet {
                "oui".to_string()
            } else {
                String::new()
            },
            libelle: d.libelle.clone(),
            notes: d.notes.clone(),
        }
    }

    /// Pour le formulaire : la case « plein complet » est-elle cochée ? Une valeur illisible
    /// compte pour décochée ; la validation, elle, la refuse.
    pub fn plein_complet_coche(&self) -> bool {
        case_a_cocher(&self.plein_complet, "plein complet").unwrap_or(false)
    }
}

/// Valide la saisie. `carburant_du_vehicule` sert à proposer le carburant d'un plein qui ne le
/// dit pas ; `None` quand le véhicule a un carburant inconnu, et « Autre » est alors proposé.
pub fn valider(
    s: &SaisieDepense,
    carburant_du_vehicule: Option<Carburant>,
) -> Resultat<NouvelleDepense> {
    let type_depense = code_parmi(&s.type_depense, "type", TypeDepense::depuis_code)?;
    let (carburant, quantite_centiemes, kilometrage, plein_complet) = match type_depense {
        TypeDepense::Carburant => (
            Some(if s.carburant.trim().is_empty() {
                CarburantPlein::propose_pour(carburant_du_vehicule.unwrap_or(Carburant::Autre))
            } else {
                code_parmi(&s.carburant, "carburant", CarburantPlein::depuis_code)?
            }),
            if s.quantite.trim().is_empty() {
                0
            } else {
                quantite_en_centiemes(&s.quantite, "quantité")?
            },
            if s.kilometrage.trim().is_empty() {
                None
            } else {
                Some(entier_positif(&s.kilometrage, "kilométrage")?)
            },
            case_a_cocher(&s.plein_complet, "plein complet")?,
        ),
        TypeDepense::Peage => {
            if !s.quantite.trim().is_empty() || !s.kilometrage.trim().is_empty() {
                return Err(ErreurApp::metier(
                    "Un péage n'a ni quantité ni kilométrage : c'est un plein de carburant qui les porte.",
                ));
            }
            (None, 0, None, false)
        }
    };
    Ok(NouvelleDepense {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_depense,
        date_depense: date_obligatoire(&s.date_depense, "date")?,
        montant_centimes: montant_en_centimes(&s.montant, "montant")?,
        carburant,
        quantite_centiemes,
        kilometrage,
        plein_complet,
        libelle: texte_facultatif(&s.libelle, "station ou trajet")?,
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

/// Le carburant du véhicule de la saisie, pour proposer celui du plein. Un véhicule qui n'existe
/// pas est refusé ici, avant d'écrire, avec le message que donnerait la base.
async fn carburant_du_vehicule(pool: &PgPool, s: &SaisieDepense) -> Resultat<Option<Carburant>> {
    let vehicule = vehicules::obtenir(pool, identifiant(&s.vehicule_id, "véhicule")?)
        .await?
        .ok_or_else(|| ErreurApp::metier("Le véhicule choisi n'existe plus."))?;
    Ok(Carburant::depuis_code(&vehicule.carburant))
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Depense>> {
    Ok(depenses::lister(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Depense>> {
    Ok(depenses::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Depense> {
    depenses::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

/// Enregistre la dépense et, pour un plein avec kilométrage, relève le compteur du véhicule s'il
/// est plus bas. Tout est dans une même transaction : soit tout réussit, soit rien.
pub async fn creer(pool: &PgPool, saisie: &SaisieDepense) -> Resultat<Depense> {
    let validee = valider(saisie, carburant_du_vehicule(pool, saisie).await?)?;
    let mut transaction = pool.begin().await?;
    let id = depenses::inserer(&mut *transaction, &validee)
        .await
        .map_err(traduire)?;
    relever(&mut transaction, &validee).await?;
    transaction.commit().await?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieDepense) -> Resultat<Depense> {
    let validee = valider(saisie, carburant_du_vehicule(pool, saisie).await?)?;
    let mut transaction = pool.begin().await?;
    if !depenses::mettre_a_jour(&mut *transaction, id, &validee)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    relever(&mut transaction, &validee).await?;
    transaction.commit().await?;
    obtenir(pool, id).await
}

/// Le compteur du véhicule suit le plein, jamais en arrière (`GREATEST` en base).
async fn relever(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    validee: &NouvelleDepense,
) -> Resultat<()> {
    if let Some(kilometrage) = validee.kilometrage {
        vehicules::relever_kilometrage(&mut **transaction, validee.vehicule_id, kilometrage)
            .await?;
    }
    Ok(())
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if depenses::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plein() -> SaisieDepense {
        SaisieDepense {
            vehicule_id: "3".into(),
            type_depense: "carburant".into(),
            date_depense: "2026-10-12".into(),
            montant: "68,40".into(),
            carburant: String::new(),
            quantite: "45,2".into(),
            kilometrage: "61 200".into(),
            plein_complet: "oui".into(),
            libelle: " Station fictive ".into(),
            notes: String::new(),
        }
    }

    fn refus(s: &SaisieDepense) -> String {
        match valider(s, Some(Carburant::Diesel)) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        }
    }

    #[test]
    fn un_plein_se_valide_en_centimes_et_en_centiemes() {
        let v = valider(&plein(), Some(Carburant::Diesel)).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Carburant);
        assert_eq!(v.montant_centimes, 6840);
        assert_eq!(v.quantite_centiemes, 4520);
        assert_eq!(v.kilometrage, Some(61200));
        assert!(v.plein_complet);
        assert_eq!(v.libelle, "Station fictive");
    }

    #[test]
    fn le_carburant_du_plein_est_propose_d_apres_le_vehicule_ou_choisi() {
        let s = plein();
        for (vehicule, propose) in [
            (Some(Carburant::Diesel), CarburantPlein::Gazole),
            (Some(Carburant::Essence), CarburantPlein::Sp95E10),
            (Some(Carburant::Hybride), CarburantPlein::Sp95E10),
            (Some(Carburant::Electrique), CarburantPlein::Electricite),
            (Some(Carburant::Gpl), CarburantPlein::Gpl),
            (Some(Carburant::Autre), CarburantPlein::Autre),
            (None, CarburantPlein::Autre),
        ] {
            assert_eq!(valider(&s, vehicule).unwrap().carburant, Some(propose));
        }
        // Choisi, il prime : du SP98 dans un véhicule « essence ».
        let mut choisi = plein();
        choisi.carburant = "sp98".into();
        assert_eq!(
            valider(&choisi, Some(Carburant::Essence))
                .unwrap()
                .carburant,
            Some(CarburantPlein::Sp98)
        );
        choisi.carburant = "kerosene".into();
        assert!(refus(&choisi).contains("kerosene"));
        // Les unités suivent le carburant du plein.
        assert_eq!(CarburantPlein::Sp98.unite(), "L");
        assert_eq!(CarburantPlein::Electricite.unite(), "kWh");
        assert_eq!(CarburantPlein::Gnv.unite(), "kg");
    }

    #[test]
    fn la_quantite_et_le_kilometrage_sont_facultatifs_et_la_case_se_lit() {
        let mut s = plein();
        s.quantite = String::new();
        s.kilometrage = String::new();
        s.plein_complet = String::new();
        let v = valider(&s, Some(Carburant::Diesel)).unwrap();
        assert_eq!(v.quantite_centiemes, 0);
        assert_eq!(v.kilometrage, None);
        assert!(!v.plein_complet);
        for cochee in ["on", "oui", "true", "1"] {
            s.plein_complet = cochee.into();
            assert!(
                valider(&s, Some(Carburant::Diesel)).unwrap().plein_complet,
                "{cochee}"
            );
        }
        s.plein_complet = "peut-être".into();
        assert!(refus(&s).contains("plein complet"));
        s.plein_complet = String::new();
        s.quantite = "0".into();
        assert!(refus(&s).contains("quantité"));
        s.quantite = "abc".into();
        assert!(refus(&s).contains("quantité"));
        s.quantite = String::new();
        s.kilometrage = "-5".into();
        assert!(refus(&s).contains("kilométrage"));
    }

    #[test]
    fn un_peage_refuse_quantite_et_kilometrage_et_ignore_le_reste() {
        let mut s = plein();
        s.type_depense = "peage".into();
        assert!(refus(&s).contains("péage"));
        s.quantite = String::new();
        assert!(refus(&s).contains("péage"));
        s.kilometrage = String::new();
        s.carburant = "sp98".into();
        let v = valider(&s, Some(Carburant::Diesel)).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Peage);
        assert_eq!(
            (
                v.carburant,
                v.quantite_centiemes,
                v.kilometrage,
                v.plein_complet
            ),
            (None, 0, None, false)
        );
    }

    #[test]
    fn type_inconnu_vehicule_absent_et_montant_illisible_sont_refuses() {
        let mut s = plein();
        s.type_depense = "parking".into();
        assert!(refus(&s).contains("parking"));
        let mut s = plein();
        s.vehicule_id = String::new();
        assert!(refus(&s).contains("véhicule"));
        let mut s = plein();
        s.montant = "abc".into();
        assert!(refus(&s).contains("montant"));
        let mut s = plein();
        s.montant = String::new();
        assert_eq!(
            valider(&s, Some(Carburant::Diesel))
                .unwrap()
                .montant_centimes,
            0
        );
    }

    #[test]
    fn la_saisie_se_reconstruit_depuis_la_depense() {
        let d = Depense {
            id: 1,
            vehicule_id: 3,
            type_depense: "carburant".into(),
            date_depense: chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            montant_centimes: 6840,
            carburant: "sp98".into(),
            quantite_centiemes: 4520,
            kilometrage: Some(61200),
            plein_complet: true,
            libelle: "Station fictive".into(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AB-123-CD".into(),
            couleur: String::new(),
        };
        let s = SaisieDepense::depuis(&d);
        assert_eq!(s.montant, "68,40");
        assert_eq!(s.carburant, "sp98");
        assert_eq!(s.quantite, "45,2");
        assert_eq!(s.kilometrage, "61200");
        assert!(s.plein_complet_coche());
        assert_eq!(
            d.detail_fr(),
            "45,2 L SP98 · 61\u{202F}200 km · Station fictive"
        );
        let v = valider(&s, Some(Carburant::Essence)).unwrap();
        assert_eq!(v.quantite_centiemes, 4520);
        assert_eq!(v.carburant, Some(CarburantPlein::Sp98));

        let mut partiel = d.clone();
        partiel.plein_complet = false;
        partiel.quantite_centiemes = 0;
        partiel.kilometrage = None;
        partiel.carburant = "electricite".into();
        assert_eq!(
            partiel.detail_fr(),
            "Électricité · plein partiel · Station fictive"
        );
        assert_eq!(SaisieDepense::depuis(&partiel).quantite, "");
        assert_eq!(partiel.unite(), "kWh");

        let mut peage = d.clone();
        peage.type_depense = "peage".into();
        peage.carburant = String::new();
        peage.quantite_centiemes = 0;
        peage.kilometrage = None;
        peage.plein_complet = false;
        peage.libelle = "Lyon – Grenoble".into();
        assert_eq!(peage.detail_fr(), "Lyon – Grenoble");
        assert_eq!(peage.carburant_libelle(), "");
        assert!(!peage.est_un_plein());
    }
}
