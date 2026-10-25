//! Le message de confirmation après une action : « Échéance marquée traitée. », « Véhicule
//! enregistré. »…
//!
//! Une action est un `POST` qui redirige vers une page. Pour que cette page dise ce qui vient
//! d'être fait, la redirection pose un cookie `fait` qui porte un **code** d'un vocabulaire fermé ;
//! la page suivante le lit (`session.rs` le met dans le [`Contexte`](crate::web::rendu::Contexte)),
//! l'affiche, et l'intergiciel de `web/mod.rs` l'efface dans la même réponse. Un code inconnu
//! n'affiche rien : le cookie ne porte jamais de texte libre.

use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use crate::commun::types::TypeEcheance;
use crate::metier::echeances::SuiteContreVisite;

pub const COOKIE_FAIT: &str = "fait";

/// Ce qu'une action vient de faire. Chaque valeur a son code (dans le cookie) et son message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fait {
    EcheanceTraitee,
    KilometrageReleve,
    ReleveModifie,
    ReleveSupprime,
    VehiculeCree,
    VehiculeCreeAvecPhoto,
    VehiculeModifie,
    VehiculeRemisEnService,
    VehiculeRemisEnServiceSansAffectations,
    VehiculeSupprime,
    ConducteurCree,
    ConducteurModifie,
    ConducteurSupprime,
    EcheanceCreee,
    EcheanceModifiee,
    EcheanceSupprimee,
    InterventionCreee,
    InterventionCreeeAvecFacture,
    InterventionSoldeControleTechnique,
    InterventionSoldeControleTechniqueAvecFacture,
    InterventionSoldeEntretienPeriodique,
    InterventionSoldeEntretienPeriodiqueAvecFacture,
    InterventionSoldeContreVisite,
    InterventionSoldeContreVisiteAvecFacture,
    InterventionContreVisiteAFaire,
    InterventionContreVisiteAFaireAvecFacture,
    InterventionControleARefaire,
    InterventionControleARefaireAvecFacture,
    InterventionSoldeAFaire,
    InterventionSoldeAFaireAvecFacture,
    InterventionSoldeAFairePlusieurs,
    InterventionSoldeAFairePlusieursAvecFacture,
    InterventionModifiee,
    InterventionModifieeAvecFacture,
    InterventionSupprimee,
    OperationSuivie,
    OperationSuivieAvecDerniereFois,
    PlanModifie,
    PlanDeDepart,
    OperationPlusSuivie,
    PleinEnregistre,
    PleinEnregistreAvecTicket,
    PeageEnregistre,
    PeageEnregistreAvecTicket,
    DepenseModifiee,
    DepenseModifieeAvecTicket,
    DepenseSupprimee,
    AffectationCreee,
    AffectationModifiee,
    AffectationCloturee,
    AffectationSupprimee,
    DocumentDepose,
    DocumentSupprime,
    DocumentRenomme,
    PhotoMiseEnTete,
    MotDePasseChange,
    CourrielChange,
    CompteCree,
    CompteDesactive,
    CompteReactive,
    MotDePasseRemplace,
}

