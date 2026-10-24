//! Vocabulaires fermés : statuts et types.
//!
//! Chaque valeur a un **code** (stocké en base, stable, sans accent) et un **libellé** (affiché).
//! La liste des codes est répétée dans la contrainte `CHECK` de la migration : une valeur qui
//! s'ajoute s'ajoute aux deux endroits, et le test `codes_acceptes_par_la_base` de
//! `tests/vehicules.rs` vérifie que les deux listes sont d'accord.

use chrono::{Datelike, NaiveDate};

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
    /// Couleur de repère d'un véhicule : celle de sa carrosserie, le plus souvent, ou celle qui
    /// aide à le reconnaître d'un coup d'œil parmi les autres. Facultative : le code vide n'est
    /// pas une variante, la saisie le garde tel quel. Le gris, le rouge, le vert et le bleu, les
    /// plus courantes sur une carrosserie, ont trois niveaux (clair, normal, foncé), le marron
    /// deux (clair, normal) ; les autres n'en ont qu'un. L'ordre est celui des couleurs dans leur
    /// rangée du nuancier (voir `rangs`) : les neutres d'abord, puis les teintes dans l'ordre de
    /// l'arc-en-ciel, chaque niveau du plus clair au plus foncé.
    Couleur {
        Blanc => "blanc", "Blanc";
        Beige => "beige", "Beige";
        GrisClair => "gris_clair", "Gris clair";
        Gris => "gris", "Gris";
        GrisFonce => "gris_fonce", "Gris foncé";
        Noir => "noir", "Noir";
        RougeClair => "rouge_clair", "Rouge clair";
        Rouge => "rouge", "Rouge";
        RougeFonce => "rouge_fonce", "Rouge foncé";
        Orange => "orange", "Orange";
        Jaune => "jaune", "Jaune";
        VertClair => "vert_clair", "Vert clair";
        Vert => "vert", "Vert";
        VertFonce => "vert_fonce", "Vert foncé";
        BleuClair => "bleu_clair", "Bleu clair";
        Bleu => "bleu", "Bleu";
        BleuFonce => "bleu_fonce", "Bleu foncé";
        Violet => "violet", "Violet";
        MarronClair => "marron_clair", "Marron clair";
        Marron => "marron", "Marron";
    }
}

impl Couleur {
    /// La couleur dont celle-ci est un niveau : `Gris` pour `GrisClair`, `Gris` et `GrisFonce`.
    /// `None` pour celles qui n'ont qu'un niveau. Sans `_` : une couleur de plus oblige à dire
    /// de quelle rangée du nuancier elle est.
    fn famille(self) -> Option<Couleur> {
        use Couleur::*;
        match self {
            GrisClair | Gris | GrisFonce => Some(Gris),
            RougeClair | Rouge | RougeFonce => Some(Rouge),
            VertClair | Vert | VertFonce => Some(Vert),
            BleuClair | Bleu | BleuFonce => Some(Bleu),
            MarronClair | Marron => Some(Marron),
            Blanc | Beige | Noir | Orange | Jaune | Violet => None,
        }
    }

