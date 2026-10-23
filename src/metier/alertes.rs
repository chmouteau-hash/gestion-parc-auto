//! Vue alertes : échéances dépassées ou proches, permis de conduire compris, et opérations du plan
//! d'entretien qui pressent (#94, bloc 2b).
//!
//! Le classement est une fonction pure ([`niveau`], [`construire`]) testée sans base ; la lecture
//! des données est déléguée à `donnees::alertes` et à `metier::plan_entretien`.
//!
//! Une alerte a une borne en date (échéance, permis) **ou** en kilomètres (une opération du plan
//! dont le compteur approche) : « Vidange dans 800 km ». « Dépassée » et « à venir » se décident
//! ici pour les deux, avec un horizon en jours et un horizon en kilomètres ([`Horizon`]), pour que
//! la fiche, les listes, les alertes et l'accueil restent d'accord.

use std::collections::HashSet;

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgPool;

use crate::commun::format::{date_fr, nombre_fr};
use crate::commun::types::TypeEcheance;
use crate::donnees::alertes;
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::echeances::Echeance;
use crate::erreurs::Resultat;
use crate::metier::plan_entretien::{self, Borne, EtatPlan, SuiviVehicule};

/// Jusqu'où les alertes regardent : un nombre de jours avant une date limite, un nombre de
/// kilomètres avant une borne au compteur (`ALERTES_HORIZON_JOURS`, `ALERTES_HORIZON_KM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Horizon {
    pub jours: i64,
    pub km: i64,
}

impl Horizon {
    /// La borne en date tombe dans l'horizon (le jour même compris), ou elle est passée.
    pub fn date_proche(self, jours_restants: i64) -> bool {
        jours_restants <= self.jours
    }

    /// La borne au compteur est à moins de l'horizon en kilomètres, ou elle est passée.
    pub fn km_proche(self, km_restants: i64) -> bool {
        km_restants <= self.km
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Niveau {
    /// La date limite, ou la borne au compteur, est passée.
    Depassee,
    /// La borne tombe dans l'horizon (aujourd'hui compris).
    AVenir,
}

impl Niveau {
    pub fn libelle(self) -> &'static str {
        match self {
            Niveau::Depassee => "Dépassée",
            Niveau::AVenir => "À venir",
        }
    }

    /// Classe CSS de la ligne et de sa pastille.
    pub fn classe(self) -> &'static str {
        match self {
            Niveau::Depassee => "alerte-depassee",
            Niveau::AVenir => "alerte-a-venir",
        }
    }
}

/// Une ligne de la vue alertes, quelle que soit son origine (échéance, permis, plan d'entretien).
#[derive(Debug, Clone, Serialize)]
pub struct Alerte {
    pub niveau: Niveau,
    /// « Véhicule AB-123-CD » ou « Conducteur Léa Martin ».
    pub objet: String,
    /// « Contrôle technique », « À faire : Câble d'embrayage », « Permis de conduire »,
    /// « Vidange »…
    pub motif: String,
    /// Le constat d'une échéance « à faire » (« câble qui accroche ») ; vide sinon, et alors absent
    /// du JSON de l'API.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub constat: String,
    /// La borne en date ; absente pour une opération du plan qui presse au kilomètre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_limite: Option<NaiveDate>,
    /// Jours restants (négatif si dépassée) ; absent avec `date_limite`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jours: Option<i64>,
    /// La borne au compteur, pour une opération du plan qui presse au kilomètre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kilometrage_limite: Option<i32>,
    /// Kilomètres restants (négatif si dépassée) ; absent avec `kilometrage_limite`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kilometres: Option<i64>,
    /// Page à ouvrir pour traiter l'alerte.
    pub lien: String,
    /// Échéance que « Marquer traitée » ferme ; absente pour un permis ou le plan. Hors du JSON de
    /// l'API.
    #[serde(skip)]
    pub echeance_id: Option<i64>,
    /// La ligne du plan d'entretien d'où vient l'alerte ; absente sinon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<i64>,
    /// Pour une opération du plan, l'adresse de « C'est fait » : le formulaire d'intervention
    /// pré-rempli (#94, bloc 3). Absente sinon. Hors du JSON de l'API.
    #[serde(skip)]
    pub c_est_fait: Option<String>,
    /// Part du trajet déjà parcourue vers la borne, sur l'horizon, de 0 à 100 : ce que la barre de
    /// trajet remplit.
    #[serde(skip)]
    pub avancement: u8,
}