const TOUS: &[(Fait, &str, &str)] = &[
    (
        Fait::EcheanceTraitee,
        "echeance-traitee",
        "Échéance marquée traitée.",
    ),
    (
        Fait::KilometrageReleve,
        "kilometrage-releve",
        "Kilométrage mis à jour.",
    ),
    (
        Fait::ReleveModifie,
        "releve-modifie",
        "Relevé corrigé. Le compteur suit l'historique.",
    ),
    (
        Fait::ReleveSupprime,
        "releve-supprime",
        "Relevé supprimé. Le compteur suit l'historique.",
    ),
    (Fait::VehiculeCree, "vehicule-cree", "Véhicule enregistré."),
    (
        Fait::VehiculeCreeAvecPhoto,
        "vehicule-cree-photo",
        "Véhicule enregistré, avec sa photo.",
    ),
    (
        Fait::VehiculeModifie,
        "vehicule-modifie",
        "Véhicule modifié.",
    ),
    (
        Fait::VehiculeRemisEnService,
        "vehicule-remis-en-service",
        "Véhicule remis en service. Les affectations que la réforme a terminées ne sont pas rétablies.",
    ),
    (
        Fait::VehiculeRemisEnServiceSansAffectations,
        "vehicule-remis-en-service-seul",
        "Véhicule remis en service.",
    ),
    (
        Fait::VehiculeSupprime,
        "vehicule-supprime",
        "Véhicule supprimé.",
    ),
    (
        Fait::ConducteurCree,
        "conducteur-cree",
        "Conducteur enregistré.",
    ),
    (
        Fait::ConducteurModifie,
        "conducteur-modifie",
        "Conducteur modifié.",
    ),
    (
        Fait::ConducteurSupprime,
        "conducteur-supprime",
        "Conducteur supprimé.",
    ),
    (
        Fait::EcheanceCreee,
        "echeance-creee",
        "Échéance enregistrée.",
    ),
    (
        Fait::EcheanceModifiee,
        "echeance-modifiee",
        "Échéance modifiée.",
    ),
    (
        Fait::EcheanceSupprimee,
        "echeance-supprimee",
        "Échéance supprimée.",
    ),
    (
        Fait::InterventionCreee,
        "intervention-creee",
        "Intervention enregistrée.",
    ),
    (
        Fait::InterventionCreeeAvecFacture,
        "intervention-creee-facture",
        "Intervention enregistrée, avec sa facture.",
    ),
    (
        Fait::InterventionSoldeControleTechnique,
        "intervention-solde-ct",
        "Intervention enregistrée. L'échéance « Contrôle technique » est traitée : voici la suivante, datée de la visite.",
    ),
    (
        Fait::InterventionSoldeControleTechniqueAvecFacture,
        "intervention-solde-ct-facture",
        "Intervention enregistrée, avec sa facture. L'échéance « Contrôle technique » est traitée : voici la suivante, datée de la visite.",
    ),
    (
        Fait::InterventionSoldeEntretienPeriodique,
        "intervention-solde-ep",
        "Intervention enregistrée. L'échéance « Entretien périodique » est traitée : voici la suivante, datée de la révision.",
    ),
    (
        Fait::InterventionSoldeEntretienPeriodiqueAvecFacture,
        "intervention-solde-ep-facture",
        "Intervention enregistrée, avec sa facture. L'échéance « Entretien périodique » est traitée : voici la suivante, datée de la révision.",
    ),
    (
        Fait::InterventionSoldeContreVisite,
        "intervention-solde-cv",
        "Intervention enregistrée. La contre-visite est faite : voici le prochain contrôle technique, deux ans après le contrôle initial.",
    ),
    (
        Fait::InterventionSoldeContreVisiteAvecFacture,
        "intervention-solde-cv-facture",
        "Intervention enregistrée, avec sa facture. La contre-visite est faite : voici le prochain contrôle technique, deux ans après le contrôle initial.",
    ),
    (
        Fait::InterventionContreVisiteAFaire,
        "intervention-contre-visite",
        "Intervention enregistrée. Contrôle défavorable : la contre-visite est à faire dans les deux mois, son échéance est créée.",
    ),
    (
        Fait::InterventionContreVisiteAFaireAvecFacture,
        "intervention-contre-visite-facture",
        "Intervention enregistrée, avec sa facture. Contrôle défavorable : la contre-visite est à faire dans les deux mois, son échéance est créée.",
    ),
    (
        Fait::InterventionControleARefaire,
        "intervention-controle-a-refaire",
        "Intervention enregistrée. Contre-visite défavorable, ou faite après les deux mois : un contrôle technique complet est à refaire.",
    ),
    (
        Fait::InterventionControleARefaireAvecFacture,
        "intervention-controle-a-refaire-facture",
        "Intervention enregistrée, avec sa facture. Contre-visite défavorable, ou faite après les deux mois : un contrôle technique complet est à refaire.",
    ),
    (
        Fait::InterventionSoldeAFaire,
        "intervention-solde-a-faire",
        "Intervention enregistrée. L'échéance « À faire » qu'elle remplit est traitée.",
    ),
    (
        Fait::InterventionSoldeAFaireAvecFacture,
        "intervention-solde-a-faire-facture",
        "Intervention enregistrée, avec sa facture. L'échéance « À faire » qu'elle remplit est traitée.",
    ),
    (
        Fait::InterventionSoldeAFairePlusieurs,
        "intervention-solde-a-faire-plusieurs",
        "Intervention enregistrée. Les échéances « À faire » qu'elle remplit sont traitées.",
    ),
    (
        Fait::InterventionSoldeAFairePlusieursAvecFacture,
        "intervention-solde-a-faire-plusieurs-facture",
        "Intervention enregistrée, avec sa facture. Les échéances « À faire » qu'elle remplit sont traitées.",
    ),
    (
        Fait::InterventionModifiee,
        "intervention-modifiee",
        "Intervention modifiée.",
    ),
    (
        Fait::InterventionModifieeAvecFacture,
        "intervention-modifiee-facture",
        "Intervention modifiée, facture jointe.",
    ),
    (
        Fait::InterventionSupprimee,
        "intervention-supprimee",
        "Intervention supprimée.",
    ),
    (
        Fait::OperationSuivie,
        "operation-suivie",
        "Opération ajoutée au plan d'entretien.",
    ),
    (
        Fait::OperationSuivieAvecDerniereFois,
        "operation-suivie-avec-derniere-fois",
        "Opération ajoutée au plan d'entretien. Sa dernière fois est notée dans l'entretien.",
    ),
    (
        Fait::PlanModifie,
        "plan-modifie",
        "Plan d'entretien modifié.",
    ),
    (
        Fait::PlanDeDepart,
        "plan-de-depart",
        "Plan d'entretien enregistré : les opérations cochées sont suivies.",
    ),
    (
        Fait::OperationPlusSuivie,
        "operation-plus-suivie",
        "Opération retirée du plan d'entretien. Les interventions restent dans l'entretien.",
    ),
    (
        Fait::PleinEnregistre,
        "plein-enregistre",
        "Plein enregistré.",
    ),
    (
        Fait::PleinEnregistreAvecTicket,
        "plein-enregistre-ticket",
        "Plein enregistré, avec son ticket.",
    ),
    (
        Fait::PeageEnregistre,
        "peage-enregistre",
        "Péage enregistré.",
    ),
    (
        Fait::PeageEnregistreAvecTicket,
        "peage-enregistre-ticket",
        "Péage enregistré, avec son ticket.",
    ),
    (
        Fait::DepenseModifiee,
        "depense-modifiee",
        "Dépense modifiée.",
    ),
    (
        Fait::DepenseModifieeAvecTicket,
        "depense-modifiee-ticket",
        "Dépense modifiée, ticket joint.",
    ),
    (
        Fait::DepenseSupprimee,
        "depense-supprimee",
        "Dépense supprimée.",
    ),
    (
        Fait::AffectationCreee,
        "affectation-creee",
        "Affectation enregistrée.",
    ),
    (
        Fait::AffectationModifiee,
        "affectation-modifiee",
        "Affectation modifiée.",
    ),
    (
        Fait::AffectationCloturee,
        "affectation-cloturee",
        "Affectation terminée aujourd'hui.",
    ),
    (
        Fait::AffectationSupprimee,
        "affectation-supprimee",
        "Affectation supprimée.",
    ),
    (
        Fait::DocumentDepose,
        "document-depose",
        "Document enregistré.",
    ),
    (
        Fait::DocumentSupprime,
        "document-supprime",
        "Document supprimé.",
    ),
    (
        Fait::DocumentRenomme,
        "document-renomme",
        "Document renommé.",
    ),
    (
        Fait::PhotoMiseEnTete,
        "photo-en-tete",
        "Photo mise en tête de la fiche.",
    ),
    (
        Fait::MotDePasseChange,
        "mot-de-passe-change",
        "Mot de passe changé.",
    ),
    (
        Fait::CourrielChange,
        "courriel-change",
        "Adresse de courriel enregistrée.",
    ),
    (Fait::CompteCree, "compte-cree", "Compte créé."),
    (
        Fait::CompteDesactive,
        "compte-desactive",
        "Compte désactivé : il ne peut plus se connecter.",
    ),
    (Fait::CompteReactive, "compte-reactive", "Compte réactivé."),
    (
        Fait::MotDePasseRemplace,
        "mot-de-passe-remplace",
        "Mot de passe remplacé : à communiquer à la personne.",
    ),
];