    /// Le nuancier du formulaire, rangée par rangée : celle des couleurs qui n'ont qu'un niveau,
    /// puis une rangée par couleur à niveaux, du plus clair au plus foncé. Chaque rangée tient
    /// ensemble à l'écran : « Gris clair » ne se retrouve pas seul au bout d'une ligne.
    pub fn rangs() -> Vec<Vec<Choix>> {
        let mut seules = Vec::new();
        let mut familles: Vec<(Couleur, Vec<Choix>)> = Vec::new();
        for couleur in Self::TOUS {
            let choix = Choix {
                code: couleur.code(),
                libelle: couleur.libelle(),
            };
            match couleur.famille() {
                None => seules.push(choix),
                Some(famille) => match familles.iter_mut().find(|(f, _)| *f == famille) {
                    Some((_, rang)) => rang.push(choix),
                    None => familles.push((famille, vec![choix])),
                },
            }
        }
        std::iter::once(seules)
            .chain(familles.into_iter().map(|(_, rang)| rang))
            .collect()
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
        // Le gonflage des pneus, les niveaux du mois : ni une révision ni une réparation (#94).
        EntretienCourant => "entretien_courant", "Entretien courant";
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
    /// celui du formulaire et des résumés : les pièces et consommables, puis les contrôles et
    /// réglages qui ne posent aucune pièce ([`Operation::est_controle`]), « Autre » en dernier,
    /// qui porte alors un libellé libre.
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
        HuileBoite => "huile_boite", "Huile de boîte de vitesses";
        Batterie => "batterie", "Batterie";
        Pneus => "pneus", "Pneus";
        Amortisseurs => "amortisseurs", "Amortisseurs";
        Echappement => "echappement", "Échappement";
        Balais => "balais", "Balais d'essuie-glace";
        AdBlue => "adblue", "AdBlue";
        Climatisation => "climatisation", "Climatisation";
        PressionPneus => "pression_pneus", "Pression des pneus";
        PermutationPneus => "permutation_pneus", "Permutation des pneus";
        Niveaux => "niveaux", "Niveaux (huile, lave-glace, refroidissement)";
        Geometrie => "geometrie", "Géométrie, parallélisme";
        Eclairage => "eclairage", "Éclairage et feux";
        Autre => "autre", "Autre";
    }
}

impl Operation {
    /// Un contrôle ou un réglage, qui ne pose aucune pièce le plus souvent : le formulaire les
    /// range à part des pièces et consommables. « Autre » va avec eux, en dernier.
    pub fn est_controle(self) -> bool {
        matches!(
            self,
            Operation::PressionPneus
                | Operation::PermutationPneus
                | Operation::Niveaux
                | Operation::Geometrie
                | Operation::Eclairage
                | Operation::Autre
        )
    }

    /// Les pièces et consommables, options prêtes pour le formulaire.
    pub fn pieces() -> Vec<Choix> {
        Self::choix()
            .into_iter()
            .filter(|c| !Self::depuis_code(c.code).is_some_and(Self::est_controle))
            .collect()
    }

    /// Les contrôles et réglages, « Autre » compris.
    pub fn controles() -> Vec<Choix> {
        Self::choix()
            .into_iter()
            .filter(|c| Self::depuis_code(c.code).is_some_and(Self::est_controle))
            .collect()
    }
}

vocabulaire! {
    /// « À faire » : ce qu'il y a à remplacer ou à faire sur le véhicule, une seule fois (#98). Il
    /// porte une opération du vocabulaire [`Operation`] (« Autre » pour une pièce absente de la
    /// liste, avec son libellé) et un constat ; il n'a pas de suivante.
    /// « Contre-visite » : créée d'office par un contrôle technique défavorable, à faire dans
    /// les deux mois ; elle n'a pas de suivante (#107).
    TypeEcheance {
        ControleTechnique => "controle_technique", "Contrôle technique";
        ContreVisite => "contre_visite", "Contre-visite";
        Assurance => "assurance", "Assurance";
        EntretienPeriodique => "entretien_periodique", "Entretien périodique";
        AFaire => "a_faire", "À faire";
    }
}

vocabulaire! {
    StatutEcheance {
        Ouverte => "ouverte", "À traiter";
        Traitee => "traitee", "Traitée";
    }
}

vocabulaire! {
    /// Ce qu'on paie pour rouler (#126) : un plein de carburant (ou une recharge), un ticket de
    /// péage. Un seul écran « Dépenses » pour tous les types ; parking ou lavage s'ajouteraient
    /// ici et dans la contrainte de la table, sans nouvel écran.
    TypeDepense {
        Carburant => "carburant", "Carburant";
        Peage => "peage", "Péage";
    }
}

