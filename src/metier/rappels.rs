//! Le rappel quotidien (#176) : ce que la page Alertes dit, en texte brut, pour un courriel.
//!
//! Rien de plus que la page : objet, motif, borne, délai, et le lien vers la page concernée. Ni
//! le conducteur d'un véhicule, ni un document. Rien à dire, pas de rappel : le courriel qui
//! arrive a toujours une raison.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::commun::format::date_fr;
use crate::erreurs::Resultat;
use crate::metier::alertes::{self, Alerte, Horizon, Niveau};
use crate::metier::releves::{self, ARelever};

/// Un rappel prêt à partir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rappel {
    pub sujet: String,
    pub corps: String,
    /// Nombre de points rappelés, pour le journal.
    pub nombre: usize,
}

/// Lit le jour et compose. `None` quand il n'y a rien à dire.
pub async fn du_jour(
    pool: &PgPool,
    aujourd_hui: NaiveDate,
    horizon: Horizon,
    marque: &str,
    adresse_application: &str,
) -> Resultat<Option<Rappel>> {
    let alertes = alertes::calculer(pool, aujourd_hui, horizon).await?;
    let a_relever = releves::a_relever(pool, aujourd_hui).await?;
    Ok(composer(
        &alertes,
        &a_relever,
        marque,
        adresse_application,
        aujourd_hui,
    ))
}

/// Compose le rappel : les dépassées d'abord, puis les à venir, puis les compteurs à relever.
/// `adresse_application` préfixe les liens (`https://parc.exemple.fr`) ; vide, les liens restent
/// relatifs.
pub fn composer(
    alertes: &[Alerte],
    a_relever: &[ARelever],
    marque: &str,
    adresse_application: &str,
    aujourd_hui: NaiveDate,
) -> Option<Rappel> {
    if alertes.is_empty() && a_relever.is_empty() {
        return None;
    }
    let adresse = adresse_application.trim_end_matches('/');
    let depassees: Vec<&Alerte> = alertes
        .iter()
        .filter(|a| a.niveau == Niveau::Depassee)
        .collect();
    let a_venir: Vec<&Alerte> = alertes
        .iter()
        .filter(|a| a.niveau == Niveau::AVenir)
        .collect();

    let mut parties = Vec::new();
    if !depassees.is_empty() {
        parties.push(pluriel(
            depassees.len(),
            "alerte dépassée",
            "alertes dépassées",
        ));
    }
    if !a_venir.is_empty() {
        parties.push(format!("{} à venir", a_venir.len()));
    }
    if !a_relever.is_empty() {
        parties.push(pluriel(
            a_relever.len(),
            "compteur à relever",
            "compteurs à relever",
        ));
    }
    let sujet = format!("{marque} : {}", parties.join(", "));

    let mut corps = format!("Rappel du {}.\n", date_fr(aujourd_hui));
    section(&mut corps, "DÉPASSÉES", &depassees, adresse);
    section(&mut corps, "À VENIR", &a_venir, adresse);
    if !a_relever.is_empty() {
        corps.push_str(&format!("\nCOMPTEURS À RELEVER ({})\n", a_relever.len()));
        for r in a_relever {
            corps.push_str(&format!(
                "- Véhicule {} : dernier relevé {}\n  {adresse}/vehicules/{}\n",
                r.releve.immatriculation,
                r.depuis_fr(),
                r.releve.vehicule_id
            ));
        }
    }
    corps.push_str(&format!(
        "\nTout est dans l'application : {adresse}/alertes\n"
    ));

    Some(Rappel {
        sujet,
        corps,
        nombre: alertes.len() + a_relever.len(),
    })
}

fn section(corps: &mut String, titre: &str, alertes: &[&Alerte], adresse: &str) {
    if alertes.is_empty() {
        return;
    }
    corps.push_str(&format!("\n{titre} ({})\n", alertes.len()));
    for a in alertes {
        let constat = if a.constat.is_empty() {
            String::new()
        } else {
            format!(" ({})", a.constat)
        };
        corps.push_str(&format!(
            "- {} · {}{constat} : {}, {}\n  {adresse}{}\n",
            a.objet,
            a.motif,
            a.limite_fr(),
            a.delai_fr(),
            a.lien
        ));
    }
}