impl Fait {
    pub fn code(self) -> &'static str {
        TOUS.iter()
            .find(|(fait, _, _)| *fait == self)
            .map(|(_, code, _)| *code)
            .unwrap_or("")
    }

    pub fn message(self) -> &'static str {
        TOUS.iter()
            .find(|(fait, _, _)| *fait == self)
            .map(|(_, _, message)| *message)
            .unwrap_or("")
    }

    /// Le message d'une intervention qui a soldé une échéance (#94, bloc 3) : il la nomme, et
    /// dit si la facture est jointe.
    pub fn intervention_soldant(type_echeance: TypeEcheance, avec_facture: bool) -> Fait {
        match (type_echeance, avec_facture) {
            (TypeEcheance::ControleTechnique, false) => Fait::InterventionSoldeControleTechnique,
            (TypeEcheance::ControleTechnique, true) => {
                Fait::InterventionSoldeControleTechniqueAvecFacture
            }
            (TypeEcheance::ContreVisite, false) => Fait::InterventionSoldeContreVisite,
            (TypeEcheance::ContreVisite, true) => Fait::InterventionSoldeContreVisiteAvecFacture,
            (_, false) => Fait::InterventionSoldeEntretienPeriodique,
            (_, true) => Fait::InterventionSoldeEntretienPeriodiqueAvecFacture,
        }
    }

    /// Le message d'un contrôle technique défavorable, qui crée la contre-visite, ou d'une
    /// contre-visite manquée, qui demande un contrôle complet (#107).
    pub fn contre_visite(suite: SuiteContreVisite, avec_facture: bool) -> Fait {
        match (suite, avec_facture) {
            (SuiteContreVisite::AFaire(_), false) => Fait::InterventionContreVisiteAFaire,
            (SuiteContreVisite::AFaire(_), true) => Fait::InterventionContreVisiteAFaireAvecFacture,
            (SuiteContreVisite::ControleARefaire, false) => Fait::InterventionControleARefaire,
            (SuiteContreVisite::ControleARefaire, true) => {
                Fait::InterventionControleARefaireAvecFacture
            }
        }
    }

    /// Le message d'une intervention qui a soldé des choses à faire de même opération (#107,
    /// bloc 4) : une ou plusieurs, avec ou sans facture.
    pub fn choses_faites(nombre: usize, avec_facture: bool) -> Fait {
        match (nombre > 1, avec_facture) {
            (false, false) => Fait::InterventionSoldeAFaire,
            (false, true) => Fait::InterventionSoldeAFaireAvecFacture,
            (true, false) => Fait::InterventionSoldeAFairePlusieurs,
            (true, true) => Fait::InterventionSoldeAFairePlusieursAvecFacture,
        }
    }

    pub fn depuis_code(code: &str) -> Option<Fait> {
        TOUS.iter()
            .find(|(_, c, _)| *c == code)
            .map(|(fait, _, _)| *fait)
    }
}

