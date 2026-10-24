//! Les relevés du compteur et son historique (#94, bloc 4 ; retour #4). Le compteur d'un véhicule
//! est le plus haut kilométrage de son historique : ses relevés, ses interventions et ses pleins.
//! Chaque écriture de l'une de ces sources le recalcule dans sa transaction
//! ([`vehicules::recalculer_kilometrage`]) : corriger ou supprimer la saisie fautive le fait
//! redescendre.

use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::format::{date_fr, nombre_fr};
use crate::donnees::releves::{self, DernierReleve, LigneHistorique, Releve};
use crate::donnees::vehicules;
use crate::erreurs::{ErreurApp, Resultat};
use crate::metier::validation::{date_obligatoire, entier_positif};
use crate::metier::vehicules as metier_vehicules;

/// Ce que le formulaire (ou le JSON) envoie pour un relevé.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieReleve {
    #[serde(default)]
    pub date_releve: String,
    #[serde(default)]
    pub kilometrage: String,
}

impl SaisieReleve {
    pub fn depuis(r: &Releve) -> SaisieReleve {
        SaisieReleve {
            date_releve: r.date_releve.to_string(),
            kilometrage: r.kilometrage.to_string(),
        }
    }
}

/// Un relevé se fait au plus tard aujourd'hui, avec un kilométrage entier positif.
pub fn valider(s: &SaisieReleve, aujourd_hui: NaiveDate) -> Resultat<(NaiveDate, i32)> {
    let date = date_obligatoire(&s.date_releve, "date du relevé")?;
    if date > aujourd_hui {
        return Err(ErreurApp::metier(
            "Un relevé ne peut pas être dans le futur : le compteur se lit le jour même.",
        ));
    }
    Ok((date, entier_positif(&s.kilometrage, "kilométrage")?))
}