impl Alerte {
    /// La borne en clair : « 02/10/2026 » ou « 67 000 km ».
    pub fn limite_fr(&self) -> String {
        match (self.date_limite, self.kilometrage_limite) {
            (Some(d), _) => date_fr(d),
            (None, Some(k)) => format!("{} km", nombre_fr(i64::from(k))),
            (None, None) => String::new(),
        }
    }

    /// « dans 15 jours », « depuis 5 jours », « dans 800 km », « dépassée de 500 km ».
    pub fn delai_fr(&self) -> String {
        match (self.jours, self.kilometres) {
            (Some(j), _) => delai_fr(j),
            (None, Some(k)) => plan_entretien::delai_km_fr(k),
            (None, None) => String::new(),
        }
    }

    /// L'avancement arrondi à la dizaine (0, 10, … 100) : la barre de trajet a une classe par
    /// dizaine, puisque la consigne de sécurité interdit un style écrit dans la page.
    pub fn avancement_dizaine(&self) -> u8 {
        (self.avancement + 5) / 10 * 10
    }

    /// Vrai pour un permis de conduire.
    pub fn est_permis(&self) -> bool {
        self.echeance_id.is_none() && self.plan_id.is_none()
    }

    /// Vrai si la borne est en kilomètres.
    pub fn au_kilometre(&self) -> bool {
        self.kilometrage_limite.is_some()
    }

    /// Pour trier par urgence : la part de l'horizon qui reste, en millièmes (négative si la
    /// borne est passée). Un jour et un kilomètre ne se comparent pas ; leur part de l'horizon,
    /// si : 3 jours sur 30 et 100 km sur 1 000 pressent autant.
    fn urgence(&self, horizon: Horizon) -> i64 {
        match (self.jours, self.kilometres) {
            (Some(j), _) => j * 1000 / horizon.jours.max(1),
            (None, Some(k)) => k * 1000 / horizon.km.max(1),
            (None, None) => 0,
        }
    }
}

/// « dans 15 jours », « depuis 5 jours », « aujourd'hui »… à partir du nombre de jours restants
/// (négatif si la date est passée). Partout où une date limite s'affiche, son délai s'écrit ainsi.
pub fn delai_fr(jours: i64) -> String {
    match jours {
        j if j < -1 => format!("depuis {} jours", -j),
        -1 => "depuis hier".to_string(),
        0 => "aujourd'hui".to_string(),
        1 => "demain".to_string(),
        j => format!("dans {j} jours"),
    }
}

/// Part du trajet déjà parcourue vers la borne, de 0 à 100, sur l'horizon : 0 tant que la borne
/// est hors de l'horizon, 100 dès qu'elle est atteinte. Vaut pour des jours comme pour des
/// kilomètres.
pub fn avancement(restants: i64, horizon: i64) -> u8 {
    if horizon <= 0 || restants <= 0 {
        return 100;
    }
    if restants >= horizon {
        return 0;
    }
    (100 - restants * 100 / horizon) as u8
}

/// Classe une date limite par rapport à aujourd'hui.
pub fn niveau(date_limite: NaiveDate, aujourd_hui: NaiveDate) -> Niveau {
    if date_limite < aujourd_hui {
        Niveau::Depassee
    } else {
        Niveau::AVenir
    }
}

/// Dernier jour couvert par la vue alertes, compris : aujourd'hui plus l'horizon. Les listes et les
/// fiches s'en servent pour dire « à venir » : la règle n'est écrite qu'ici.
pub fn jusqu_au(aujourd_hui: NaiveDate, horizon_jours: i64) -> NaiveDate {
    aujourd_hui + chrono::Duration::days(horizon_jours)
}

