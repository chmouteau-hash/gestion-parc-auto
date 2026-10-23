//! Rendu des templates Askama et contexte commun à toutes les pages.

use askama::Template;
use axum::response::Html;
use chrono::NaiveDate;

use crate::commun::types::Profil;
use crate::donnees::affectations::{Affectation, Etat};
use crate::donnees::conducteurs::Conducteur;
use crate::donnees::echeances::Echeance;
use crate::erreurs::Resultat;
use crate::metier::echeances::EtatEcheance;
use crate::web::fait::Fait;

/// Ce que `templates/base.html` affiche sur chaque page : qui est connecté, et quel onglet du
/// menu est actif. Le profil de l'installation y voyage aussi : les gabarits lui demandent ce
/// qu'ils doivent montrer.
#[derive(Debug, Clone)]
pub struct Contexte {
    pub login: String,
    pub est_admin: bool,
    pub page: &'static str,
    pub profil: Profil,
    /// Ce que l'action précédente vient de faire, à dire en tête de page (`web::fait`).
    pub fait: Option<Fait>,
}

impl Contexte {
    /// Qui conduit quoi, depuis quand : utile à une entreprise, sans objet pour un particulier.
    /// Le profil masque, il n'interdit pas : les routes des affectations répondent toujours.
    pub fn montre_affectations(&self) -> bool {
        self.profil == Profil::Entreprise
    }

    /// Le service auquel un conducteur est rattaché n'existe que dans une entreprise.
    pub fn montre_service(&self) -> bool {
        self.profil == Profil::Entreprise
    }

    /// L'entrée du menu qui mène à la racine : un particulier rentre chez lui, un gestionnaire
    /// ouvre son tableau de bord.
    pub fn libelle_accueil(&self) -> &'static str {
        match self.profil {
            Profil::Particulier => "Accueil",
            Profil::Entreprise => "Tableau de bord",
        }
    }

    /// Autoclair Pro a la barre latérale et la recherche ; Autoclair a les quatre onglets.
    pub fn est_entreprise(&self) -> bool {
        self.profil == Profil::Entreprise
    }

    /// L'onglet Autoclair que la page en cours allume. Quatre onglets pour toutes les pages :
    /// l'accueil et tout ce qui touche un véhicule allument « Véhicules », le reste « Plus ».
    pub fn onglet(&self) -> &'static str {
        match self.page {
            "accueil" | "vehicules" => "vehicules",
            "echeances" => "echeances",
            "interventions" => "interventions",
            _ => "plus",
        }
    }
}

/// L'identité visuelle d'une installation. La charte Autoclair a deux marques : Autoclair pour
/// les particuliers, Autoclair Pro pour les entreprises ; chaque profil prend la sienne
/// (`design-system/autoclair/charte/00-fondements.md`).
impl Profil {
    /// Le nom, écrit comme la charte l'exige : « Autoclair » ou « Autoclair Pro », rien d'autre.
    /// Il sert de titre de page et de texte alternatif au logo.
    pub fn marque(self) -> &'static str {
        match self {
            Profil::Entreprise => "Autoclair Pro",
            Profil::Particulier => "Autoclair",
        }
    }

    /// Adresse du logo horizontal, celui de l'en-tête. Le fichier vient de la charte : le nom
    /// n'est jamais recomposé en texte.
    pub fn logo(self) -> &'static str {
        match self {
            Profil::Entreprise => "/static/logos/autoclair-pro.svg",
            Profil::Particulier => "/static/logos/autoclair.svg",
        }
    }

    /// Le même logo en négatif (blanc), pour la barre latérale marine d'Autoclair Pro.
    pub fn logo_negatif(self) -> &'static str {
        match self {
            Profil::Entreprise => "/static/logos/autoclair-pro-negatif.svg",
            Profil::Particulier => "/static/logos/autoclair-negatif.svg",
        }
    }

    /// Adresse de l'icône simplifiée : l'onglet du navigateur, qui montre à lui seul de quelle
    /// installation il s'agit (A marine sur blanc pour Autoclair, A blanc sur marine pour Pro).
    pub fn icone(self) -> &'static str {
        match self {
            Profil::Entreprise => "/static/logos/icone-autoclair-pro.svg",
            Profil::Particulier => "/static/logos/icone-autoclair.svg",
        }
    }

    /// L'icône qui remplace le logo quand il passerait sous sa hauteur minimale de 24 px : la
    /// charte la prévoit pour Autoclair Pro (« sous 24 px de hauteur, utiliser l'icône Pro
    /// seule »). Autoclair n'en a pas : son logo, plus court, tient jusqu'au zoom de 200 % (#145).
    pub fn icone_de_repli(self) -> Option<&'static str> {
        match self {
            Profil::Entreprise => Some(self.icone()),
            Profil::Particulier => None,
        }
    }

    /// Adresse de l'icône d'application (écran d'accueil du téléphone) à une taille donnée :
    /// l'export PNG de la charte, avec ses pointillés. Les tailles servies sont 512, 192 et 180.
    pub fn icone_application(self, taille: u16) -> String {
        let marque = match self {
            Profil::Entreprise => "autoclair-pro",
            Profil::Particulier => "autoclair",
        };
        format!("/static/logos/{marque}-{taille}.png")
    }

    /// La couleur que le téléphone donne à la barre d'état quand l'application est installée :
    /// le marine de la barre latérale pour Autoclair Pro, le blanc du fond d'icône pour
    /// Autoclair. Lue dans les jetons de la charte, jamais écrite ici.
    pub fn couleur_theme(self) -> &'static str {
        match self {
            Profil::Entreprise => crate::web::jeton("--marine-700"),
            Profil::Particulier => crate::web::jeton("--blanc"),
        }
    }

    /// Le fond de l'écran d'attente, entre le toucher de l'icône et la première page : le fond
    /// des pages, pour que la transition ne se voie pas.
    pub fn couleur_fond(self) -> &'static str {
        crate::web::jeton("--fond")
    }

    /// Le manifest d'application web de cette marque : ce que le téléphone lit pour l'installer
    /// sur l'écran d'accueil (nom, icônes, plein écran, couleurs).
    pub fn manifeste(self) -> serde_json::Value {
        serde_json::json!({
            "id": "/",
            "name": self.marque(),
            "short_name": self.marque(),
            "lang": "fr",
            "start_url": "/",
            "scope": "/",
            "display": "standalone",
            "theme_color": self.couleur_theme(),
            "background_color": self.couleur_fond(),
            "icons": [
                { "src": self.icone_application(192), "sizes": "192x192", "type": "image/png" },
                { "src": self.icone_application(512), "sizes": "512x512", "type": "image/png" }
            ]
        })
    }
}

