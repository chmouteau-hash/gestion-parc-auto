//! Mise en forme à la française des dates, montants et tailles, pour l'affichage.

use chrono::{DateTime, Local, NaiveDate, Utc};

/// `2026-10-11` → `11/10/2026`.
pub fn date_fr(date: NaiveDate) -> String {
    date.format("%d/%m/%Y").to_string()
}

/// Horodatage en heure locale de la machine, `11/10/2026 14:05`.
pub fn date_heure_fr(instant: DateTime<Utc>) -> String {
    instant
        .with_timezone(&Local)
        .format("%d/%m/%Y %H:%M")
        .to_string()
}

/// `123456` centimes → `1 234,56 €`. L'espace avant « € » est fine et insécable, comme celle
/// des milliers : le symbole ne passe jamais seul à la ligne (#103).
pub fn centimes_fr(centimes: i64) -> String {
    let signe = if centimes < 0 { "-" } else { "" };
    let absolu = centimes.unsigned_abs();
    let euros = absolu / 100;
    let reste = absolu % 100;
    format!("{signe}{},{reste:02}\u{202F}€", separer_milliers(euros))
}

/// Quantité en centièmes → texte sans zéro inutile : `540` → `5,4`, `200` → `2`, `25` → `0,25`.
/// Sert à l'affichage (« × 5,4 ») et au formulaire.
pub fn quantite_fr(centiemes: i32) -> String {
    let entier = centiemes / 100;
    let reste = centiemes % 100;
    if reste == 0 {
        entier.to_string()
    } else if reste % 10 == 0 {
        format!("{entier},{}", reste / 10)
    } else {
        format!("{entier},{reste:02}")
    }
}

/// Dixièmes → un chiffre après la virgule, zéro compris : `62` → `6,2`, `60` → `6,0`. Pour une
/// consommation aux 100 km (#126, bloc 3).
pub fn dixiemes_fr(dixiemes: i32) -> String {
    let signe = if dixiemes < 0 { "-" } else { "" };
    let absolu = dixiemes.unsigned_abs();
    format!("{signe}{},{}", absolu / 10, absolu % 10)
}

/// Des euros sans les centimes, arrondis : `127949` → `1 279 €`. Pour un chiffre de tableau de
/// bord, où les centimes ne disent rien et allongeraient le chiffre.
pub fn euros_ronds_fr(centimes: i64) -> String {
    let signe = if centimes < 0 { "-" } else { "" };
    let euros = (centimes.unsigned_abs() + 50) / 100;
    format!("{signe}{}\u{202F}€", separer_milliers(euros))
}

/// Un coût aux 100 km en centimes → des euros au kilomètre, deux décimales : `1240` centimes aux
/// 100 km → `0,12 €/km`.
pub fn euros_au_km_fr(centimes_aux_100_km: i64) -> String {
    let signe = if centimes_aux_100_km < 0 { "-" } else { "" };
    let absolu = centimes_aux_100_km.unsigned_abs();
    // Centimes aux 100 km → centièmes d'euro au km : 10 000 centimes aux 100 km = 1 € au km.
    let centiemes = (absolu + 50) / 100;
    format!(
        "{signe}{},{:02}\u{202F}€/km",
        centiemes / 100,
        centiemes % 100
    )
}

/// Un entier lisible : `160000` → `160 000` (espace fine insécable). Pour un kilométrage, un
/// compte.
pub fn nombre_fr(n: i64) -> String {
    let signe = if n < 0 { "-" } else { "" };
    format!("{signe}{}", separer_milliers(n.unsigned_abs()))
}

/// Sépare les milliers par une espace insécable : `1234567` → `1 234 567`.
fn separer_milliers(n: u64) -> String {
    let chiffres = n.to_string();
    let mut resultat = String::with_capacity(chiffres.len() + chiffres.len() / 3);
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (chiffres.len() - i).is_multiple_of(3) {
            resultat.push('\u{202F}');
        }
        resultat.push(c);
    }
    resultat
}

/// Taille lisible : `1,5 Mo`, `820 Ko`, `12 octets`.
pub fn taille_fr(octets: i64) -> String {
    let octets = octets.max(0) as f64;
    if octets >= 1_000_000.0 {
        format!("{:.1} Mo", octets / 1_000_000.0).replace('.', ",")
    } else if octets >= 1_000.0 {
        format!("{:.0} Ko", octets / 1_000.0)
    } else {
        format!("{octets:.0} octets")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date() {
        assert_eq!(
            date_fr(NaiveDate::from_ymd_opt(2026, 1, 9).unwrap()),
            "09/01/2026"
        );
    }

    #[test]
    fn montants() {
        assert_eq!(centimes_fr(0), "0,00\u{202F}€");
        assert_eq!(centimes_fr(5), "0,05\u{202F}€");
        assert_eq!(centimes_fr(123456), "1\u{202F}234,56\u{202F}€");
        assert_eq!(centimes_fr(-250), "-2,50\u{202F}€");
    }

    #[test]
    fn consommations_et_couts() {
        assert_eq!(dixiemes_fr(62), "6,2");
        assert_eq!(dixiemes_fr(60), "6,0");
        assert_eq!(dixiemes_fr(5), "0,5");
        assert_eq!(dixiemes_fr(1234), "123,4");
        assert_eq!(euros_ronds_fr(127_949), "1\u{202F}279\u{202F}€");
        assert_eq!(euros_ronds_fr(49), "0\u{202F}€");
        assert_eq!(euros_ronds_fr(50), "1\u{202F}€");
        assert_eq!(euros_au_km_fr(1240), "0,12\u{202F}€/km");
        assert_eq!(euros_au_km_fr(1250), "0,13\u{202F}€/km");
        assert_eq!(euros_au_km_fr(30), "0,00\u{202F}€/km");
        assert_eq!(euros_au_km_fr(25_000), "2,50\u{202F}€/km");
    }

    #[test]
    fn nombres() {
        assert_eq!(nombre_fr(0), "0");
        assert_eq!(nombre_fr(999), "999");
        assert_eq!(nombre_fr(160_000), "160\u{202F}000");
        assert_eq!(nombre_fr(-1_234), "-1\u{202F}234");
    }

    #[test]
    fn tailles() {
        assert_eq!(taille_fr(12), "12 octets");
        assert_eq!(taille_fr(820_000), "820 Ko");
        assert_eq!(taille_fr(1_500_000), "1,5 Mo");
    }
}