/// Vrai si le permis n'est pas expiré mais expire dans l'horizon des alertes (le jour même compris).
pub fn permis_a_venir(
    date_expiration: NaiveDate,
    aujourd_hui: NaiveDate,
    horizon_jours: i64,
) -> bool {
    niveau(date_expiration, aujourd_hui) == Niveau::AVenir
        && date_expiration <= jusqu_au(aujourd_hui, horizon_jours)
}

/// Une échéance « Entretien périodique » d'un véhicule qui a un plan d'entretien : le plan la
/// remplace (décidé le 23/10/2026, #94, bloc 2b). Elle reste en base et sur la fiche, mais ne
/// compte ni dans les alertes ni sur l'accueil.
pub fn remplacee_par_le_plan(e: &Echeance, vehicules_avec_plan: &HashSet<i64>) -> bool {
    TypeEcheance::depuis_code(&e.type_echeance) == Some(TypeEcheance::EntretienPeriodique)
        && vehicules_avec_plan.contains(&e.vehicule_id)
}

fn alerte_d_echeance(e: &Echeance, aujourd_hui: NaiveDate, horizon: Horizon) -> Alerte {
    let jours = (e.date_limite - aujourd_hui).num_days();
    Alerte {
        niveau: niveau(e.date_limite, aujourd_hui),
        objet: format!("Véhicule {}", e.immatriculation),
        motif: e.type_libelle(),
        constat: e.constat.clone(),
        date_limite: Some(e.date_limite),
        jours: Some(jours),
        kilometrage_limite: None,
        kilometres: None,
        lien: format!("/vehicules/{}", e.vehicule_id),
        echeance_id: Some(e.id),
        plan_id: None,
        c_est_fait: None,
        avancement: avancement(jours, horizon.jours),
    }
}

fn alerte_de_permis(c: &Conducteur, aujourd_hui: NaiveDate, horizon: Horizon) -> Alerte {
    let jours = (c.date_expiration_permis - aujourd_hui).num_days();
    Alerte {
        niveau: niveau(c.date_expiration_permis, aujourd_hui),
        objet: format!("Conducteur {}", c.nom_complet()),
        motif: "Permis de conduire".to_string(),
        constat: String::new(),
        date_limite: Some(c.date_expiration_permis),
        jours: Some(jours),
        kilometrage_limite: None,
        kilometres: None,
        lien: format!("/conducteurs/{}", c.id),
        echeance_id: None,
        plan_id: None,
        c_est_fait: None,
        avancement: avancement(jours, horizon.jours),
    }
}

/// L'alerte d'une opération du plan, si elle presse (« dépassée » ou « à venir », décidé par
/// [`plan_entretien::suivre`] avec le même [`Horizon`]). La borne montrée est celle qui la met en
/// alerte ; si les deux le font, celle qui presse le plus sur la fiche.
fn alerte_de_plan(s: &SuiviVehicule, horizon: Horizon) -> Option<Alerte> {
    let suivi = &s.suivi;
    let niveau = match suivi.etat {
        EtatPlan::Depassee => Niveau::Depassee,
        EtatPlan::AVenir => Niveau::AVenir,
        EtatPlan::AJour | EtatPlan::Inconnue => return None,
    };
    let par_km = suivi.borne_en_alerte(horizon)? == Borne::Kilometrage;
    let (date_limite, jours, kilometrage_limite, kilometres, avance) = if par_km {
        let k = suivi.reste_km?;
        (
            None,
            None,
            suivi.prochaine_km,
            Some(k),
            avancement(k, horizon.km),
        )
    } else {
        let j = suivi.reste_jours?;
        (
            suivi.prochaine_date,
            Some(j),
            None,
            None,
            avancement(j, horizon.jours),
        )
    };
    Some(Alerte {
        niveau,
        objet: format!("Véhicule {}", s.immatriculation),
        motif: suivi.ligne.operation_libelle().to_string(),
        constat: String::new(),
        date_limite,
        jours,
        kilometrage_limite,
        kilometres,
        lien: format!("/vehicules/{}#plan", s.vehicule_id),
        echeance_id: None,
        plan_id: Some(suivi.ligne.id),
        c_est_fait: Some(s.c_est_fait()),
        avancement: avance,
    })
}

