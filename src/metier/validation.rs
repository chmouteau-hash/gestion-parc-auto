//! Fonctions de validation des champs de formulaire.
//!
//! Chaque fonction reçoit la chaîne brute et le nom du champ (pour le message), et renvoie la
//! valeur convertie ou une erreur métier en français. Les formulaires HTML et l'API JSON passent
//! par les mêmes fonctions : il n'y a qu'une seule définition de « valide ».

use chrono::NaiveDate;

use crate::erreurs::{ErreurApp, Resultat};

/// Longueur maximale d'un champ texte court (nom, marque, prestataire…).
pub const LONGUEUR_MAX_TEXTE: usize = 200;
/// Longueur maximale d'un champ texte long (notes).
pub const LONGUEUR_MAX_NOTES: usize = 5000;

/// Texte non vide, sans blancs autour, de longueur bornée.
pub fn texte_obligatoire(valeur: &str, champ: &str) -> Resultat<String> {
    let propre = valeur.trim();
    if propre.is_empty() {
        return Err(ErreurApp::metier(format!(
            "Le champ « {champ} » est obligatoire."
        )));
    }
    borner(propre, champ, LONGUEUR_MAX_TEXTE)
}

/// Texte pouvant être vide, sans blancs autour, de longueur bornée.
pub fn texte_facultatif(valeur: &str, champ: &str) -> Resultat<String> {
    borner(valeur.trim(), champ, LONGUEUR_MAX_TEXTE)
}

/// Notes libres, longueur bornée plus large.
pub fn notes(valeur: &str, champ: &str) -> Resultat<String> {
    borner(valeur.trim(), champ, LONGUEUR_MAX_NOTES)
}

fn borner(valeur: &str, champ: &str, max: usize) -> Resultat<String> {
    if valeur.chars().count() > max {
        return Err(ErreurApp::metier(format!(
            "Le champ « {champ} » dépasse {max} caractères."
        )));
    }
    Ok(valeur.to_string())
}

/// Immatriculation : majuscules, sans blancs autour. Le format exact n'est pas imposé (plaques
/// anciennes, étrangères, provisoires).
pub fn immatriculation(valeur: &str) -> Resultat<String> {
    let propre = texte_obligatoire(valeur, "immatriculation")?;
    Ok(propre.to_uppercase())
}

/// Numéro d'identification du véhicule (VIN) : facultatif ; sinon 17 caractères, lettres et
/// chiffres, sans I, O ni Q (ils se confondent avec 1 et 0), mis en majuscules. Les espaces et
/// tirets qu'on tape en recopiant une carte grise sont retirés.
pub fn vin(valeur: &str) -> Resultat<String> {
    let propre: String = valeur
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if propre.is_empty() {
        return Ok(propre);
    }
    let admis = |c: char| c.is_ascii_digit() || (c.is_ascii_uppercase() && !"IOQ".contains(c));
    if propre.chars().count() != 17 || !propre.chars().all(admis) {
        return Err(ErreurApp::metier(
            "Le numéro d'identification (VIN) a 17 caractères, lettres et chiffres, \
             sans I, O ni Q : c'est la case E de la carte grise.",
        ));
    }
    Ok(propre)
}

/// Date au format `AAAA-MM-JJ`, celui des champs `<input type="date">` et de JSON.
pub fn date_obligatoire(valeur: &str, champ: &str) -> Resultat<NaiveDate> {
    let propre = valeur.trim();
    if propre.is_empty() {
        return Err(ErreurApp::metier(format!(
            "La date « {champ} » est obligatoire."
        )));
    }
    NaiveDate::parse_from_str(propre, "%Y-%m-%d").map_err(|_| {
        ErreurApp::metier(format!(
            "La date « {champ} » doit être au format AAAA-MM-JJ (reçu « {propre} »)."
        ))
    })
}

/// Date facultative : chaîne vide = absente.
pub fn date_facultative(valeur: &str, champ: &str) -> Resultat<Option<NaiveDate>> {
    if valeur.trim().is_empty() {
        Ok(None)
    } else {
        date_obligatoire(valeur, champ).map(Some)
    }
}

/// Entier positif ou nul (kilométrage).
pub fn entier_positif(valeur: &str, champ: &str) -> Resultat<i32> {
    let propre = valeur.trim().replace([' ', '\u{202F}'], "");
    if propre.is_empty() {
        return Err(ErreurApp::metier(format!(
            "Le champ « {champ} » est obligatoire."
        )));
    }
    match propre.parse::<i32>() {
        Ok(n) if n >= 0 => Ok(n),
        _ => Err(ErreurApp::metier(format!(
            "Le champ « {champ} » doit être un nombre entier positif (reçu « {propre} »)."
        ))),
    }
}

