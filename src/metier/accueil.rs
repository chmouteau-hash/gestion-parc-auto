//! Page d'arrivée de chaque profil : ce qui demande une action, et quand.
//!
//! Autoclair (particulier) montre une carte par véhicule avec sa prochaine échéance ; Autoclair
//! Pro (entreprise) un tableau de bord chiffré. Les deux reposent sur les mêmes règles que la
//! vue alertes (`metier::alertes`, `metier::echeances`) : un même nombre de jours partout.
//! Les calculs sont des fonctions pures, testées sans base ; la lecture est en bas du fichier.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::donnees::affectations::Affectation;
use crate::donnees::documents;
use crate::donnees::echeances::{self, Echeance};
use crate::donnees::interventions::{self, Intervention};
use crate::donnees::vehicules::{self, Vehicule};
use crate::erreurs::Resultat;
use crate::metier::alertes::{self, delai_fr, Alerte, Niveau};
use crate::metier::echeances::{etat, EtatEcheance};

/// La prochaine échéance ouverte d'un véhicule, avec ce que la borne en dit.
#[derive(Debug, Clone)]
pub struct Prochaine {
    pub echeance: Echeance,
    /// Jours restants, négatif si dépassée.
    pub jours: i64,
    pub etat: EtatEcheance,
}

impl Prochaine {
    pub fn delai_fr(&self) -> String {
        delai_fr(self.jours)
    }
}

/// Un véhicule tel que la carte de l'accueil le résume.
#[derive(Debug, Clone)]
pub struct ResumeVehicule {
    pub vehicule: Vehicule,
    /// Absente quand aucune échéance n'est ouverte.
    pub prochaine: Option<Prochaine>,
    /// La photo de la fiche du véhicule (la choisie, à défaut la plus récente), pour la vignette
    /// de la carte.
    pub photo: Option<i64>,
}

impl ResumeVehicule {
    /// Classe de la borne : la couleur de ce que l'échéance a de plus pressant. Au-delà de
    /// l'horizon des alertes, ou sans échéance, le véhicule est « à jour ».
    pub fn classe_borne(&self) -> &'static str {
        match &self.prochaine {
            Some(p) if p.etat == EtatEcheance::Depassee => "borne-depassee",
            Some(p) if p.etat == EtatEcheance::AVenir => "borne-a-venir",
            Some(_) => "borne-a-jour",
            None if self.vehicule.est_reforme() => "borne-neutre",
            None => "borne-a-jour",
        }
    }

    /// Le chiffre de la borne : les jours restants, en valeur absolue ; un tiret sans échéance.
    pub fn borne_valeur(&self) -> String {
        match &self.prochaine {
            Some(p) => p.jours.abs().to_string(),
            None => "—".to_string(),
        }
    }

    /// Sous le chiffre : « jours », « jour », ou « retard » quand la date est passée.
    pub fn borne_unite(&self) -> &'static str {
        match &self.prochaine {
            Some(p) if p.jours < 0 => "retard",
            Some(p) if p.jours.abs() == 1 => "jour",
            Some(_) => "jours",
            None => "",
        }
    }

    /// Vrai quand l'échéance presse (dépassée ou dans l'horizon des alertes) : c'est alors que
    /// la carte propose « C'est fait ». Une échéance lointaine n'a rien à cocher.
    pub fn presse(&self) -> bool {
        matches!(
            &self.prochaine,
            Some(p) if p.etat == EtatEcheance::Depassee || p.etat == EtatEcheance::AVenir
        )
    }

    /// Pour trier : le plus pressant en premier, les véhicules sans échéance ensuite, les
    /// réformés en dernier.
    fn rang(&self) -> (u8, i64) {
        match &self.prochaine {
            Some(p) => (0, p.jours),
            None if self.vehicule.est_reforme() => (2, 0),
            None => (1, 0),
        }
    }
}