/// Construit la liste d'alertes à partir des données déjà filtrées par la base (échéances et
/// permis dans l'horizon) et du plan d'entretien calculé, triée par urgence : la part de
/// l'horizon qui reste, les plus pressantes en premier. Fonction pure.
pub fn construire(
    echeances: &[Echeance],
    conducteurs: &[Conducteur],
    plan: &[SuiviVehicule],
    aujourd_hui: NaiveDate,
    horizon: Horizon,
) -> Vec<Alerte> {
    let mut alertes: Vec<Alerte> = echeances
        .iter()
        .map(|e| alerte_d_echeance(e, aujourd_hui, horizon))
        .chain(
            conducteurs
                .iter()
                .map(|c| alerte_de_permis(c, aujourd_hui, horizon)),
        )
        .chain(plan.iter().filter_map(|s| alerte_de_plan(s, horizon)))
        .collect();
    alertes.sort_by(|a, b| {
        a.urgence(horizon)
            .cmp(&b.urgence(horizon))
            .then(a.date_limite.cmp(&b.date_limite))
            .then(a.objet.cmp(&b.objet))
            .then(a.motif.cmp(&b.motif))
    });
    alertes
}

/// Lit la base et construit les alertes : échéances et permis jusqu'à `aujourd_hui` plus
/// l'horizon, opérations du plan qui pressent. L'« Entretien périodique » d'un véhicule qui a un
/// plan n'en est pas.
pub async fn calculer(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon: Horizon,
) -> Resultat<Vec<Alerte>> {
    let avec_plan = plan_entretien::vehicules_avec_plan(pool).await?;
    let plan = plan_entretien::suivis_du_parc(pool, aujourd_hui, horizon).await?;
    calculer_avec_le_plan(pool, aujourd_hui, horizon, &plan, &avec_plan).await
}

