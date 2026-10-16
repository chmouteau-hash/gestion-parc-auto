//! Vocabulaires fermés : statuts et types.
//!
//! Chaque valeur a un **code** (stocké en base, stable, sans accent) et un **libellé** (affiché).
//! La liste des codes est répétée dans la contrainte `CHECK` de la migration : une valeur qui
//! s'ajoute s'ajoute aux deux endroits, et le test `codes_acceptes_par_la_base` de
//! `tests/vehicules.rs` vérifie que les deux listes sont d'accord.

/// Une option de liste déroulante : code envoyé par le formulaire, libellé affiché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choix {
    pub code: &'static str,
    pub libelle: &'static str,
}

/// Déclare une énumération avec ses codes et ses libellés, et les fonctions de conversion.
macro_rules! vocabulaire {
    (
        $(#[$doc:meta])*
        $nom:ident { $($variante:ident => $code:literal, $libelle:literal;)+ }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $nom {
            $($variante),+
        }

        impl $nom {
            /// Toutes les valeurs, dans l'ordre d'affichage.
            pub const TOUS: &'static [$nom] = &[$($nom::$variante),+];

            pub fn code(self) -> &'static str {
                match self {
                    $($nom::$variante => $code),+
                }
            }

            pub fn libelle(self) -> &'static str {
                match self {
                    $($nom::$variante => $libelle),+
                }
            }

            pub fn depuis_code(code: &str) -> Option<$nom> {
                match code {
                    $($code => Some($nom::$variante),)+
                    _ => None,
                }
            }

            /// Options prêtes pour une liste déroulante.
            pub fn choix() -> Vec<Choix> {
                Self::TOUS
                    .iter()
                    .map(|v| Choix { code: v.code(), libelle: v.libelle() })
                    .collect()
            }
        }
    };
}

vocabulaire! {
    /// Rôle d'un compte de connexion.
    Role {
        Administrateur => "administrateur", "Administrateur";
        Lecteur => "lecteur", "Lecture seule";
    }
}

vocabulaire! {
    /// Profil d'une installation : à qui elle s'adresse, donc ce que ses pages montrent. Il ne
    /// se stocke pas en base : il vient de la variable `PROFIL` (voir `src/config.rs`).
    Profil {
        Entreprise => "entreprise", "Entreprise";
        Particulier => "particulier", "Particulier";
    }
}

vocabulaire! {
    StatutVehicule {
        EnService => "en_service", "En service";
        EnMaintenance => "en_maintenance", "En maintenance";
        Reforme => "reforme", "Réformé";
    }
}

vocabulaire! {
    Carburant {
        Essence => "essence", "Essence";
        Diesel => "diesel", "Diesel";
        Electrique => "electrique", "Électrique";
        Hybride => "hybride", "Hybride";
        Gpl => "gpl", "GPL";
        Autre => "autre", "Autre";
    }
}

vocabulaire! {
    StatutConducteur {
        Actif => "actif", "Actif";
        Inactif => "inactif", "Inactif";
    }
}

vocabulaire! {
    TypeIntervention {
        Revision => "revision", "Révision";
        Reparation => "reparation", "Réparation";
        Pneumatiques => "pneumatiques", "Pneumatiques";
        ControleTechnique => "controle_technique", "Contrôle technique";
        ContreVisite => "contre_visite", "Contre-visite";
        Carrosserie => "carrosserie", "Carrosserie";
        Autre => "autre", "Autre";
    }
}

vocabulaire! {
    /// Le résultat d'un contrôle technique ou d'une contre-visite (#107). Une défaillance majeure
    /// impose une contre-visite dans les deux mois ; une défaillance critique interdit en plus de
    /// circuler après le jour du contrôle.
    ResultatControle {
        Favorable => "favorable", "Favorable";
        Defavorable => "defavorable", "Défavorable : contre-visite à faire";
        Critique => "critique", "Défavorable : circulation interdite";
    }
}

vocabulaire! {
    /// Ce qui a été fait pendant une intervention, une ligne par opération (#94). L'ordre est
    /// celui du formulaire et des résumés : les opérations courantes d'abord, « Autre » en
    /// dernier, qui porte alors un libellé libre.
    Operation {
        Vidange => "vidange", "Vidange";
        FiltreHuile => "filtre_huile", "Filtre à huile";
        FiltreAir => "filtre_air", "Filtre à air";
        FiltreHabitacle => "filtre_habitacle", "Filtre d'habitacle";
        FiltreCarburant => "filtre_carburant", "Filtre à carburant";
        Bougies => "bougies", "Bougies";
        CourroieDistribution => "courroie_distribution", "Courroie de distribution";
        CourroieAccessoires => "courroie_accessoires", "Courroie d'accessoires";
        Plaquettes => "plaquettes", "Plaquettes de frein";
        Disques => "disques", "Disques de frein";
        LiquideFrein => "liquide_frein", "Liquide de frein";
        LiquideRefroidissement => "liquide_refroidissement", "Liquide de refroidissement";
        Batterie => "batterie", "Batterie";
        Pneus => "pneus", "Pneus";
        Balais => "balais", "Balais d'essuie-glace";
        Climatisation => "climatisation", "Climatisation";
        Autre => "autre", "Autre";
    }
}

vocabulaire! {
    TypeEcheance {
        ControleTechnique => "controle_technique", "Contrôle technique";
        Assurance => "assurance", "Assurance";
        EntretienPeriodique => "entretien_periodique", "Entretien périodique";
    }
}

vocabulaire! {
    StatutEcheance {
        Ouverte => "ouverte", "À traiter";
        Traitee => "traitee", "Traitée";
    }
}

vocabulaire! {
    /// Nature d'un document déposé, choisie au dépôt.
    /// Nature d'un document déposé, choisie au dépôt. La photo du véhicule est la seule dont le
    /// serveur tire une vignette (`metier::vignette`).
    TypeDocument {
        PhotoVehicule => "photo_vehicule", "Photo du véhicule";
        Facture => "facture", "Facture";
        ControleTechnique => "controle_technique", "Contrôle technique";
        CarteGrise => "carte_grise", "Carte grise";
        Assurance => "assurance", "Assurance";
        Permis => "permis", "Permis de conduire";
        Autre => "autre", "Autre";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_code() {
        for v in StatutVehicule::TOUS {
            assert_eq!(StatutVehicule::depuis_code(v.code()), Some(*v));
        }
        for v in TypeIntervention::TOUS {
            assert_eq!(TypeIntervention::depuis_code(v.code()), Some(*v));
        }
    }

    #[test]
    fn code_inconnu_refuse() {
        assert_eq!(StatutVehicule::depuis_code("En service"), None);
        assert_eq!(Role::depuis_code(""), None);
    }

    #[test]
    fn choix_dans_l_ordre() {
        let choix = StatutEcheance::choix();
        assert_eq!(choix[0].code, "ouverte");
        assert_eq!(choix[1].libelle, "Traitée");
    }
}
