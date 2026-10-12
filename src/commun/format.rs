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

/// `123456` centimes → `1 234,56 €`.
pub fn centimes_fr(centimes: i64) -> String {
    let signe = if centimes < 0 { "-" } else { "" };
    let absolu = centimes.unsigned_abs();
    let euros = absolu / 100;
    let reste = absolu % 100;
    format!("{signe}{},{reste:02} €", separer_milliers(euros))
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
        assert_eq!(centimes_fr(0), "0,00 €");
        assert_eq!(centimes_fr(5), "0,05 €");
        assert_eq!(centimes_fr(123456), "1\u{202F}234,56 €");
        assert_eq!(centimes_fr(-250), "-2,50 €");
    }

    #[test]
    fn tailles() {
        assert_eq!(taille_fr(12), "12 octets");
        assert_eq!(taille_fr(820_000), "820 Ko");
        assert_eq!(taille_fr(1_500_000), "1,5 Mo");
    }
}