vocabulaire! {
    /// Ce qu'on a mis dans le réservoir à un plein (#126) : les carburants des pompes
    /// françaises, et la recharge électrique. Distinct du carburant du véhicule ([`Carburant`]),
    /// qui dit ce qu'il accepte : celui-ci dit ce qu'on a pris ce jour-là (SP95 ou SP98, E85 sur
    /// un moteur compatible, essence ou recharge sur un hybride rechargeable).
    CarburantPlein {
        Sp95 => "sp95", "SP95";
        Sp95E10 => "sp95_e10", "SP95-E10";
        Sp98 => "sp98", "SP98";
        E85 => "e85", "E85 (superéthanol)";
        Gazole => "gazole", "Gazole (B7)";
        GazoleB10 => "gazole_b10", "Gazole B10";
        Gpl => "gpl", "GPL";
        Gnv => "gnv", "GNV";
        Electricite => "electricite", "Électricité";
        Autre => "autre", "Autre";
    }
}

impl CarburantPlein {
    /// L'unité dans laquelle se note la quantité : des litres, sauf l'électricité en
    /// kilowattheures et le GNV, vendu au kilo.
    pub fn unite(self) -> &'static str {
        match self {
            CarburantPlein::Electricite => "kWh",
            CarburantPlein::Gnv => "kg",
            _ => "L",
        }
    }

    /// Le carburant proposé d'office pour un plein, d'après celui du véhicule : le plus courant
    /// à la pompe pour une essence (SP95-E10), le gazole pour un diesel. Un hybride fait le plein
    /// à la pompe ; sa recharge se note en choisissant « Électricité ».
    pub fn propose_pour(carburant: Carburant) -> CarburantPlein {
        match carburant {
            Carburant::Essence | Carburant::Hybride => CarburantPlein::Sp95E10,
            Carburant::Diesel => CarburantPlein::Gazole,
            Carburant::Electrique => CarburantPlein::Electricite,
            Carburant::Gpl => CarburantPlein::Gpl,
            Carburant::Autre => CarburantPlein::Autre,
        }
    }
}

vocabulaire! {
    /// Classe de la vignette Crit'Air d'un véhicule (#165) : celle de son certificat qualité de
    /// l'air, qu'exigent les zones à faibles émissions. E pour l'électrique et l'hydrogène, puis
    /// de 1 (le moins polluant) à 5 ; un véhicule trop ancien n'est pas classé et n'a pas de
    /// vignette. Facultative : le code vide n'est pas une variante. L'utilisateur la choisit
    /// d'après le certificat ; l'application en propose une ([`CritAir::propose_pour`]).
    CritAir {
        E => "e", "Crit'Air E";
        Un => "1", "Crit'Air 1";
        Deux => "2", "Crit'Air 2";
        Trois => "3", "Crit'Air 3";
        Quatre => "4", "Crit'Air 4";
        Cinq => "5", "Crit'Air 5";
        NonClasse => "non_classe", "Non classé";
    }
}

