//! Les relevés du compteur et son historique (#94, bloc 4 ; retour #4). Le compteur d'un véhicule
//! est le plus haut kilométrage de son historique : ses relevés, ses interventions et ses pleins.
//! Chaque écriture de l'une de ces sources le recalcule dans sa transaction
//! ([`vehicules::recalculer_kilometrage`]) : corriger ou supprimer la saisie fautive le fait
//! redescendre.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::donnees::releves::{self, LigneHistorique, Releve};
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
}
