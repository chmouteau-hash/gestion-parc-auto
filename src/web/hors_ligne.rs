//! La page « Pas de réseau » et le service worker qui l'affiche (#180).
//!
//! Deux routes sans session : le navigateur les demande sans que personne soit connecté, et la
//! page est mise en cache à l'installation du service worker. Ni l'une ni l'autre ne touche la
//! base : la page doit se servir même quand tout le reste échoue.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use askama::Template;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::commun::types::Profil;
use crate::erreurs::Resultat;
use crate::etat::Etat;
use crate::web::rendu::rendre;
use crate::web::{JETONS, SCRIPT, STYLE};

/// Le service worker, embarqué comme le reste ; deux repères y sont remplacés avant l'envoi
/// (voir [`composer`]).
const SERVICE_WORKER: &str = include_str!("../../static/sw.js");
/// Le gabarit de la page, lu une seconde fois pour entrer dans l'empreinte de [`version`].
const GABARIT_PAGE: &str = include_str!("../../templates/hors_ligne.html");

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/hors-ligne", get(page))
        .route("/sw.js", get(service_worker))
}

#[derive(Template)]
#[template(path = "hors_ligne.html")]
struct PageHorsLigne {
    profil: Profil,
}

async fn page(State(etat): State<Etat>) -> Resultat<Html<String>> {
    rendre(PageHorsLigne {
        profil: etat.config.profil,
    })
}

/// Le service worker de la marque servie, sans cache HTTP : le navigateur le relit à chaque
/// ouverture pour savoir si une version nouvelle l'attend.
async fn service_worker(State(etat): State<Etat>) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        composer(etat.config.profil),
    )
        .into_response()
}

/// Ce que le service worker garde pour afficher la page hors ligne : la page, les feuilles de
/// style, les polices, et les logos de la marque (le grand, l'icône de l'onglet, et pour
/// Autoclair Pro l'icône qui remplace le logo sous 24 px). Rien d'autre : aucune page avec des
/// données.
pub fn fichiers(profil: Profil) -> Vec<&'static str> {
    let mut liste = vec![
        "/hors-ligne",
        "/static/tokens.css",
        "/static/style.css",
        "/static/polices/montserrat.woff2",
        "/static/polices/instrument-sans.woff2",
        profil.logo(),
        profil.icone(),
    ];
    // Sans doublon : le navigateur refuse une liste qui cite deux fois la même adresse, et
    // l'icône de repli d'Autoclair Pro est celle de son onglet.
    let repli = profil.icone_de_repli().filter(|r| !liste.contains(r));
    liste.extend(repli);
    liste
}

/// Le script servi : le gabarit `static/sw.js` avec la version et la liste des fichiers.
fn composer(profil: Profil) -> String {
    let liste = serde_json::to_string(&fichiers(profil)).expect("liste de chemins");
    SERVICE_WORKER
        .replace("__VERSION__", version())
        .replace("__FICHIERS__", &liste)
}

/// L'empreinte de ce que le cache contient : change avec la feuille de style, les jetons, le
/// script des pages, la page hors ligne, le service worker lui-même ou la version de
/// l'application. Calculée une fois par processus. Les polices et les logos ne sont pas comptés :
/// ils ne changent qu'avec une version de l'application.
pub fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let mut empreinte = DefaultHasher::new();
        env!("CARGO_PKG_VERSION").hash(&mut empreinte);
        STYLE.hash(&mut empreinte);
        JETONS.hash(&mut empreinte);
        SCRIPT.hash(&mut empreinte);
        GABARIT_PAGE.hash(&mut empreinte);
        SERVICE_WORKER.hash(&mut empreinte);
        format!("{:016x}", empreinte.finish())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_version_est_stable_et_les_reperes_remplaces() {
        assert_eq!(version(), version());
        assert_eq!(version().len(), 16);
        for profil in [Profil::Entreprise, Profil::Particulier] {
            let script = composer(profil);
            assert!(!script.contains("__VERSION__"), "{profil:?}");
            assert!(!script.contains("__FICHIERS__"), "{profil:?}");
            assert!(
                script.contains(&format!("\"{}\"", profil.logo())),
                "{profil:?}"
            );
        }
        // Sept fichiers, sans doublon : l'icône de repli d'Autoclair Pro est déjà celle de son
        // onglet, et Autoclair n'en a pas.
        for profil in [Profil::Entreprise, Profil::Particulier] {
            let liste = fichiers(profil);
            let mut sans_doublon = liste.clone();
            sans_doublon.sort_unstable();
            sans_doublon.dedup();
            assert_eq!(liste.len(), 7, "{profil:?} : {liste:?}");
            assert_eq!(sans_doublon.len(), liste.len(), "{profil:?} : {liste:?}");
        }
    }
}
