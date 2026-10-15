//! Couche HTTP : routeur, intergiciels, et une page par entité.
//!
//! Garanties de cette couche :
//! - aucune requête SQL : tout passe par `metier` ;
//! - toute route qui écrit (`POST`, `PUT`, `DELETE`) prend un [`session::Administrateur`] en
//!   paramètre ; toute route qui lit prend un [`session::UtilisateurConnecte`]. Les seules routes
//!   anonymes sont la page de connexion et les fichiers statiques ;
//! - les en-têtes de sécurité sont posés sur toutes les réponses par [`entetes_securite`].

pub mod api;
pub mod chemin;
pub mod fait;
pub mod rendu;
pub mod session;

mod accueil;
mod affectations;
mod alertes;
mod conducteurs;
mod connexion;
mod documents;
mod echeances;
mod export;
mod interventions;
mod navigation;
mod vehicules;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::erreurs::ErreurApp;
use crate::etat::Etat;
use crate::metier::documents::TAILLE_MAX_OCTETS;

/// Feuille de style et script, embarqués dans l'exécutable : pas de dossier à déployer à côté.
const STYLE: &str = include_str!("../../static/style.css");
const SCRIPT: &str = include_str!("../../static/app.js");
/// Les jetons de la charte (couleurs, espacements, angles), lus tels quels dans `design-system/` :
/// la feuille de style les emploie sans les recopier, donc sans pouvoir s'en écarter.
const JETONS: &str = include_str!("../../design-system/autoclair/code/tokens.css");

/// Un fichier embarqué qui ne change pas d'un jour à l'autre : police ou logo.
struct FichierStatique {
    chemin: &'static str,
    type_mime: &'static str,
    octets: &'static [u8],
}

/// Polices (`static/polices/`) et logos (`design-system/autoclair/logos/svg/`). La liste est
/// fermée : seuls ces chemins se servent, aucun nom n'est lu dans l'adresse demandée.
///
/// Les logos sont ceux de la charte, jamais recomposés : le grand pour l'en-tête, l'icône
/// simplifiée (sans pointillés, lisible à 16 px) pour l'onglet du navigateur.
const FICHIERS_STATIQUES: &[FichierStatique] = &[
    FichierStatique {
        chemin: "/static/polices/montserrat.woff2",
        type_mime: "font/woff2",
        octets: include_bytes!("../../static/polices/montserrat.woff2"),
    },
    FichierStatique {
        chemin: "/static/polices/instrument-sans.woff2",
        type_mime: "font/woff2",
        octets: include_bytes!("../../static/polices/instrument-sans.woff2"),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!("../../design-system/autoclair/logos/svg/autoclair-principale.svg"),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-pro.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/svg/autoclair-pro-principale.svg"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-negatif.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/svg/autoclair-principale-negatif.svg"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-pro-negatif.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/svg/autoclair-pro-principale-negatif.svg"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/icone-autoclair.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/svg/autoclair-symbole-simple.svg"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/icone-autoclair-pro.svg",
        type_mime: "image/svg+xml",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/svg/autoclair-pro-symbole-simple.svg"
        ),
    },
    // Icônes d'application (écran d'accueil du téléphone) : les exports PNG de la charte, avec
    // pointillés, aux tailles qu'elle livre (512 et 192 px pour Android, 180 px pour iPhone).
    FichierStatique {
        chemin: "/static/logos/autoclair-512.png",
        type_mime: "image/png",
        octets: include_bytes!("../../design-system/autoclair/logos/png/autoclair-symbole-512.png"),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-192.png",
        type_mime: "image/png",
        octets: include_bytes!("../../design-system/autoclair/logos/png/autoclair-symbole-192.png"),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-180.png",
        type_mime: "image/png",
        octets: include_bytes!("../../design-system/autoclair/logos/png/autoclair-symbole-180.png"),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-pro-512.png",
        type_mime: "image/png",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/png/autoclair-pro-symbole-512.png"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-pro-192.png",
        type_mime: "image/png",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/png/autoclair-pro-symbole-192.png"
        ),
    },
    FichierStatique {
        chemin: "/static/logos/autoclair-pro-180.png",
        type_mime: "image/png",
        octets: include_bytes!(
            "../../design-system/autoclair/logos/png/autoclair-pro-symbole-180.png"
        ),
    },
];

