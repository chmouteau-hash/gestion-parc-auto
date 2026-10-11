//! Conducteurs : validation, opérations et purge RGPD.

use std::path::Path;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::StatutConducteur;
use crate::donnees::conducteurs::{self, Conducteur, NouveauConducteur};
use crate::donnees::{affectations, documents};
use crate::erreurs::{ErreurApp, Resultat};
use crate::metier::validation::{
    code_parmi, date_obligatoire, texte_facultatif, texte_obligatoire,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieConducteur {
    #[serde(default)]
    pub nom: String,
    #[serde(default)]
    pub prenom: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub date_expiration_permis: String,
    #[serde(default)]
    pub statut: String,
}

impl SaisieConducteur {
    pub fn depuis(c: &Conducteur) -> SaisieConducteur {
        SaisieConducteur {
            nom: c.nom.clone(),
            prenom: c.prenom.clone(),
            service: c.service.clone(),
            date_expiration_permis: c.date_expiration_permis.to_string(),
            statut: c.statut.clone(),
        }
    }
}

pub fn valider(s: &SaisieConducteur) -> Resultat<NouveauConducteur> {
    Ok(NouveauConducteur {
        nom: texte_obligatoire(&s.nom, "nom")?,
        prenom: texte_obligatoire(&s.prenom, "prénom")?,
        service: texte_facultatif(&s.service, "service")?,
        date_expiration_permis: date_obligatoire(
            &s.date_expiration_permis,
            "expiration du permis",
        )?,
        statut: code_parmi(&s.statut, "statut", StatutConducteur::depuis_code)?,
    })
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Conducteur>> {
    Ok(conducteurs::lister(pool).await?)
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Conducteur> {
    conducteurs::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn creer(pool: &PgPool, saisie: &SaisieConducteur) -> Resultat<Conducteur> {
    let nouveau = valider(saisie)?;
    Ok(conducteurs::inserer(pool, &nouveau).await?)
}

/// Modifie le conducteur.
///
/// Un conducteur inactif ne conduit plus : l'enregistrer avec ce statut met fin, à la date du
/// jour, à ses affectations encore ouvertes. Les deux écritures sont dans une même transaction :
/// soit les deux réussissent, soit aucune.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieConducteur,
    aujourd_hui: NaiveDate,
) -> Resultat<Conducteur> {
    let nouveau = valider(saisie)?;
    let mut transaction = pool.begin().await?;
    let conducteur = conducteurs::mettre_a_jour(&mut *transaction, id, &nouveau)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    if !conducteur.est_actif() {
        affectations::cloturer_pour_conducteur(&mut *transaction, id, aujourd_hui).await?;
    }
    transaction.commit().await?;
    Ok(conducteur)
}

pub async fn supprimer(pool: &PgPool, documents_dir: &Path, id: i64) -> Resultat<()> {
    let docs = documents::lister_par_conducteur(pool, id).await?;
    if !conducteurs::supprimer(pool, id).await? {
        return Err(ErreurApp::Introuvable);
    }
    crate::metier::documents::supprimer_fichiers(documents_dir, &docs).await;
    Ok(())
}

/// RGPD : supprime les conducteurs inactifs dont la fiche n'a pas bougé depuis plus de
/// `conservation_jours` jours, avec leur historique et leurs documents. Renvoie le nombre de
/// fiches supprimées.
///
/// « N'a pas bougé » se lit sur `modifie_le`, mis à jour à chaque enregistrement de la fiche,
/// donc au plus tard au passage en statut inactif.
pub async fn purger_inactifs(
    pool: &PgPool,
    documents_dir: &Path,
    maintenant: DateTime<Utc>,
    conservation_jours: i64,
) -> Resultat<u64> {
    let limite = maintenant - Duration::days(conservation_jours);
    let candidats = conducteurs::lister_inactifs_avant(pool, limite).await?;
    let mut supprimes = 0;
    for conducteur in candidats {
        let docs = documents::lister_par_conducteur(pool, conducteur.id).await?;
        if conducteurs::supprimer(pool, conducteur.id).await? {
            crate::metier::documents::supprimer_fichiers(documents_dir, &docs).await;
            supprimes += 1;
            tracing::info!(id = conducteur.id, "conducteur inactif purgé (RGPD)");
        }
    }
    Ok(supprimes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_facultatif() {
        let s = SaisieConducteur {
            nom: "Martin".into(),
            prenom: "Léa".into(),
            service: "".into(),
            date_expiration_permis: "2030-01-01".into(),
            statut: "actif".into(),
        };
        let c = valider(&s).unwrap();
        assert_eq!(c.service, "");
        assert_eq!(c.statut, StatutConducteur::Actif);
    }

    #[test]
    fn nom_obligatoire() {
        let s = SaisieConducteur {
            prenom: "Léa".into(),
            date_expiration_permis: "2030-01-01".into(),
            statut: "actif".into(),
            ..Default::default()
        };
        assert!(valider(&s).is_err());
    }
}