/// [`calculer`], avec le plan du parc déjà lu et calculé.
pub async fn calculer_avec_le_plan(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon: Horizon,
    plan: &[SuiviVehicule],
    vehicules_avec_plan: &HashSet<i64>,
) -> Resultat<Vec<Alerte>> {
    let limite = jusqu_au(aujourd_hui, horizon.jours);
    let echeances: Vec<Echeance> = alertes::echeances_a_traiter(pool, limite)
        .await?
        .into_iter()
        .filter(|e| !remplacee_par_le_plan(e, vehicules_avec_plan))
        .collect();
    let conducteurs = alertes::conducteurs_permis_expirant(pool, limite).await?;
    Ok(construire(
        &echeances,
        &conducteurs,
        plan,
        aujourd_hui,
        horizon,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::plan_entretien::LignePlan;
    use crate::metier::plan_entretien::Suivi;
    use chrono::Utc;

    const HORIZON: Horizon = Horizon {
        jours: 30,
        km: 1000,
    };

    fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, j).unwrap()
    }

    #[test]
    fn classement() {
        let aujourd_hui = jour(2026, 10, 11);
        assert_eq!(niveau(jour(2026, 10, 10), aujourd_hui), Niveau::Depassee);
        assert_eq!(niveau(jour(2026, 10, 11), aujourd_hui), Niveau::AVenir);
        assert_eq!(niveau(jour(2026, 11, 10), aujourd_hui), Niveau::AVenir);
    }

    #[test]
    fn delai_et_avancement() {
        assert_eq!(delai_fr(-10), "depuis 10 jours");
        assert_eq!(delai_fr(-1), "depuis hier");
        assert_eq!(delai_fr(0), "aujourd'hui");
        assert_eq!(delai_fr(1), "demain");
        assert_eq!(delai_fr(15), "dans 15 jours");

        // Le trajet se remplit sur l'horizon : à mi-chemin à 15 jours sur 30, plein le jour même
        // et au-delà, vide tant que l'échéance est hors de l'horizon. Les kilomètres de même.
        assert_eq!(avancement(30, 30), 0);
        assert_eq!(avancement(45, 30), 0);
        assert_eq!(avancement(15, 30), 50);
        assert_eq!(avancement(3, 30), 90);
        assert_eq!(avancement(0, 30), 100);
        assert_eq!(avancement(-5, 30), 100);
        assert_eq!(avancement(5, 0), 100);
        assert_eq!(avancement(800, 1000), 20);
    }

    #[test]
    fn horizon_et_permis_a_venir() {
        let aujourd_hui = jour(2026, 10, 11);
        assert_eq!(jusqu_au(aujourd_hui, 30), jour(2026, 11, 10));
        let a_venir = |date| permis_a_venir(date, aujourd_hui, 30);
        // Expiré : ce n'est plus « à venir ».
        assert!(!a_venir(jour(2026, 10, 10)));
        // Valable le jour même, et jusqu'au dernier jour de l'horizon compris.
        assert!(a_venir(jour(2026, 10, 11)));
        assert!(a_venir(jour(2026, 11, 10)));
        // Au-delà de l'horizon, rien à dire.
        assert!(!a_venir(jour(2026, 11, 11)));
        // L'horizon en kilomètres : 1 000 km compris.
        assert!(HORIZON.km_proche(1000) && HORIZON.km_proche(-200) && !HORIZON.km_proche(1001));
        assert!(HORIZON.date_proche(30) && !HORIZON.date_proche(31));
    }

    fn echeance(id: i64, type_echeance: &str, date_limite: NaiveDate) -> Echeance {
        Echeance {
            id,
            vehicule_id: 10 + id,
            type_echeance: type_echeance.into(),
            date_limite,
            statut: "ouverte".into(),
            cree_le: Utc::now(),
            immatriculation: format!("AA-00{id}-AA"),
            couleur: String::new(),
            operation: String::new(),
            libelle: String::new(),
            constat: String::new(),
        }
    }

    #[test]
    fn construction_triee_et_libellee() {
        let aujourd_hui = jour(2026, 10, 11);
        let echeances = vec![
            echeance(1, "assurance", jour(2026, 10, 20)),
            echeance(2, "controle_technique", jour(2026, 10, 1)),
        ];
        let conducteurs = vec![Conducteur {
            id: 5,
            nom: "Martin".into(),
            prenom: "Léa".into(),
            service: "".into(),
            date_expiration_permis: jour(2026, 10, 11),
            statut: "actif".into(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }];
        let alertes = construire(&echeances, &conducteurs, &[], aujourd_hui, HORIZON);
        assert_eq!(alertes.len(), 3);

        assert_eq!(alertes[0].niveau, Niveau::Depassee);
        assert_eq!(alertes[0].motif, "Contrôle technique");
        assert_eq!(alertes[0].jours, Some(-10));
        assert_eq!(alertes[0].delai_fr(), "depuis 10 jours");
        assert_eq!(alertes[0].limite_fr(), "01/10/2026");
        assert_eq!(alertes[0].lien, "/vehicules/12");
        assert_eq!(alertes[0].avancement_dizaine(), 100);

        assert_eq!(alertes[1].objet, "Conducteur Léa Martin");
        assert_eq!(alertes[1].motif, "Permis de conduire");
        assert_eq!(alertes[1].delai_fr(), "aujourd'hui");
        assert_eq!(alertes[1].lien, "/conducteurs/5");
        assert!(alertes[1].est_permis());

        assert_eq!(alertes[2].niveau, Niveau::AVenir);
        assert_eq!(alertes[2].delai_fr(), "dans 9 jours");
        assert_eq!(alertes[2].avancement_dizaine(), 70);
    }

    fn suivi(
        id: i64,
        operation: &str,
        etat: EtatPlan,
        reste_km: Option<i64>,
        reste_jours: Option<i64>,
        borne: Borne,
    ) -> SuiviVehicule {
        SuiviVehicule {
            vehicule_id: 20 + id,
            immatriculation: format!("PL-00{id}-AN"),
            kilometrage: 60_000,
            suivi: Suivi {
                ligne: LignePlan {
                    id,
                    vehicule_id: 20 + id,
                    operation: operation.into(),
                    periodicite_km: reste_km.map(|_| 15000),
                    periodicite_mois: reste_jours.map(|_| 12),
                    cree_le: Utc::now(),
                },
                derniere_fois: None,
                prochaine_km: reste_km.map(|r| 60000 + r as i32),
                prochaine_date: reste_jours.map(|j| jour(2026, 10, 11) + chrono::Duration::days(j)),
                reste_km,
                reste_jours,
                avancement: 0,
                borne: Some(borne),
                etat,
                mise_en_service: jour(2020, 1, 1),
            },
        }
    }

    #[test]
    fn une_operation_du_plan_qui_presse_est_une_alerte_au_kilometre_ou_a_la_date() {
        let aujourd_hui = jour(2026, 10, 11);
        let plan = vec![
            // Au kilomètre seulement : dans 800 km.
            suivi(
                1,
                "vidange",
                EtatPlan::AVenir,
                Some(800),
                None,
                Borne::Kilometrage,
            ),
            // Les deux bornes, seule la date presse (dans 20 jours ; 4 000 km de marge).
            suivi(
                2,
                "filtre_air",
                EtatPlan::AVenir,
                Some(4000),
                Some(20),
                Borne::Kilometrage,
            ),
            // Dépassée de 300 km.
            suivi(
                3,
                "plaquettes",
                EtatPlan::Depassee,
                Some(-300),
                Some(90),
                Borne::Kilometrage,
            ),
            // À jour et inconnue : pas d'alerte.
            suivi(
                4,
                "bougies",
                EtatPlan::AJour,
                Some(9000),
                None,
                Borne::Kilometrage,
            ),
            suivi(5, "batterie", EtatPlan::Inconnue, None, None, Borne::Date),
        ];
        let echeances = vec![echeance(1, "assurance", jour(2026, 10, 14))];
        let alertes = construire(&echeances, &[], &plan, aujourd_hui, HORIZON);
        let resume: Vec<(String, String, String)> = alertes
            .iter()
            .map(|a| (a.motif.clone(), a.limite_fr(), a.delai_fr()))
            .collect();
        // Rangées par part de l'horizon restante : -300 km (-0,3), 3 jours (0,1), 20 jours
        // (0,67), 800 km (0,8).
        assert_eq!(
            resume,
            [
                (
                    "Plaquettes de frein".into(),
                    "59\u{202F}700 km".into(),
                    "dépassée de 300 km".into()
                ),
                (
                    "Assurance".into(),
                    "14/10/2026".into(),
                    "dans 3 jours".into()
                ),
                (
                    "Filtre à air".into(),
                    "31/10/2026".into(),
                    "dans 20 jours".into()
                ),
                (
                    "Vidange".into(),
                    "60\u{202F}800 km".into(),
                    "dans 800 km".into()
                ),
            ]
        );
        assert_eq!(alertes[0].niveau, Niveau::Depassee);
        assert_eq!(alertes[0].plan_id, Some(3));
        assert_eq!(alertes[0].lien, "/vehicules/23#plan");
        assert!(alertes[0].au_kilometre() && !alertes[0].est_permis());
        assert_eq!(alertes[3].avancement_dizaine(), 20);
        assert_eq!(alertes[2].kilometres, None);
    }

    #[test]
    fn l_entretien_periodique_d_un_vehicule_avec_plan_est_remplace() {
        let avec_plan: HashSet<i64> = [11].into_iter().collect();
        assert!(remplacee_par_le_plan(
            &echeance(1, "entretien_periodique", jour(2026, 10, 1)),
            &avec_plan
        ));
        // Un autre type, ou un véhicule sans plan : rien ne change.
        assert!(!remplacee_par_le_plan(
            &echeance(1, "controle_technique", jour(2026, 10, 1)),
            &avec_plan
        ));
        assert!(!remplacee_par_le_plan(
            &echeance(2, "entretien_periodique", jour(2026, 10, 1)),
            &avec_plan
        ));
    }
}
