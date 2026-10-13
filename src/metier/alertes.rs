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
        match self.jours {
            j if j < -1 => format!("depuis {} jours", -j),
            -1 => "depuis hier".to_string(),
            0 => "aujourd'hui".to_string(),
            1 => "demain".to_string(),
            j => format!("dans {j} jours"),
        }
    }
}

/// Classe une date limite par rapport à aujourd'hui.
pub fn niveau(date_limite: NaiveDate, aujourd_hui: NaiveDate) -> Niveau {
    if date_limite < aujourd_hui {
        Niveau::Depassee
    } else {
        Niveau::AVenir
    }
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
    let jusqu_au = aujourd_hui + chrono::Duration::days(horizon_jours);
    let echeances = alertes::echeances_a_traiter(pool, jusqu_au).await?;
    let conducteurs = alertes::conducteurs_permis_expirant(pool, jusqu_au).await?;
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