/// La valeur d'un jeton de la charte (`--marine-700` → `#062966`), lue dans `tokens.css` plutôt
/// que recopiée : la règle « aucune couleur écrite hors des jetons » vaut aussi pour ce que
/// l'application dit au téléphone (couleur de thème du manifest).
///
/// Les jetons sont dans l'exécutable : un nom absent est une erreur de programmation, que les
/// tests attrapent ; d'où l'arrêt plutôt qu'une valeur de repli.
pub fn jeton(nom: &str) -> &'static str {
    let motif = format!("{nom}:");
    JETONS
        .lines()
        .map(str::trim)
        .find_map(|ligne| ligne.strip_prefix(motif.as_str()))
        .map(|reste| reste.trim().trim_end_matches(';').trim())
        .unwrap_or_else(|| panic!("jeton {nom} absent de tokens.css"))
}

pub fn routeur(etat: Etat) -> Router {
    let fichiers = FICHIERS_STATIQUES
        .iter()
        .fold(Router::new(), |routeur, fichier| {
            routeur.route(fichier.chemin, get(move || async move { servir(fichier) }))
        });

    Router::new()
        .merge(accueil::routes())
        .route("/static/style.css", get(style))
        .route("/static/tokens.css", get(jetons))
        .route("/static/app.js", get(script))
        .route("/manifest.webmanifest", get(manifeste))
        .merge(fichiers)
        .merge(connexion::routes())
        .merge(alertes::routes())
        .merge(vehicules::routes())
        .merge(conducteurs::routes())
        .merge(affectations::routes())
        .merge(interventions::routes())
        .merge(echeances::routes())
        .merge(documents::routes())
        .merge(navigation::routes())
        .merge(export::routes())
        .nest("/api/v1", api::routes())
        .fallback(introuvable)
        .layer(middleware::from_fn(entetes_securite))
        // Un peu plus que la taille maximale d'un document : l'enveloppe multipart compte aussi.
        .layer(DefaultBodyLimit::max(TAILLE_MAX_OCTETS + 64 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(etat)
}

async fn introuvable() -> ErreurApp {
    ErreurApp::Introuvable
}

async fn style() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE,
    )
        .into_response()
}

async fn jetons() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        JETONS,
    )
        .into_response()
}

/// Un jour de cache : ces fichiers pèsent ensemble une centaine de Ko, et sans cela le
/// navigateur les redemanderait à chaque page.
fn servir(fichier: &'static FichierStatique) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, fichier.type_mime),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        fichier.octets,
    )
        .into_response()
}

/// Le manifest d'application web de la marque servie : ce que le téléphone lit pour installer
/// l'application sur son écran d'accueil. Anonyme, comme les logos : le navigateur le demande
/// avant toute connexion. Même durée de cache que les autres fichiers de la charte.
async fn manifeste(State(etat): State<Etat>) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/manifest+json"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        etat.config.profil.manifeste().to_string(),
    )
        .into_response()
}

async fn script() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        SCRIPT,
    )
        .into_response()
}

/// En-têtes de sécurité standards, sur toutes les réponses.
///
/// - `Content-Security-Policy` : le navigateur ne charge style et script que depuis ce site, et
///   refuse tout script en ligne (ce qui neutralise la plupart des injections HTML) ;
/// - `X-Content-Type-Options: nosniff` : un fichier déposé n'est jamais « deviné » comme HTML ;
/// - `X-Frame-Options: DENY` : la page ne peut pas être incluse dans un site tiers (détournement
///   de clic) ;
/// - `Referrer-Policy` : l'adresse des pages internes ne fuit pas vers d'autres sites.
///
/// Et, quand la requête apportait un message d'action (cookie `fait`, voir [`fait`]), la page
/// HTML qui l'affiche l'efface : il ne se montre qu'une fois.
async fn entetes_securite(requete: Request, suite: Next) -> Response {
    let message_apporte = fait::lire(requete.headers()).is_some();
    let mut reponse = suite.run(requete).await;
    let page_html = reponse
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if message_apporte && reponse.status().is_success() && page_html {
        reponse
            .headers_mut()
            .append(header::SET_COOKIE, fait::entete_suppression());
    }
    let entetes = reponse.headers_mut();
    entetes.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; frame-ancestors 'none'; form-action 'self'",
        ),
    );
    entetes.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    entetes.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    entetes.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    reponse
}