impl CritAir {
    /// Ce qui s'écrit dans la pastille ronde, comme sur la vignette : « E », ou le chiffre. Vide
    /// pour « non classé », qui n'a pas de vignette.
    pub fn marque(self) -> &'static str {
        match self {
            CritAir::E => "E",
            CritAir::Un => "1",
            CritAir::Deux => "2",
            CritAir::Trois => "3",
            CritAir::Quatre => "4",
            CritAir::Cinq => "5",
            CritAir::NonClasse => "",
        }
    }

    /// Les tranches de la classe proposée pour un carburant : (première année, classe), de la
    /// plus récente à la plus ancienne ; la dernière couvre tout ce qui précède. Aucune pour
    /// « Autre ». Toutes les tranches commencent un 1er janvier : l'année suffit.
    ///
    /// Tableau « Voitures » de l'annexe I de l'arrêté du 21 juin 2016 (certificat-air.gouv.fr,
    /// `tableaux_classement.pdf`, lu le 24/10/2026). La date de mise en service y tient lieu de
    /// date de première immatriculation. L'application ne sait pas si un hybride est
    /// rechargeable (classé 1) : il suit l'essence. Le GPL est un véhicule gaz (classé 1).
    pub fn tranches(carburant: Carburant) -> &'static [(i32, CritAir)] {
        use CritAir::*;
        match carburant {
            Carburant::Electrique => &[(i32::MIN, E)],
            Carburant::Gpl => &[(i32::MIN, Un)],
            Carburant::Essence | Carburant::Hybride => &[
                (2011, Un),
                (2006, Deux),
                (1997, Trois),
                (i32::MIN, NonClasse),
            ],
            Carburant::Diesel => &[
                (2011, Deux),
                (2006, Trois),
                (2001, Quatre),
                (1997, Cinq),
                (i32::MIN, NonClasse),
            ],
            Carburant::Autre => &[],
        }
    }

    /// La classe que l'application propose d'après le carburant et la date de mise en service.
    /// Ce n'est qu'une proposition : la vignette qui fait foi est celle du certificat. Un
    /// véhicule d'occasion a souvent été immatriculé bien avant sa mise en service dans le parc,
    /// et un utilitaire léger a des dates un peu différentes.
    pub fn propose_pour(carburant: Carburant, mise_en_service: NaiveDate) -> Option<CritAir> {
        let annee = mise_en_service.year();
        CritAir::tranches(carburant)
            .iter()
            .find(|(debut, _)| annee >= *debut)
            .map(|(_, classe)| *classe)
    }
}

vocabulaire! {
    /// Nature d'un document déposé, choisie au dépôt. La photo du véhicule est la seule dont le
    /// serveur tire une vignette (`metier::vignette`). Chaque type a son [dossier](Dossier) sur
    /// la fiche du véhicule (#109), sauf la photo, qui a son bloc.
    TypeDocument {
        PhotoVehicule => "photo_vehicule", "Photo du véhicule";
        Facture => "facture", "Facture";
        ControleTechnique => "controle_technique", "Contrôle technique";
        CarteGrise => "carte_grise", "Carte grise";
        Assurance => "assurance", "Assurance";
        CertificatQualiteAir => "certificat_qualite_air", "Certificat qualité de l'air (Crit'Air)";
        FactureAchat => "facture_achat", "Facture d'achat";
        CertificatCession => "certificat_cession", "Certificat de cession";
        ContratAchat => "contrat_achat", "Contrat ou bon de commande";
        Garantie => "garantie", "Garantie";
        Ticket => "ticket", "Ticket de carburant ou de péage";
        Permis => "permis", "Permis de conduire";
        Autre => "autre", "Autre";
    }
}

vocabulaire! {
    /// Les dossiers de la section « Documents » d'une fiche véhicule (#109), dans l'ordre
    /// d'affichage. Un dossier se déduit du type du document ([`TypeDocument::dossier`]) : il
    /// n'existe pas en base. Le code sert dans l'adresse de la page de dépôt
    /// (`/documents/nouveau?dossier=achat`).
    Dossier {
        Achat => "achat", "Achat";
        Administratif => "administratif", "Administratif";
        Entretien => "entretien", "Entretien";
        Depenses => "depenses", "Dépenses";
        Autres => "autres", "Autres";
    }
}