/// Redirige vers `vers` en posant le message : la page d'arrivée le dira.
pub fn rediriger(vers: &str, fait: Fait) -> Response {
    let mut cookie = Cookie::new(COOKIE_FAIT, fait.code());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    (CookieJar::new().add(cookie), Redirect::to(vers)).into_response()
}

/// Le message que la requête apporte, s'il y en a un et s'il est connu.
pub fn lire(entetes: &HeaderMap) -> Option<Fait> {
    entetes
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|valeur| valeur.to_str().ok())
        .flat_map(|valeur| valeur.split(';'))
        .filter_map(|morceau| morceau.trim().strip_prefix("fait="))
        .find_map(Fait::depuis_code)
}

/// `Set-Cookie` qui efface le message, une fois la page qui le porte servie.
pub fn entete_suppression() -> HeaderValue {
    HeaderValue::from_static("fait=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_message_d_une_intervention_qui_solde_nomme_l_echeance() {
        let m = |t, f| Fait::intervention_soldant(t, f).message();
        assert!(m(TypeEcheance::ControleTechnique, false)
            .contains("« Contrôle technique » est traitée"));
        assert!(m(TypeEcheance::ControleTechnique, true)
            .starts_with("Intervention enregistrée, avec sa facture."));
        assert!(m(TypeEcheance::EntretienPeriodique, false)
            .contains("« Entretien périodique » est traitée"));
        assert!(m(TypeEcheance::EntretienPeriodique, true).contains("avec sa facture"));
    }

    #[test]
    fn les_messages_de_la_contre_visite() {
        let limite = chrono::NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        assert!(
            Fait::contre_visite(SuiteContreVisite::AFaire(limite), false)
                .message()
                .contains("la contre-visite est à faire dans les deux mois")
        );
        assert!(
            Fait::contre_visite(SuiteContreVisite::ControleARefaire, true)
                .message()
                .starts_with("Intervention enregistrée, avec sa facture.")
        );
        assert!(
            Fait::intervention_soldant(TypeEcheance::ContreVisite, false)
                .message()
                .contains("deux ans après le contrôle initial")
        );
    }

    #[test]
    fn les_messages_des_choses_a_faire() {
        assert!(Fait::choses_faites(1, false)
            .message()
            .contains("L'échéance « À faire » qu'elle remplit est traitée"));
        assert!(Fait::choses_faites(2, false)
            .message()
            .contains("Les échéances « À faire » qu'elle remplit sont traitées"));
        assert!(Fait::choses_faites(1, true)
            .message()
            .starts_with("Intervention enregistrée, avec sa facture."));
        assert!(Fait::choses_faites(3, true)
            .message()
            .starts_with("Intervention enregistrée, avec sa facture."));
    }

    #[test]
    fn chaque_fait_a_un_code_unique_et_un_message() {
        for (fait, code, message) in TOUS {
            assert_eq!(fait.code(), *code);
            assert_eq!(fait.message(), *message);
            assert_eq!(Fait::depuis_code(code), Some(*fait));
            assert!(message.ends_with('.'), "{code}");
        }
        let codes: std::collections::HashSet<_> = TOUS.iter().map(|(_, c, _)| *c).collect();
        assert_eq!(codes.len(), TOUS.len());
        assert_eq!(Fait::depuis_code("inconnu"), None);
    }

    #[test]
    fn le_cookie_se_lit_parmi_les_autres() {
        let mut entetes = HeaderMap::new();
        entetes.insert(
            header::COOKIE,
            HeaderValue::from_static("session=abc; fait=echeance-traitee; autre=1"),
        );
        assert_eq!(lire(&entetes), Some(Fait::EcheanceTraitee));
        entetes.insert(header::COOKIE, HeaderValue::from_static("fait=zzz"));
        assert_eq!(lire(&entetes), None);
        assert_eq!(lire(&HeaderMap::new()), None);
    }
}