/// Relève le compteur aujourd'hui (« Relever », sur la fiche et la carte d'accueil). Plus bas que
/// le compteur, le relevé est refusé : le compteur est le plus haut de l'historique, et la saisie
/// fautive est ailleurs, à corriger dans l'historique.
pub async fn relever(
    pool: &PgPool,
    vehicule_id: i64,
    saisie: &str,
    aujourd_hui: NaiveDate,
) -> Resultat<Releve> {
    let kilometrage = entier_positif(saisie, "kilométrage")?;
    let vehicule = metier_vehicules::obtenir(pool, vehicule_id).await?;
    if kilometrage < vehicule.kilometrage {
        return Err(ErreurApp::metier(format!(
            "Le compteur est déjà à {} : le kilométrage ne peut pas baisser. S'il est faux, corrigez la saisie fautive dans l'historique du compteur.",
            vehicule.kilometrage_fr()
        )));
    }
    let mut transaction = pool.begin().await?;
    let releve = releves::inserer(&mut *transaction, vehicule_id, aujourd_hui, kilometrage).await?;
    vehicules::recalculer_kilometrage(&mut *transaction, vehicule_id).await?;
    transaction.commit().await?;
    Ok(releve)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Releve> {
    releves::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

/// Corrige la date ou le kilométrage d'un relevé ; le compteur suit, à la hausse comme à la
/// baisse.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieReleve,
    aujourd_hui: NaiveDate,
) -> Resultat<Releve> {
    let (date, kilometrage) = valider(saisie, aujourd_hui)?;
    let mut transaction = pool.begin().await?;
    let releve = releves::mettre_a_jour(&mut *transaction, id, date, kilometrage)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    vehicules::recalculer_kilometrage(&mut *transaction, releve.vehicule_id).await?;
    transaction.commit().await?;
    Ok(releve)
}

/// Supprime un relevé ; le compteur suit le reste de l'historique. Rend le véhicule.
pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<i64> {
    let mut transaction = pool.begin().await?;
    let vehicule_id = releves::supprimer(&mut *transaction, id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    vehicules::recalculer_kilometrage(&mut *transaction, vehicule_id).await?;
    transaction.commit().await?;
    Ok(vehicule_id)
}

pub async fn historique(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<LigneHistorique>> {
    Ok(releves::historique(pool, vehicule_id).await?)
}

/// La ligne d'où vient le compteur : la plus récente de celles qui portent le plus haut
/// kilométrage (l'historique arrive du plus récent au plus ancien).
pub fn source_du_compteur(historique: &[LigneHistorique]) -> Option<usize> {
    let plus_haut = historique.iter().map(|l| l.kilometrage).max()?;
    historique.iter().position(|l| l.kilometrage == plus_haut)
}

/// La fenêtre du rythme : les trois derniers mois de l'historique (#94, bloc 4).
pub const RYTHME_MOIS: u32 = 3;
/// Deux points trop proches ne disent pas un rythme : il faut au moins deux semaines entre eux.
pub const RYTHME_ECART_MINIMUM_JOURS: i64 = 14;
/// Au-delà d'un mois sans relevé, le compteur est « à relever » (#94, bloc 4).
pub const RELEVE_ANCIEN_MOIS: u32 = 1;
/// Une estimation à plus de dix ans ne veut plus rien dire.
const ESTIMATION_MAX_JOURS: i64 = 3650;

const MOIS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// Le rythme d'un véhicule : les kilomètres qu'il fait par jour, mesurés entre le plus ancien et
/// le plus récent point de son historique des trois derniers mois (#94, bloc 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rythme {
    pub km_par_jour: f64,
    /// La date du point le plus récent : les estimations partent de lui.
    pub dernier: NaiveDate,
}

impl Rythme {
    /// « environ 1 520 km par mois » (un mois moyen de 30,44 jours), à la dizaine près.
    pub fn par_mois_fr(&self) -> String {
        let par_mois = (self.km_par_jour * 30.44 / 10.0).round() as i64 * 10;
        format!("environ {} km par mois", nombre_fr(par_mois))
    }

    /// Le jour où le véhicule aura fait `reste` kilomètres de plus que son dernier point, au
    /// rythme actuel. Rien si ce jour est déjà passé (le compteur n'a pas été relevé depuis), ou
    /// s'il est à plus de dix ans.
    pub fn date_dans(&self, reste: i64, aujourd_hui: NaiveDate) -> Option<NaiveDate> {
        if reste <= 0 {
            return None;
        }
        let jours = (reste as f64 / self.km_par_jour).ceil() as i64;
        if jours > ESTIMATION_MAX_JOURS {
            return None;
        }
        let date = self.dernier.checked_add_signed(Duration::days(jours))?;
        (date >= aujourd_hui).then_some(date)
    }
}

/// Le rythme du véhicule sur les trois derniers mois de son historique : du plus ancien au plus
/// récent point de la fenêtre. Rien sans deux points espacés d'au moins deux semaines, ni quand
/// le compteur n'a pas avancé. Les saisies datées dans le futur ne comptent pas.
pub fn rythme(historique: &[LigneHistorique], aujourd_hui: NaiveDate) -> Option<Rythme> {
    let debut = aujourd_hui.checked_sub_months(Months::new(RYTHME_MOIS))?;
    let fenetre = || {
        historique
            .iter()
            .filter(|l| l.date >= debut && l.date <= aujourd_hui)
    };
    let dernier = fenetre().max_by_key(|l| (l.date, l.kilometrage))?;
    let premier = fenetre().min_by_key(|l| (l.date, l.kilometrage))?;
    let jours = (dernier.date - premier.date).num_days();
    let km = i64::from(dernier.kilometrage) - i64::from(premier.kilometrage);
    (jours >= RYTHME_ECART_MINIMUM_JOURS && km > 0).then(|| Rythme {
        km_par_jour: km as f64 / jours as f64,
        dernier: dernier.date,
    })
}

/// « vers mi-janvier », « vers début mars 2027 » : une date estimée, à la dizaine de jours près ;
/// l'année seulement si ce n'est pas celle d'aujourd'hui.
pub fn vers_fr(date: NaiveDate, aujourd_hui: NaiveDate) -> String {
    let mois = MOIS[date.month0() as usize];
    let annee = if date.year() == aujourd_hui.year() {
        String::new()
    } else {
        format!(" {}", date.year())
    };
    match date.day() {
        1..=10 => format!("vers début {mois}{annee}"),
        11..=20 => format!("vers mi-{mois}{annee}"),
        _ => format!("vers fin {mois}{annee}"),
    }
}

/// « vers mi-janvier au rythme actuel » : ce que les écrans ajoutent à une borne en kilomètres.
pub fn estimation_fr(date: NaiveDate, aujourd_hui: NaiveDate) -> String {
    format!("{} au rythme actuel", vers_fr(date, aujourd_hui))
}

/// Vrai quand le dernier point de l'historique a plus d'un mois : le compteur est à relever.
pub fn releve_ancien(dernier: NaiveDate, aujourd_hui: NaiveDate) -> bool {
    aujourd_hui
        .checked_sub_months(Months::new(RELEVE_ANCIEN_MOIS))
        .is_some_and(|limite| dernier < limite)
}

/// Un véhicule dont le compteur n'a pas été relevé depuis plus d'un mois : ni relevé, ni
/// intervention, ni plein (#94, bloc 4).
#[derive(Debug, Clone)]
pub struct ARelever {
    pub releve: DernierReleve,
    /// Jours depuis le dernier point de son historique.
    pub jours: i64,
}

impl ARelever {
    /// « le 12/09/2026 · il y a 42 jours ».
    pub fn depuis_fr(&self) -> String {
        format!(
            "le {} · il y a {} jours",
            date_fr(self.releve.dernier),
            self.jours
        )
    }

    pub fn kilometrage_fr(&self) -> String {
        format!("{} km", nombre_fr(i64::from(self.releve.kilometrage)))
    }
}

/// Les véhicules du parc, réformés exceptés, dont le compteur est à relever : le plus ancien
/// relevé en premier.
pub async fn a_relever(pool: &PgPool, aujourd_hui: NaiveDate) -> Resultat<Vec<ARelever>> {
    let mut liste: Vec<ARelever> = releves::derniers_releves(pool, aujourd_hui)
        .await?
        .into_iter()
        .filter(|r| releve_ancien(r.dernier, aujourd_hui))
        .map(|releve| ARelever {
            jours: (aujourd_hui - releve.dernier).num_days(),
            releve,
        })
        .collect();
    liste.sort_by(|a, b| {
        a.releve
            .dernier
            .cmp(&b.releve.dernier)
            .then_with(|| a.releve.immatriculation.cmp(&b.releve.immatriculation))
    });
    Ok(liste)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(origine: &str, date: (i32, u32, u32), kilometrage: i32) -> LigneHistorique {
        LigneHistorique {
            origine: origine.into(),
            id: 1,
            date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            kilometrage,
            detail: match origine {
                "intervention" => "revision".into(),
                "depense" => "carburant".into(),
                _ => String::new(),
            },
        }
    }

    #[test]
    fn un_releve_n_est_pas_dans_le_futur() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let saisie = |date: &str, km: &str| SaisieReleve {
            date_releve: date.into(),
            kilometrage: km.into(),
        };
        assert_eq!(
            valider(&saisie("2026-10-24", "48 000"), aujourd_hui).unwrap(),
            (aujourd_hui, 48_000)
        );
        assert!(valider(&saisie("2026-10-25", "48000"), aujourd_hui).is_err());
        assert!(valider(&saisie("2026-10-20", "-3"), aujourd_hui).is_err());
        assert!(valider(&saisie("", "48000"), aujourd_hui).is_err());
    }

    #[test]
    fn le_compteur_vient_de_la_plus_recente_des_lignes_les_plus_hautes() {
        let historique = vec![
            ligne("releve", (2026, 10, 20), 48_000),
            ligne("intervention", (2026, 9, 1), 99_000),
            ligne("depense", (2026, 8, 1), 99_000),
            ligne("releve", (2026, 1, 1), 30_000),
        ];
        assert_eq!(source_du_compteur(&historique), Some(1));
        assert_eq!(historique[1].d_ou_fr(), "l'intervention du 01/09/2026");
        assert_eq!(historique[2].d_ou_fr(), "le plein du 01/08/2026");
        assert_eq!(historique[0].origine_fr(), "Relevé");
        assert_eq!(historique[1].origine_fr(), "Intervention · Révision");
        assert_eq!(historique[2].origine_fr(), "Plein");
        assert_eq!(source_du_compteur(&[]), None);
    }

    #[test]
    fn le_rythme_se_mesure_sur_les_trois_derniers_mois() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        // 3 000 km en 60 jours dans la fenêtre ; le point de janvier, hors fenêtre, ne compte pas.
        let historique = vec![
            ligne("releve", (2026, 10, 20), 48_000),
            ligne("depense", (2026, 8, 21), 45_000),
            ligne("intervention", (2026, 1, 10), 20_000),
        ];
        let r = rythme(&historique, aujourd_hui).unwrap();
        assert_eq!(r.dernier, NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
        assert!((r.km_par_jour - 50.0).abs() < 1e-9);
        assert_eq!(r.par_mois_fr(), "environ 1\u{202F}520 km par mois");
        // 1 800 km à 50 km par jour : 36 jours après le dernier point.
        assert_eq!(
            r.date_dans(1_800, aujourd_hui),
            NaiveDate::from_ymd_opt(2026, 11, 25)
        );
        // Une borne passée, ou une date estimée déjà passée : rien.
        assert_eq!(r.date_dans(-200, aujourd_hui), None);
        assert_eq!(r.date_dans(100, aujourd_hui), None);
        // Trop proches, ou un compteur qui n'avance pas : pas de rythme.
        let proches = vec![
            ligne("releve", (2026, 10, 20), 48_000),
            ligne("releve", (2026, 10, 10), 47_000),
        ];
        assert_eq!(rythme(&proches, aujourd_hui), None);
        let immobile = vec![
            ligne("releve", (2026, 10, 20), 48_000),
            ligne("releve", (2026, 8, 1), 48_000),
        ];
        assert_eq!(rythme(&immobile, aujourd_hui), None);
        // Une saisie datée dans le futur ne compte pas.
        let futur = vec![
            ligne("intervention", (2026, 12, 1), 60_000),
            ligne("releve", (2026, 10, 20), 48_000),
        ];
        assert_eq!(rythme(&futur, aujourd_hui), None);
    }

    #[test]
    fn une_date_estimee_se_dit_a_la_dizaine_de_jours_pres() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        assert_eq!(vers_fr(d(2026, 11, 3), aujourd_hui), "vers début novembre");
        assert_eq!(vers_fr(d(2026, 12, 15), aujourd_hui), "vers mi-décembre");
        assert_eq!(
            vers_fr(d(2027, 1, 28), aujourd_hui),
            "vers fin janvier 2027"
        );
        assert_eq!(
            estimation_fr(d(2027, 2, 12), aujourd_hui),
            "vers mi-février 2027 au rythme actuel"
        );
    }

    #[test]
    fn un_releve_est_ancien_apres_un_mois() {
        let aujourd_hui = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let d = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        assert!(!releve_ancien(d(2026, 9, 24), aujourd_hui));
        assert!(releve_ancien(d(2026, 9, 23), aujourd_hui));
    }
}