impl TypeDocument {
    /// Le dossier où la fiche du véhicule range un document de ce type. `None` pour la photo du
    /// véhicule : elle est en tête de fiche et dans le bloc « Photos », pas dans un tableau.
    pub fn dossier(self) -> Option<Dossier> {
        match self {
            TypeDocument::PhotoVehicule => None,
            TypeDocument::FactureAchat
            | TypeDocument::CertificatCession
            | TypeDocument::ContratAchat
            | TypeDocument::Garantie => Some(Dossier::Achat),
            TypeDocument::CarteGrise
            | TypeDocument::Assurance
            | TypeDocument::CertificatQualiteAir => Some(Dossier::Administratif),
            TypeDocument::Facture | TypeDocument::ControleTechnique => Some(Dossier::Entretien),
            // Le ticket d'un plein ou d'un péage (#126, bloc 2).
            TypeDocument::Ticket => Some(Dossier::Depenses),
            // Le permis est un document de conducteur ; s'il se trouve sur un véhicule (l'API
            // accepte tout type), il ne disparaît pas de la fiche.
            TypeDocument::Permis | TypeDocument::Autre => Some(Dossier::Autres),
        }
    }
}

impl Dossier {
    /// Les types de document du dossier, dans l'ordre de `TypeDocument::TOUS`. Jamais vide.
    pub fn types(self) -> Vec<TypeDocument> {
        TypeDocument::TOUS
            .iter()
            .copied()
            .filter(|t| t.dossier() == Some(self))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(jour: u32, mois: u32, annee: i32) -> NaiveDate {
        NaiveDate::from_ymd_opt(annee, mois, jour).unwrap()
    }

    #[test]
    fn la_vignette_proposee_suit_le_tableau_des_voitures() {
        use CritAir::*;
        let cas = [
            // Essence, et hybride comme l'essence : 1 depuis 2011, 2 de 2006 à 2010, 3 de 1997
            // à 2005, non classé avant. Chaque tranche se vérifie à ses deux bords.
            (Carburant::Essence, le(1, 1, 2011), Some(Un)),
            (Carburant::Essence, le(31, 12, 2010), Some(Deux)),
            (Carburant::Essence, le(1, 1, 2006), Some(Deux)),
            (Carburant::Essence, le(31, 12, 2005), Some(Trois)),
            (Carburant::Essence, le(1, 1, 1997), Some(Trois)),
            (Carburant::Essence, le(31, 12, 1996), Some(NonClasse)),
            (Carburant::Hybride, le(15, 3, 2008), Some(Deux)),
            (Carburant::Hybride, le(1, 6, 2022), Some(Un)),
            // Diesel : 2 depuis 2011, 3 de 2006 à 2010, 4 de 2001 à 2005, 5 de 1997 à 2000.
            (Carburant::Diesel, le(1, 1, 2011), Some(Deux)),
            (Carburant::Diesel, le(31, 12, 2010), Some(Trois)),
            (Carburant::Diesel, le(15, 3, 2008), Some(Trois)),
            (Carburant::Diesel, le(1, 1, 2006), Some(Trois)),
            (Carburant::Diesel, le(31, 12, 2005), Some(Quatre)),
            (Carburant::Diesel, le(1, 1, 2001), Some(Quatre)),
            (Carburant::Diesel, le(31, 12, 2000), Some(Cinq)),
            (Carburant::Diesel, le(1, 1, 1997), Some(Cinq)),
            (Carburant::Diesel, le(31, 12, 1996), Some(NonClasse)),
            // Électrique, GPL : quelle que soit la date. « Autre » : rien à proposer.
            (Carburant::Electrique, le(1, 1, 1990), Some(E)),
            (Carburant::Electrique, le(1, 1, 2024), Some(E)),
            (Carburant::Gpl, le(1, 1, 1995), Some(Un)),
            (Carburant::Autre, le(1, 1, 2020), None),
        ];
        for (carburant, date, attendue) in cas {
            assert_eq!(
                CritAir::propose_pour(carburant, date),
                attendue,
                "{carburant:?} mis en service le {date}"
            );
        }
    }

    #[test]
    fn chaque_classe_a_sa_marque_sauf_non_classe() {
        let marques: Vec<&str> = CritAir::TOUS.iter().map(|c| c.marque()).collect();
        assert_eq!(marques, ["E", "1", "2", "3", "4", "5", ""]);
    }

    #[test]
    fn le_certificat_crit_air_est_un_document_administratif() {
        assert_eq!(
            TypeDocument::CertificatQualiteAir.dossier(),
            Some(Dossier::Administratif)
        );
    }

    #[test]
    fn le_nuancier_range_chaque_couleur_une_seule_fois() {
        let rangs = Couleur::rangs();
        let codes: Vec<&str> = rangs.iter().flatten().map(|c| c.code).collect();
        assert_eq!(codes.len(), Couleur::TOUS.len());
        for couleur in Couleur::TOUS {
            let fois = codes.iter().filter(|c| **c == couleur.code()).count();
            assert_eq!(fois, 1, "{couleur:?}");
        }
        // Les couleurs sans niveau d'abord, puis une rangée par couleur à niveaux, du plus clair
        // au plus foncé.
        let rangee = |i: usize| rangs[i].iter().map(|c| c.code).collect::<Vec<_>>();
        assert_eq!(rangs.len(), 6);
        assert_eq!(
            rangee(0),
            ["blanc", "beige", "noir", "orange", "jaune", "violet"]
        );
        assert_eq!(rangee(1), ["gris_clair", "gris", "gris_fonce"]);
        assert_eq!(rangee(4), ["bleu_clair", "bleu", "bleu_fonce"]);
        assert_eq!(rangee(5), ["marron_clair", "marron"]);
    }

    #[test]
    fn aller_retour_code() {
        for v in StatutVehicule::TOUS {
            assert_eq!(StatutVehicule::depuis_code(v.code()), Some(*v));
        }
        for v in TypeIntervention::TOUS {
            assert_eq!(TypeIntervention::depuis_code(v.code()), Some(*v));
        }
    }

    /// Le formulaire range les opérations en deux groupes : chacune est dans un seul, dans l'ordre
    /// du vocabulaire, et « Autre » ferme le second (#94).
    #[test]
    fn chaque_operation_est_dans_un_seul_groupe() {
        let pieces: Vec<_> = Operation::pieces().iter().map(|c| c.code).collect();
        let controles: Vec<_> = Operation::controles().iter().map(|c| c.code).collect();
        let tous: Vec<_> = Operation::TOUS.iter().map(|o| o.code()).collect();
        assert_eq!([pieces.clone(), controles.clone()].concat(), tous);
        assert_eq!(pieces.first(), Some(&"vidange"));
        assert_eq!(
            controles,
            [
                "pression_pneus",
                "permutation_pneus",
                "niveaux",
                "geometrie",
                "eclairage",
                "autre"
            ]
        );
    }

    #[test]
    fn chaque_type_de_document_a_son_dossier_sauf_la_photo() {
        for t in TypeDocument::TOUS {
            assert_eq!(
                t.dossier().is_none(),
                *t == TypeDocument::PhotoVehicule,
                "{t:?}"
            );
        }
        assert_eq!(TypeDocument::FactureAchat.dossier(), Some(Dossier::Achat));
        assert_eq!(
            TypeDocument::CarteGrise.dossier(),
            Some(Dossier::Administratif)
        );
        assert_eq!(TypeDocument::Facture.dossier(), Some(Dossier::Entretien));
        assert_eq!(TypeDocument::Ticket.dossier(), Some(Dossier::Depenses));
        assert_eq!(TypeDocument::Autre.dossier(), Some(Dossier::Autres));
        // Chaque dossier a ses types, et tout type (hors photo) est dans un seul dossier.
        let total: usize = Dossier::TOUS.iter().map(|d| d.types().len()).sum();
        assert_eq!(total, TypeDocument::TOUS.len() - 1);
        assert_eq!(
            Dossier::Achat.types(),
            vec![
                TypeDocument::FactureAchat,
                TypeDocument::CertificatCession,
                TypeDocument::ContratAchat,
                TypeDocument::Garantie,
            ]
        );
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
