//! Vue alertes : échéances dépassées ou proches, permis de conduire compris.
//!
//! Le classement est une fonction pure ([`niveau`]) testée sans base ; la lecture des données
//! est déléguée à `donnees::alertes`.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgPool;

use crate::commun::format::date_fr;
use crate::donnees::alertes;
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::echeances::Echeance;
use crate::erreurs::Resultat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Niveau {
    /// La date limite est passée.
    Depassee,
    /// La date limite tombe dans l'horizon (aujourd'hui compris).
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

/// Une ligne de la vue alertes, quelle que soit son origine (échéance ou permis).
#[derive(Debug, Clone, Serialize)]
pub struct Alerte {
    pub niveau: Niveau,
    /// « Véhicule AB-123-CD » ou « Conducteur Léa Martin ».
    pub objet: String,
    /// « Contrôle technique », « Permis de conduire »…
    pub motif: String,
    pub date_limite: NaiveDate,
    /// Jours restants (négatif si dépassée).
    pub jours: i64,
    /// Page à ouvrir pour traiter l'alerte.
    pub lien: String,
    /// Échéance que « Marquer traitée » ferme ; absent pour un permis. Hors du JSON de l'API,
    /// qui ne change pas.
    #[serde(skip)]
    pub echeance_id: Option<i64>,
}

impl Alerte {
    pub fn date_limite_fr(&self) -> String {
        date_fr(self.date_limite)
    }

    pub fn delai_fr(&self) -> String {
        delai_fr(self.jours)
    }

    /// Part du trajet déjà parcourue vers l'échéance, de 0 à 100, sur l'horizon des alertes :
    /// 0 tant que l'échéance est hors de l'horizon, 100 dès qu'elle est dépassée. C'est ce que la
    /// barre de trajet remplit.
    pub fn avancement(&self, horizon_jours: i64) -> u8 {
        avancement(self.jours, horizon_jours)
    }

    /// Le même avancement arrondi à la dizaine (0, 10, … 100) : la barre de trajet a une classe
    /// par dizaine, puisque la consigne de sécurité interdit un style écrit dans la page.
    pub fn avancement_dizaine(&self, horizon_jours: i64) -> u8 {
        (self.avancement(horizon_jours) + 5) / 10 * 10
    }

    /// Vrai pour un permis de conduire, faux pour une échéance d'un véhicule.
    pub fn est_permis(&self) -> bool {
        self.echeance_id.is_none()
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

/// Voir [`Alerte::avancement`].
pub fn avancement(jours: i64, horizon_jours: i64) -> u8 {
    if horizon_jours <= 0 || jours <= 0 {
        return 100;
    }
    if jours >= horizon_jours {
        return 0;
    }
    (100 - jours * 100 / horizon_jours) as u8
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

/// Construit la liste d'alertes à partir des données déjà filtrées par la base, triée par date
/// limite (les plus urgentes en premier). Fonction pure.
pub fn construire(
    echeances: &[Echeance],
    conducteurs: &[Conducteur],
    aujourd_hui: NaiveDate,
) -> Vec<Alerte> {
    let mut alertes: Vec<Alerte> = echeances
        .iter()
        .map(|e| Alerte {
            niveau: niveau(e.date_limite, aujourd_hui),
            objet: format!("Véhicule {}", e.immatriculation),
            motif: e.type_libelle().to_string(),
            date_limite: e.date_limite,
            jours: (e.date_limite - aujourd_hui).num_days(),
            lien: format!("/vehicules/{}", e.vehicule_id),
            echeance_id: Some(e.id),
        })
        .chain(conducteurs.iter().map(|c| Alerte {
            niveau: niveau(c.date_expiration_permis, aujourd_hui),
            objet: format!("Conducteur {}", c.nom_complet()),
            motif: "Permis de conduire".to_string(),
            date_limite: c.date_expiration_permis,
            jours: (c.date_expiration_permis - aujourd_hui).num_days(),
            lien: format!("/conducteurs/{}", c.id),
            echeance_id: None,
        }))
        .collect();
    alertes.sort_by(|a, b| {
        a.date_limite
            .cmp(&b.date_limite)
            .then(a.objet.cmp(&b.objet))
    });
    alertes
}

/// Lit la base et construit les alertes jusqu'à `aujourd_hui + horizon_jours`.
pub async fn calculer(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon_jours: i64,
) -> Resultat<Vec<Alerte>> {
    let limite = jusqu_au(aujourd_hui, horizon_jours);
    let echeances = alertes::echeances_a_traiter(pool, limite).await?;
    let conducteurs = alertes::conducteurs_permis_expirant(pool, limite).await?;
    Ok(construire(&echeances, &conducteurs, aujourd_hui))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
        // et au-delà, vide tant que l'échéance est hors de l'horizon.
        assert_eq!(avancement(30, 30), 0);
        assert_eq!(avancement(45, 30), 0);
        assert_eq!(avancement(15, 30), 50);
        assert_eq!(avancement(3, 30), 90);
        assert_eq!(avancement(0, 30), 100);
        assert_eq!(avancement(-5, 30), 100);
        assert_eq!(avancement(5, 0), 100);
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
        let alertes = construire(&echeances, &conducteurs, aujourd_hui);
        assert_eq!(alertes.len(), 3);

        assert_eq!(alertes[0].niveau, Niveau::Depassee);
        assert_eq!(alertes[0].motif, "Contrôle technique");
        assert_eq!(alertes[0].jours, -10);
        assert_eq!(alertes[0].delai_fr(), "depuis 10 jours");
        assert_eq!(alertes[0].lien, "/vehicules/12");

        assert_eq!(alertes[1].objet, "Conducteur Léa Martin");
        assert_eq!(alertes[1].motif, "Permis de conduire");
        assert_eq!(alertes[1].delai_fr(), "aujourd'hui");
        assert_eq!(alertes[1].lien, "/conducteurs/5");

        assert_eq!(alertes[2].niveau, Niveau::AVenir);
        assert_eq!(alertes[2].delai_fr(), "dans 9 jours");
    }
}