fn pluriel(n: usize, un: &str, plusieurs: &str) -> String {
    format!("{n} {}", if n > 1 { plusieurs } else { un })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::releves::DernierReleve;

    fn alerte(niveau: Niveau, objet: &str, motif: &str, jours: i64, lien: &str) -> Alerte {
        Alerte {
            niveau,
            objet: objet.to_string(),
            motif: motif.to_string(),
            constat: String::new(),
            date_limite: Some(NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()),
            jours: Some(jours),
            kilometrage_limite: None,
            kilometres: None,
            lien: lien.to_string(),
            echeance_id: Some(1),
            plan_id: None,
            c_est_fait: None,
            avancement: 0,
            estimation: None,
        }
    }

    fn jour() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 24).unwrap()
    }

    #[test]
    fn rien_a_dire_pas_de_rappel() {
        assert_eq!(composer(&[], &[], "Autoclair", "https://x", jour()), None);
    }

    #[test]
    fn une_depassee_seule() {
        let alertes = [alerte(
            Niveau::Depassee,
            "Véhicule AB-123-CD",
            "Contrôle technique",
            -4,
            "/vehicules/7",
        )];
        let rappel = composer(
            &alertes,
            &[],
            "Autoclair Pro",
            "https://parc.exemple.fr/",
            jour(),
        )
        .unwrap();
        assert_eq!(rappel.sujet, "Autoclair Pro : 1 alerte dépassée");
        assert_eq!(rappel.nombre, 1);
        assert!(
            rappel.corps.starts_with("Rappel du 24/10/2026.\n"),
            "{}",
            rappel.corps
        );
        assert!(
            rappel.corps.contains("\nDÉPASSÉES (1)\n"),
            "{}",
            rappel.corps
        );
        assert!(
            rappel.corps.contains(
                "- Véhicule AB-123-CD · Contrôle technique : 20/10/2026, depuis 4 jours\n  https://parc.exemple.fr/vehicules/7\n"
            ),
            "{}",
            rappel.corps
        );
        assert!(!rappel.corps.contains("À VENIR"));
        assert!(rappel
            .corps
            .ends_with("Tout est dans l'application : https://parc.exemple.fr/alertes\n"));
    }

    #[test]
    fn melange_dans_l_ordre_avec_les_compteurs() {
        let alertes = [
            alerte(
                Niveau::AVenir,
                "Véhicule AB-123-CD",
                "Assurance",
                10,
                "/vehicules/7",
            ),
            alerte(
                Niveau::Depassee,
                "Conducteur Léa Martin",
                "Permis de conduire",
                -1,
                "/conducteurs/2",
            ),
            alerte(
                Niveau::Depassee,
                "Véhicule EF-456-GH",
                "Vidange",
                -2,
                "/vehicules/8",
            ),
        ];
        let compteurs = [ARelever {
            releve: DernierReleve {
                vehicule_id: 9,
                immatriculation: "IJ-789-KL".into(),
                couleur: String::new(),
                kilometrage: 50_000,
                dernier: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            },
            jours: 53,
        }];
        let rappel = composer(&alertes, &compteurs, "Autoclair", "", jour()).unwrap();
        assert_eq!(
            rappel.sujet,
            "Autoclair : 2 alertes dépassées, 1 à venir, 1 compteur à relever"
        );
        assert_eq!(rappel.nombre, 4);
        let corps = &rappel.corps;
        let (i_dep, i_venir, i_compteurs) = (
            corps.find("DÉPASSÉES (2)").unwrap(),
            corps.find("À VENIR (1)").unwrap(),
            corps.find("COMPTEURS À RELEVER (1)").unwrap(),
        );
        assert!(i_dep < i_venir && i_venir < i_compteurs, "{corps}");
        assert!(corps.contains("- Véhicule IJ-789-KL : dernier relevé le 01/09/2026 · il y a 53 jours\n  /vehicules/9\n"), "{corps}");
        assert!(
            corps.contains("  /conducteurs/2\n"),
            "les liens restent relatifs sans adresse"
        );
    }

    #[test]
    fn le_constat_d_une_chose_a_faire_est_repris() {
        let mut a = alerte(
            Niveau::AVenir,
            "Véhicule AB-123-CD",
            "À faire : Balais",
            3,
            "/vehicules/7",
        );
        a.constat = "balais usés".into();
        let rappel = composer(&[a], &[], "Autoclair", "", jour()).unwrap();
        assert!(
            rappel
                .corps
                .contains("À faire : Balais (balais usés) : 20/10/2026, dans 3 jours"),
            "{}",
            rappel.corps
        );
    }
}
