//! Écriture CSV « à la française » : point-virgule entre les champs, guillemets doublés, fins de
//! ligne CRLF, et la marque d'ordre des octets UTF-8 en tête pour qu'un tableur ouvre le fichier
//! avec ses accents sans rien demander.

/// Un fichier entier : la ligne d'en-têtes, puis une ligne par enregistrement.
pub fn document(entetes: &[&str], lignes: &[Vec<String>]) -> String {
    let mut sortie = String::from("\u{FEFF}");
    sortie.push_str(&ligne(entetes.iter().map(|e| e.to_string())));
    for enregistrement in lignes {
        sortie.push_str(&ligne(enregistrement.iter().cloned()));
    }
    sortie
}

fn ligne(champs: impl Iterator<Item = String>) -> String {
    let mut sortie = champs.map(|c| champ(&c)).collect::<Vec<_>>().join(";");
    sortie.push_str("\r\n");
    sortie
}

/// Un champ est mis entre guillemets s'il contient le séparateur, un guillemet ou un retour à
/// la ligne ; ses guillemets sont alors doublés.
fn champ(valeur: &str) -> String {
    if valeur.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", valeur.replace('"', "\"\""))
    } else {
        valeur.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_document_commence_par_la_marque_et_separe_par_des_points_virgules() {
        let csv = document(
            &["Nom", "Note"],
            &[
                vec!["Clio".to_string(), "simple".to_string()],
                vec![
                    "Kangoo".to_string(),
                    "avec ; et \"guillemets\"\nsur deux lignes".to_string(),
                ],
            ],
        );
        assert_eq!(
            csv,
            "\u{FEFF}Nom;Note\r\nClio;simple\r\nKangoo;\"avec ; et \"\"guillemets\"\"\nsur deux lignes\"\r\n"
        );
    }
}
