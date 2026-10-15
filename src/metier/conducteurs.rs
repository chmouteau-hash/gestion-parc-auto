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

/// Un nom ou un prénom sous la forme qui sert à le comparer : sans blancs autour ni doublés, en
/// minuscules, sans accents, le trait d'union lu comme un espace et l'apostrophe typographique
/// comme l'apostrophe droite. « Léa », « LEA » et « lea » sont le même prénom, « Jean-Pierre » et
/// « Jean Pierre » aussi.
///
/// Les accents sont repliés ici plutôt que par PostgreSQL : l'extension `unaccent` demande un
/// compte d'administration, que l'application n'a pas.
pub fn forme_comparable(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    // Le trait d'union sépare comme un espace : un prénom composé se tape des deux façons. Les
    // variantes U+2010 et U+2011 viennent des copier-coller (Word, tableurs).
    let separe = |c: char| c.is_whitespace() || matches!(c, '-' | '\u{2010}' | '\u{2011}');
    for mot in texte.split(separe).filter(|mot| !mot.is_empty()) {
        if !sortie.is_empty() {
            sortie.push(' ');
        }
        for c in mot.chars().flat_map(char::to_lowercase) {
            match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => sortie.push('a'),
                'ç' => sortie.push('c'),
                'è' | 'é' | 'ê' | 'ë' => sortie.push('e'),
                'ì' | 'í' | 'î' | 'ï' => sortie.push('i'),
                'ñ' => sortie.push('n'),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' => sortie.push('o'),
                'ù' | 'ú' | 'û' | 'ü' => sortie.push('u'),
                'ý' | 'ÿ' => sortie.push('y'),
                'œ' => sortie.push_str("oe"),
                'æ' => sortie.push_str("ae"),
                'ß' => sortie.push_str("ss"),
                // « D’Angelo » copié d'un traitement de texte est « D'Angelo ».
                '\u{2018}' | '\u{2019}' | '\u{02bc}' => sortie.push('\''),
                // L'accent combinant d'un texte décomposé (certains claviers, macOS) disparaît.
                '\u{0300}'..='\u{036f}' => {}
                autre => sortie.push(autre),
            }
        }
    }
    sortie
}

/// Parmi `existants`, ceux qui portent le même nom et le même prénom que la saisie, selon
/// [`forme_comparable`]. Actifs ou non : un conducteur inactif qui revient doit être réactivé, pas
/// recréé. Le service n'entre pas dans la comparaison : deux personnes de même nom dans des
/// services différents sont des homonymes à signaler, et l'avertissement affiche le service.
///
/// Sans nom ou sans prénom, il n'y a rien à comparer : la validation de la création dira ce qui
/// manque.
pub fn homonymes_parmi(existants: Vec<Conducteur>, saisie: &SaisieConducteur) -> Vec<Conducteur> {
    let nom = forme_comparable(&saisie.nom);
    let prenom = forme_comparable(&saisie.prenom);
    if nom.is_empty() || prenom.is_empty() {
        return Vec::new();
    }
    existants
        .into_iter()
        .filter(|c| forme_comparable(&c.nom) == nom && forme_comparable(&c.prenom) == prenom)
        .collect()
}

/// Les conducteurs déjà enregistrés sous le même nom et le même prénom que la saisie.
///
/// Ne sert qu'au formulaire de création, pour avertir avant d'enregistrer : [`creer`] ne l'appelle
/// pas, donc l'API JSON crée sans avertir. Un homonyme n'est jamais refusé, deux personnes
/// pouvant réellement porter le même nom.
pub async fn homonymes(pool: &PgPool, saisie: &SaisieConducteur) -> Resultat<Vec<Conducteur>> {
    Ok(homonymes_parmi(conducteurs::lister(pool).await?, saisie))
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

    fn existant(id: i64, nom: &str, prenom: &str, statut: &str) -> Conducteur {
        Conducteur {
            id,
            nom: nom.into(),
            prenom: prenom.into(),
            service: "Logistique".into(),
            date_expiration_permis: NaiveDate::from_ymd_opt(2031, 1, 1).unwrap(),
            statut: statut.into(),
            cree_le: Utc::now(),
            modifie_le: Utc::now(),
        }
    }

    fn saisie(nom: &str, prenom: &str) -> SaisieConducteur {
        SaisieConducteur {
            nom: nom.into(),
            prenom: prenom.into(),
            ..Default::default()
        }
    }

    #[test]
    fn la_forme_comparable_ignore_casse_accents_blancs_et_traits_d_union() {
        assert_eq!(forme_comparable("  Léa  "), "lea");
        assert_eq!(forme_comparable("LÉA"), "lea");
        assert_eq!(forme_comparable("Jean   Pierre"), "jean pierre");
        assert_eq!(forme_comparable("Çelik"), "celik");
        assert_eq!(forme_comparable("Œuvrard"), "oeuvrard");
        assert_eq!(forme_comparable("Müller"), "muller");
        // Un « é » décomposé (e + accent combinant), comme en sortent certains claviers.
        assert_eq!(forme_comparable("Le\u{0301}a"), "lea");
        // Le trait d'union vaut un espace, y compris ses variantes de copier-coller (U+2010, U+2011).
        assert_eq!(forme_comparable("Jean-Pierre"), "jean pierre");
        assert_eq!(forme_comparable("Jean Pierre"), "jean pierre");
        assert_eq!(forme_comparable("Jean\u{2010}Pierre"), "jean pierre");
        assert_eq!(forme_comparable("Jean\u{2011}Pierre"), "jean pierre");
        assert_eq!(forme_comparable(" Jean - Pierre "), "jean pierre");
        // L'apostrophe typographique vaut l'apostrophe droite ; l'apostrophe, elle, reste.
        assert_eq!(forme_comparable("D’Angelo"), "d'angelo");
        assert_eq!(forme_comparable("D'Angelo"), "d'angelo");
        assert_ne!(forme_comparable("D'Angelo"), forme_comparable("Dangelo"));
        assert_eq!(forme_comparable("   "), "");
        assert_eq!(forme_comparable(" - "), "");
    }

    #[test]
    fn les_homonymes_se_trouvent_sans_tenir_compte_de_la_casse_ni_des_accents() {
        let existants = vec![
            existant(1, "Martin", "Léa", "actif"),
            existant(2, "Martin", "Paul", "actif"),
            existant(3, "Durand", "Léa", "inactif"),
            existant(4, "MARTIN", "LEA", "inactif"),
        ];
        let ids: Vec<i64> = homonymes_parmi(existants.clone(), &saisie(" martin ", "lea"))
            .iter()
            .map(|c| c.id)
            .collect();
        // Les inactifs comptent ; le prénom ET le nom doivent concorder.
        assert_eq!(ids, vec![1, 4]);
        assert!(homonymes_parmi(existants.clone(), &saisie("Martin", "Jean")).is_empty());
        // Sans nom ou sans prénom : rien à comparer.
        assert!(homonymes_parmi(existants.clone(), &saisie("", "Léa")).is_empty());
        assert!(homonymes_parmi(existants, &saisie("Martin", "  ")).is_empty());
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