/// Identifiant choisi dans une liste déroulante.
pub fn identifiant(valeur: &str, champ: &str) -> Resultat<i64> {
    valeur
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            ErreurApp::metier(format!(
                "Le champ « {champ} » doit être choisi dans la liste."
            ))
        })
}

/// Montant saisi en euros (`123`, `123,45`, `123.45`, `1 234,5`) converti en centimes entiers.
/// Vide = 0. Jamais de virgule flottante : on découpe la chaîne.
pub fn montant_en_centimes(valeur: &str, champ: &str) -> Resultat<i64> {
    let propre = valeur
        .trim()
        .replace([' ', '\u{202F}', '€'], "")
        .replace(',', ".");
    if propre.is_empty() {
        return Ok(0);
    }
    let erreur = || {
        ErreurApp::metier(format!(
            "Le champ « {champ} » doit être un montant en euros, par exemple 149,90 (reçu « {valeur} »)."
        ))
    };
    let (euros, centimes) = match propre.split_once('.') {
        Some((e, c)) => (e, c),
        None => (propre.as_str(), ""),
    };
    if centimes.len() > 2 || !centimes.chars().all(|c| c.is_ascii_digit()) {
        return Err(erreur());
    }
    let euros: i64 = if euros.is_empty() {
        0
    } else {
        euros.parse().map_err(|_| erreur())?
    };
    if euros < 0 {
        return Err(erreur());
    }
    let centimes: i64 = match centimes.len() {
        0 => 0,
        1 => centimes.parse::<i64>().map_err(|_| erreur())? * 10,
        _ => centimes.parse().map_err(|_| erreur())?,
    };
    Ok(euros * 100 + centimes)
}

/// Centimes → texte saisissable dans un formulaire (`149.90`), sans symbole.
pub fn centimes_en_saisie(centimes: i64) -> String {
    format!("{}.{:02}", centimes / 100, centimes % 100)
}

/// Code appartenant à un vocabulaire fermé (statut, type…).
pub fn code_parmi<T: Copy>(
    valeur: &str,
    champ: &str,
    depuis_code: fn(&str) -> Option<T>,
) -> Resultat<T> {
    depuis_code(valeur.trim()).ok_or_else(|| {
        ErreurApp::metier(format!(
            "La valeur « {} » n'est pas acceptée pour « {champ} ».",
            valeur.trim()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_obligatoire_refuse_le_vide() {
        assert!(texte_obligatoire("   ", "marque").is_err());
        assert_eq!(
            texte_obligatoire("  Renault ", "marque").unwrap(),
            "Renault"
        );
    }

    #[test]
    fn immatriculation_en_majuscules() {
        assert_eq!(immatriculation(" ab-123-cd ").unwrap(), "AB-123-CD");
        assert!(immatriculation("").is_err());
    }

    #[test]
    fn dates() {
        assert_eq!(
            date_obligatoire("2026-02-28", "début").unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert!(date_obligatoire("28/02/2026", "début").is_err());
        assert!(date_obligatoire("2026-02-30", "début").is_err());
        assert_eq!(date_facultative("", "fin").unwrap(), None);
    }

    #[test]
    fn entiers() {
        assert_eq!(entier_positif("12 500", "kilométrage").unwrap(), 12500);
        assert!(entier_positif("-1", "kilométrage").is_err());
        assert!(entier_positif("abc", "kilométrage").is_err());
        assert!(entier_positif("", "kilométrage").is_err());
    }

    #[test]
    fn montants() {
        assert_eq!(montant_en_centimes("", "coût").unwrap(), 0);
        assert_eq!(montant_en_centimes("149", "coût").unwrap(), 14900);
        assert_eq!(montant_en_centimes("149,9", "coût").unwrap(), 14990);
        assert_eq!(montant_en_centimes("149.90", "coût").unwrap(), 14990);
        assert_eq!(montant_en_centimes("1 234,56 €", "coût").unwrap(), 123456);
        assert_eq!(montant_en_centimes(",5", "coût").unwrap(), 50);
        assert!(montant_en_centimes("12,345", "coût").is_err());
        assert!(montant_en_centimes("-3", "coût").is_err());
        assert!(montant_en_centimes("abc", "coût").is_err());
        assert_eq!(centimes_en_saisie(14990), "149.90");
        assert_eq!(centimes_en_saisie(5), "0.05");
    }

    #[test]
    fn codes() {
        use crate::commun::types::StatutVehicule;
        assert_eq!(
            code_parmi("reforme", "statut", StatutVehicule::depuis_code).unwrap(),
            StatutVehicule::Reforme
        );
        assert!(code_parmi("perdu", "statut", StatutVehicule::depuis_code).is_err());
    }
}