/// Associe à chaque véhicule sa prochaine échéance ouverte. `echeances_ouvertes` est trié par
/// date limite : la première rencontrée pour un véhicule est la bonne.
pub fn resumer(
    vehicules: Vec<Vehicule>,
    echeances_ouvertes: &[Echeance],
    aujourd_hui: NaiveDate,
    horizon_jours: i64,
) -> Vec<ResumeVehicule> {
    let mut resumes: Vec<ResumeVehicule> = vehicules
        .into_iter()
        .map(|vehicule| {
            let prochaine = echeances_ouvertes
                .iter()
                .find(|e| e.vehicule_id == vehicule.id)
                .map(|e| Prochaine {
                    jours: (e.date_limite - aujourd_hui).num_days(),
                    etat: etat(e, aujourd_hui, horizon_jours),
                    echeance: e.clone(),
                });
            ResumeVehicule {
                vehicule,
                prochaine,
                photo: None,
            }
        })
        .collect();
    resumes.sort_by(|a, b| {
        a.rang()
            .cmp(&b.rang())
            .then_with(|| a.vehicule.immatriculation.cmp(&b.vehicule.immatriculation))
    });
    resumes
}

/// Les chiffres du tableau de bord. Dépassées et à venir comptent les permis comme la page
/// Alertes ; les permis expirés sont comptés à part, en plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chiffres {
    pub depassees: usize,
    pub a_venir: usize,
    pub en_maintenance: usize,
    pub permis_expires: usize,
    pub en_service: usize,
}

pub fn compter(vehicules: &[Vehicule], alertes: &[Alerte]) -> Chiffres {
    let depassees = alertes
        .iter()
        .filter(|a| a.niveau == Niveau::Depassee)
        .count();
    Chiffres {
        depassees,
        a_venir: alertes.len() - depassees,
        en_maintenance: vehicules.iter().filter(|v| v.est_en_maintenance()).count(),
        permis_expires: alertes
            .iter()
            .filter(|a| a.niveau == Niveau::Depassee && a.est_permis())
            .count(),
        en_service: vehicules.iter().filter(|v| v.est_en_service()).count(),
    }
}

/// Les véhicules en service que personne ne conduit aujourd'hui.
pub fn sans_conducteur(
    vehicules: &[Vehicule],
    affectations: &[Affectation],
    aujourd_hui: NaiveDate,
) -> Vec<Vehicule> {
    vehicules
        .iter()
        .filter(|v| v.est_en_service())
        .filter(|v| {
            !affectations
                .iter()
                .any(|a| a.vehicule_id == v.id && a.est_en_cours(aujourd_hui))
        })
        .cloned()
        .collect()
}

/// Ce que l'accueil d'un particulier affiche.
pub struct AccueilParticulier {
    /// Les véhicules au parc, une carte chacun. Les réformés n'y sont pas (décidé le
    /// 14/10/2026) : l'accueil dit seulement combien il y en a.
    pub resumes: Vec<ResumeVehicule>,
    pub nb_reformes: usize,
    /// Les alertes du jour, les plus pressantes en premier : le bandeau en tire sa phrase.
    pub alertes: Vec<Alerte>,
}

/// Ce que le tableau de bord d'une entreprise affiche.
pub struct AccueilEntreprise {
    pub chiffres: Chiffres,
    pub alertes: Vec<Alerte>,
    pub sans_conducteur: Vec<Vehicule>,
    pub interventions: Vec<Intervention>,
}

/// Combien d'interventions récentes le tableau de bord montre.
pub const INTERVENTIONS_RECENTES: i64 = 5;