/// Une affectation prête à afficher dans un tableau.
///
/// Son état (en cours, à venir, terminée) dépend de la date du jour : il est calculé ici, une
/// fois, plutôt que dans le template.
pub struct LigneAffectation {
    pub a: Affectation,
    pub etat: Etat,
    /// Vrai si la période couvre aujourd'hui : la ligne est mise en avant, et le bouton
    /// « Terminer aujourd'hui » est proposé.
    pub en_cours: bool,
}

impl LigneAffectation {
    pub fn toutes(affectations: Vec<Affectation>, aujourd_hui: NaiveDate) -> Vec<LigneAffectation> {
        affectations
            .into_iter()
            .map(|a| LigneAffectation {
                etat: a.etat(aujourd_hui),
                en_cours: a.est_en_cours(aujourd_hui),
                a,
            })
            .collect()
    }
}

/// Une échéance prête à afficher dans un tableau.
///
/// Son état (dépassée, à venir, à traiter, traitée) dépend lui aussi de la date du jour : c'est lui que la
/// pastille écrit.
pub struct LigneEcheance {
    pub e: Echeance,
    pub etat: EtatEcheance,
    /// Jours restants jusqu'à la date limite, négatif si elle est passée.
    pub jours: i64,
}

impl LigneEcheance {
    /// `remplacements` : les échéances ouvertes qu'une autre chose remplace, le plan d'entretien
    /// (#94, bloc 2b) ou la contre-visite (#107).
    pub fn toutes(
        echeances: Vec<Echeance>,
        aujourd_hui: NaiveDate,
        horizon_jours: i64,
        remplacements: &crate::metier::alertes::Remplacements,
    ) -> Vec<LigneEcheance> {
        echeances
            .into_iter()
            .map(|e| LigneEcheance {
                etat: e
                    .est_ouverte()
                    .then(|| remplacements.de(&e))
                    .flatten()
                    .unwrap_or_else(|| {
                        crate::metier::echeances::etat(&e, aujourd_hui, horizon_jours)
                    }),
                jours: (e.date_limite - aujourd_hui).num_days(),
                e,
            })
            .collect()
    }

    /// « dans 15 jours », à côté de la date ; rien pour une échéance traitée, dont la date ne
    /// presse plus.
    pub fn delai_fr(&self) -> Option<String> {
        self.e
            .est_ouverte()
            .then(|| crate::metier::alertes::delai_fr(self.jours))
    }
}

/// « À venir » à côté d'une date de permis : un avertissement, donc réservé aux conducteurs actifs,
/// comme la page Alertes. « Expiré », lui, est un fait : il s'écrit pour tous.
pub fn permis_a_venir(c: &Conducteur, aujourd_hui: NaiveDate, horizon_jours: i64) -> bool {
    c.est_actif()
        && crate::metier::alertes::permis_a_venir(
            c.date_expiration_permis,
            aujourd_hui,
            horizon_jours,
        )
}

/// Un conducteur prêt à afficher dans la liste : un permis expiré, ou qui expire dans l'horizon des
/// alertes, se dit à côté de sa date.
pub struct LigneConducteur {
    pub c: Conducteur,
    pub permis_expire: bool,
    pub permis_a_venir: bool,
    /// « depuis 20 jours », « dans 12 jours » : écrit seulement quand la date presse (expiré ou
    /// à venir), pour ne pas commenter un permis valable encore des années.
    pub permis_delai: Option<String>,
}

impl LigneConducteur {
    pub fn tous(
        conducteurs: Vec<Conducteur>,
        aujourd_hui: NaiveDate,
        horizon_jours: i64,
    ) -> Vec<LigneConducteur> {
        conducteurs
            .into_iter()
            .map(|c| Self::une(c, aujourd_hui, horizon_jours))
            .collect()
    }

    pub fn une(c: Conducteur, aujourd_hui: NaiveDate, horizon_jours: i64) -> LigneConducteur {
        let permis_expire = c.permis_expire(aujourd_hui);
        let permis_a_venir = permis_a_venir(&c, aujourd_hui, horizon_jours);
        let permis_delai = (permis_expire || permis_a_venir).then(|| {
            crate::metier::alertes::delai_fr((c.date_expiration_permis - aujourd_hui).num_days())
        });
        LigneConducteur {
            permis_expire,
            permis_a_venir,
            permis_delai,
            c,
        }
    }
}

/// Rend un template en page HTML. Une erreur de rendu est technique (bogue), pas métier.
pub fn rendre<T: Template>(template: T) -> Resultat<Html<String>> {
    Ok(Html(template.render()?))
}
