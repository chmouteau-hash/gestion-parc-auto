//! Les dépenses pour rouler (#126) : validation et opérations.
//!
//! Un seul écran pour les pleins de carburant et les tickets de péage, distingués par un type
//! d'un vocabulaire fermé ([`TypeDepense`]). Seul un plein porte une quantité, le compteur et la
//! case « plein complet » : sur un péage, ces champs sont refusés quand ils sont renseignés, parce
//! qu'un formulaire sans script les envoie quand même (vides).
//!
//! Un plein noté avec son kilométrage **relève le compteur du véhicule** s'il est plus haut, comme
//! une intervention ; un plein ancien ne le fait pas reculer. Le kilométrage est facultatif : sans
//! lui, pas de consommation pour ce plein, mais on ne bloque pas qui n'a pas regardé le compteur.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::quantite_fr;
use crate::commun::types::TypeDepense;
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
    /// Quantité en litres ou en kWh (`42,3`) ; vide = non notée. Carburant seulement.
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

pub fn valider(s: &SaisieDepense) -> Resultat<NouvelleDepense> {
    let type_depense = code_parmi(&s.type_depense, "type", TypeDepense::depuis_code)?;
    let (quantite_centiemes, kilometrage, plein_complet) = match type_depense {
        TypeDepense::Carburant => (
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
            (0, None, false)
        }
    };
    Ok(NouvelleDepense {
        vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
        type_depense,
        date_depense: date_obligatoire(&s.date_depense, "date")?,
        montant_centimes: montant_en_centimes(&s.montant, "montant")?,
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
    let validee = valider(saisie)?;
    let mut transaction = pool.begin().await?;
    let id = depenses::inserer(&mut *transaction, &validee)
        .await
        .map_err(traduire)?;
    relever(&mut transaction, &validee).await?;
    transaction.commit().await?;
    obtenir(pool, id).await
}

pub async fn modifier(pool: &PgPool, id: i64, saisie: &SaisieDepense) -> Resultat<Depense> {
    let validee = valider(saisie)?;
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
            quantite: "45,2".into(),
            kilometrage: "61 200".into(),
            plein_complet: "oui".into(),
            libelle: " Station fictive ".into(),
            notes: String::new(),
        }
    }

    fn refus(s: &SaisieDepense) -> String {
        match valider(s) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        }
    }

    #[test]
    fn un_plein_se_valide_en_centimes_et_en_centiemes() {
        let v = valider(&plein()).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Carburant);
        assert_eq!(v.montant_centimes, 6840);
        assert_eq!(v.quantite_centiemes, 4520);
        assert_eq!(v.kilometrage, Some(61200));
        assert!(v.plein_complet);
        assert_eq!(v.libelle, "Station fictive");
    }

    #[test]
    fn la_quantite_et_le_kilometrage_sont_facultatifs_et_la_case_se_lit() {
        let mut s = plein();
        s.quantite = String::new();
        s.kilometrage = String::new();
        s.plein_complet = String::new();
        let v = valider(&s).unwrap();
        assert_eq!(v.quantite_centiemes, 0);
        assert_eq!(v.kilometrage, None);
        assert!(!v.plein_complet);
        for cochee in ["on", "oui", "true", "1"] {
            s.plein_complet = cochee.into();
            assert!(valider(&s).unwrap().plein_complet, "{cochee}");
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
    fn un_peage_refuse_quantite_et_kilometrage_et_ignore_la_case() {
        let mut s = plein();
        s.type_depense = "peage".into();
        assert!(refus(&s).contains("péage"));
        s.quantite = String::new();
        assert!(refus(&s).contains("péage"));
        s.kilometrage = String::new();
        let v = valider(&s).unwrap();
        assert_eq!(v.type_depense, TypeDepense::Peage);
        assert_eq!(
            (v.quantite_centiemes, v.kilometrage, v.plein_complet),
            (0, None, false)
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
        assert_eq!(valider(&s).unwrap().montant_centimes, 0);
    }

    #[test]
    fn la_saisie_se_reconstruit_depuis_la_depense() {
        let d = Depense {
            id: 1,
            vehicule_id: 3,
            type_depense: "carburant".into(),
            date_depense: chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            montant_centimes: 6840,
            quantite_centiemes: 4520,
            kilometrage: Some(61200),
            plein_complet: true,
            libelle: "Station fictive".into(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: "AB-123-CD".into(),
            couleur: String::new(),
            carburant: "diesel".into(),
        };
        let s = SaisieDepense::depuis(&d);
        assert_eq!(s.montant, "68,40");
        assert_eq!(s.quantite, "45,2");
        assert_eq!(s.kilometrage, "61200");
        assert!(s.plein_complet_coche());
        assert_eq!(d.detail_fr(), "45,2 L · 61\u{202F}200 km · Station fictive");
        let v = valider(&s).unwrap();
        assert_eq!(v.quantite_centiemes, 4520);

        let mut partiel = d.clone();
        partiel.plein_complet = false;
        partiel.quantite_centiemes = 0;
        partiel.kilometrage = None;
        partiel.carburant = "electrique".into();
        assert_eq!(partiel.detail_fr(), "plein partiel · Station fictive");
        assert_eq!(SaisieDepense::depuis(&partiel).quantite, "");
        assert_eq!(partiel.unite(), "kWh");

        let mut peage = d.clone();
        peage.type_depense = "peage".into();
        peage.quantite_centiemes = 0;
        peage.kilometrage = None;
        peage.plein_complet = false;
        peage.libelle = "Lyon – Grenoble".into();
        assert_eq!(peage.detail_fr(), "Lyon – Grenoble");
        assert!(!peage.est_un_plein());
    }
}