pub async fn particulier(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon_jours: i64,
) -> Resultat<AccueilParticulier> {
    let (reformes, au_parc): (Vec<Vehicule>, Vec<Vehicule>) = vehicules::lister(pool)
        .await?
        .into_iter()
        .partition(Vehicule::est_reforme);
    let ouvertes = echeances::lister_ouvertes(pool).await?;
    let mut resumes = resumer(au_parc, &ouvertes, aujourd_hui, horizon_jours);
    for photo in documents::photos_des_fiches(pool).await? {
        if let Some(resume) = resumes
            .iter_mut()
            .find(|r| Some(r.vehicule.id) == photo.vehicule_id)
        {
            resume.photo = Some(photo.id);
        }
    }
    Ok(AccueilParticulier {
        resumes,
        nb_reformes: reformes.len(),
        alertes: alertes::calculer(pool, aujourd_hui, horizon_jours).await?,
    })
}

pub async fn entreprise(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon_jours: i64,
) -> Resultat<AccueilEntreprise> {
    let vehicules = vehicules::lister(pool).await?;
    let alertes = alertes::calculer(pool, aujourd_hui, horizon_jours).await?;
    let affectations = crate::donnees::affectations::lister(pool).await?;
    Ok(AccueilEntreprise {
        chiffres: compter(&vehicules, &alertes),
        sans_conducteur: sans_conducteur(&vehicules, &affectations, aujourd_hui),
        interventions: interventions::lister_recentes(pool, INTERVENTIONS_RECENTES).await?,
        alertes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, j).unwrap()
    }

    fn vehicule(id: i64, immatriculation: &str, statut: &str) -> Vehicule {
        Vehicule {
            id,
            immatriculation: immatriculation.into(),
            couleur: String::new(),
            marque: "Renault".into(),
            modele: "Clio".into(),
            date_mise_en_service: jour(2021, 3, 15),
            carburant: "essence".into(),
            kilometrage: 48_200,
            statut: statut.into(),
            vin: String::new(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }
    }

    fn echeance(vehicule_id: i64, date_limite: NaiveDate) -> Echeance {
        Echeance {
            id: vehicule_id * 10,
            vehicule_id,
            type_echeance: "controle_technique".into(),
            date_limite,
            statut: "ouverte".into(),
            cree_le: Utc::now(),
            immatriculation: String::new(),
            couleur: String::new(),
        }
    }

    #[test]
    fn chaque_vehicule_prend_sa_prochaine_echeance_et_le_plus_presse_vient_en_premier() {
        let aujourd_hui = jour(2026, 10, 14);
        let vehicules = vec![
            vehicule(1, "AA-111-AA", "en_service"),
            vehicule(2, "BB-222-BB", "en_service"),
            vehicule(3, "CC-333-CC", "en_service"),
            vehicule(4, "DD-444-DD", "reforme"),
        ];
        // Triées par date, comme la base les rend : la première de chaque véhicule est la bonne.
        let ouvertes = vec![
            echeance(2, jour(2026, 10, 9)),  // dépassée depuis 5 jours
            echeance(1, jour(2026, 10, 29)), // dans 15 jours
            echeance(1, jour(2027, 10, 29)), // plus tard : ignorée
            echeance(3, jour(2027, 2, 11)),  // dans 120 jours : hors horizon
        ];
        let resumes = resumer(vehicules, &ouvertes, aujourd_hui, 30);

        let plaques: Vec<&str> = resumes
            .iter()
            .map(|r| r.vehicule.immatriculation.as_str())
            .collect();
        assert_eq!(
            plaques,
            ["BB-222-BB", "AA-111-AA", "CC-333-CC", "DD-444-DD"]
        );

        assert_eq!(resumes[0].classe_borne(), "borne-depassee");
        assert_eq!(resumes[0].borne_valeur(), "5");
        assert_eq!(resumes[0].borne_unite(), "retard");
        assert_eq!(
            resumes[0].prochaine.as_ref().unwrap().delai_fr(),
            "depuis 5 jours"
        );

        assert_eq!(resumes[1].classe_borne(), "borne-a-venir");
        assert_eq!(resumes[1].borne_valeur(), "15");
        assert_eq!(resumes[1].borne_unite(), "jours");

        assert_eq!(resumes[2].classe_borne(), "borne-a-jour");
        assert_eq!(resumes[2].borne_valeur(), "120");

        // « C'est fait » ne se propose que pour ce qui presse.
        assert!(resumes[0].presse());
        assert!(resumes[1].presse());
        assert!(!resumes[2].presse());
        assert!(!resumes[3].presse());

        // Réformé et sans échéance : neutre, en dernier.
        assert_eq!(resumes[3].classe_borne(), "borne-neutre");
        assert_eq!(resumes[3].borne_valeur(), "—");
        assert_eq!(resumes[3].borne_unite(), "");
    }

    #[test]
    fn un_jour_se_dit_au_singulier() {
        let aujourd_hui = jour(2026, 10, 14);
        let ouvertes = vec![echeance(1, jour(2026, 10, 15))];
        let resumes = resumer(
            vec![vehicule(1, "AA-111-AA", "en_service")],
            &ouvertes,
            aujourd_hui,
            30,
        );
        assert_eq!(resumes[0].borne_valeur(), "1");
        assert_eq!(resumes[0].borne_unite(), "jour");
    }

    fn alerte(niveau: Niveau, permis: bool) -> Alerte {
        Alerte {
            niveau,
            objet: String::new(),
            motif: String::new(),
            date_limite: jour(2026, 10, 1),
            jours: 0,
            lien: String::new(),
            echeance_id: if permis { None } else { Some(1) },
        }
    }

    #[test]
    fn les_chiffres_du_tableau_de_bord() {
        let vehicules = vec![
            vehicule(1, "AA-111-AA", "en_service"),
            vehicule(2, "BB-222-BB", "en_service"),
            vehicule(3, "CC-333-CC", "en_maintenance"),
            vehicule(4, "DD-444-DD", "reforme"),
        ];
        let alertes = vec![
            alerte(Niveau::Depassee, false),
            alerte(Niveau::Depassee, true),
            alerte(Niveau::AVenir, false),
            alerte(Niveau::AVenir, true),
            alerte(Niveau::AVenir, true),
        ];
        assert_eq!(
            compter(&vehicules, &alertes),
            Chiffres {
                depassees: 2,
                a_venir: 3,
                en_maintenance: 1,
                permis_expires: 1,
                en_service: 2,
            }
        );
        assert_eq!(compter(&[], &[]), Chiffres::default());
    }

    fn affectation(vehicule_id: i64, debut: NaiveDate, fin: Option<NaiveDate>) -> Affectation {
        Affectation {
            id: vehicule_id,
            vehicule_id,
            conducteur_id: 1,
            date_debut: debut,
            date_fin: fin,
            cree_le: Utc::now(),
            immatriculation: String::new(),
            couleur: String::new(),
            conducteur_nom: String::new(),
            conducteur_prenom: String::new(),
            conducteur_service: String::new(),
        }
    }

    #[test]
    fn un_vehicule_sans_conducteur_est_en_service_et_sans_affectation_en_cours() {
        let aujourd_hui = jour(2026, 10, 14);
        let vehicules = vec![
            vehicule(1, "AA-111-AA", "en_service"),     // conduit
            vehicule(2, "BB-222-BB", "en_service"),     // affectation terminée : libre
            vehicule(3, "CC-333-CC", "en_service"),     // affectation à venir : libre aujourd'hui
            vehicule(4, "DD-444-DD", "en_maintenance"), // pas au parc
        ];
        let affectations = vec![
            affectation(1, jour(2026, 1, 1), None),
            affectation(2, jour(2026, 1, 1), Some(jour(2026, 6, 1))),
            affectation(3, jour(2026, 12, 1), None),
        ];
        let libres: Vec<String> = sans_conducteur(&vehicules, &affectations, aujourd_hui)
            .into_iter()
            .map(|v| v.immatriculation)
            .collect();
        assert_eq!(libres, ["BB-222-BB", "CC-333-CC"]);
    }
}
