//! Session de connexion : cookie chiffré et extracteurs de rôle.
//!
//! Le cookie `session` contient `identifiant.expiration` (horodatage Unix), chiffré et signé
//! par `PrivateCookieJar` avec la clé dérivée de `SESSION_SECRET` : un client ne peut ni le lire
//! ni le forger. À chaque requête, l'utilisateur est relu en base : un compte supprimé, désactivé
//! ou dont le rôle a changé est pris en compte immédiatement.
//!
//! Les deux extracteurs sont la seule façon d'obtenir « qui agit » dans une route :
//! - [`UtilisateurConnecte`] : n'importe quel compte, sinon redirection vers `/connexion` ;
//! - [`Administrateur`] : rôle administrateur, sinon 403.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};

use crate::commun::types::{Profil, Role};
use crate::donnees::utilisateurs::{self, Utilisateur};
use crate::erreurs::ErreurApp;
use crate::etat::Etat;
use crate::web::fait::{self, Fait};
use crate::web::rendu::Contexte;

pub const COOKIE_SESSION: &str = "session";
/// Durée de validité d'une connexion.
pub const DUREE_SESSION_SECONDES: i64 = 24 * 60 * 60;

/// Valeur à chiffrer dans le cookie.
pub fn valeur_cookie(utilisateur_id: i64, maintenant: i64) -> String {
    format!("{utilisateur_id}.{}", maintenant + DUREE_SESSION_SECONDES)
}

/// Relit la valeur du cookie : identifiant si le format est bon et la session non expirée.
pub fn lire_valeur_cookie(valeur: &str, maintenant: i64) -> Option<i64> {
    let (id, expiration) = valeur.split_once('.')?;
    let id: i64 = id.parse().ok()?;
    let expiration: i64 = expiration.parse().ok()?;
    (expiration > maintenant && id > 0).then_some(id)
}

pub fn cookie_session(valeur: String, securise: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(COOKIE_SESSION, valeur);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(securise);
    cookie
}

/// Cookie « vide » qui, ajouté au `PrivateCookieJar` par `remove`, efface la session.
pub fn cookie_suppression() -> Cookie<'static> {
    let mut cookie = Cookie::from(COOKIE_SESSION);
    cookie.set_path("/");
    cookie
}

fn maintenant() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Un utilisateur connecté, quel que soit son rôle.
#[derive(Debug, Clone)]
pub struct UtilisateurConnecte {
    pub utilisateur: Utilisateur,
    /// Profil de l'installation, lu dans la configuration : il suit l'utilisateur jusqu'au
    /// contexte des pages, pour qu'aucune route n'ait à le fournir.
    profil: Profil,
    /// Le message de l'action précédente (cookie `fait`), lu ici pour la même raison.
    fait: Option<Fait>,
}

impl UtilisateurConnecte {
    pub fn est_admin(&self) -> bool {
        self.utilisateur.role == Role::Administrateur.code()
    }

    pub fn contexte(&self, page: &'static str) -> Contexte {
        Contexte {
            login: self.utilisateur.login.clone(),
            est_admin: self.est_admin(),
            page,
            profil: self.profil,
            fait: self.fait,
        }
    }
}

impl FromRequestParts<Etat> for UtilisateurConnecte {
    type Rejection = ErreurApp;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApp> {
        let jar = PrivateCookieJar::<Key>::from_request_parts(parts, etat)
            .await
            .unwrap_or_else(|impossible| match impossible {});
        let id = jar
            .get(COOKIE_SESSION)
            .and_then(|cookie| lire_valeur_cookie(cookie.value(), maintenant()))
            .ok_or(ErreurApp::NonConnecte)?;
        let utilisateur = utilisateurs::obtenir(&etat.pool, id)
            .await?
            .filter(|u| u.actif)
            .ok_or(ErreurApp::NonConnecte)?;
        Ok(UtilisateurConnecte {
            utilisateur,
            profil: etat.config.profil,
            fait: fait::lire(&parts.headers),
        })
    }
}

/// Un utilisateur connecté avec le rôle administrateur. Toute route qui écrit l'exige.
#[derive(Debug, Clone)]
pub struct Administrateur {
    pub utilisateur: Utilisateur,
    profil: Profil,
    fait: Option<Fait>,
}

impl Administrateur {
    pub fn contexte(&self, page: &'static str) -> Contexte {
        Contexte {
            login: self.utilisateur.login.clone(),
            est_admin: true,
            page,
            profil: self.profil,
            fait: self.fait,
        }
    }
}

impl FromRequestParts<Etat> for Administrateur {
    type Rejection = ErreurApp;

    async fn from_request_parts(parts: &mut Parts, etat: &Etat) -> Result<Self, ErreurApp> {
        let connecte = UtilisateurConnecte::from_request_parts(parts, etat).await?;
        if connecte.est_admin() {
            Ok(Administrateur {
                utilisateur: connecte.utilisateur,
                profil: connecte.profil,
                fait: connecte.fait,
            })
        } else {
            Err(ErreurApp::Interdit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_valeur() {
        let valeur = valeur_cookie(42, 1_000);
        assert_eq!(lire_valeur_cookie(&valeur, 1_000), Some(42));
        assert_eq!(
            lire_valeur_cookie(&valeur, 1_000 + DUREE_SESSION_SECONDES - 1),
            Some(42)
        );
    }

    #[test]
    fn session_expiree_ou_malformee() {
        let valeur = valeur_cookie(42, 1_000);
        assert_eq!(
            lire_valeur_cookie(&valeur, 1_000 + DUREE_SESSION_SECONDES),
            None
        );
        assert_eq!(lire_valeur_cookie("42", 0), None);
        assert_eq!(lire_valeur_cookie("abc.999999", 0), None);
        assert_eq!(lire_valeur_cookie("0.999999", 0), None);
    }
}
