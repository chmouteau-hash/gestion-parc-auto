//! Le journal des écritures (#60) : une ligne par écriture réussie, avec qui l'a faite.
//!
//! Un seul point d'écriture, cet intergiciel, plutôt qu'une ligne copiée dans chaque
//! gestionnaire : le format est le même partout et un nouveau gestionnaire est couvert d'office.
//! La ligne porte l'identifiant de connexion, la méthode, le chemin et le code de réponse :
//! le chemin dit le type d'objet, son numéro et le geste (`POST /vehicules/12/supprimer`,
//! `PUT /api/v1/conducteurs/3`). Jamais de nom, d'immatriculation ni de contenu : le journal sort
//! dans un terminal et peut être copié.
//!
//! Une écriture **refusée** n'est pas une écriture : un formulaire qui revient avec son message
//! répond `200`, une écriture réussie répond par une redirection ; l'API répond `2xx` quand elle
//! a écrit. La connexion et la déconnexion ont déjà leur ligne (`web/connexion.rs`).

use axum::extract::FromRequestParts;
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::{Key, PrivateCookieJar};

use crate::etat::Etat;
use crate::metier::utilisateurs;
use crate::web::session::{lire_valeur_cookie, COOKIE_SESSION};

pub async fn ecritures(State(etat): State<Etat>, requete: Request, suite: Next) -> Response {
    let methode = requete.method().clone();
    let chemin = requete.uri().path().to_string();
    let concerne = methode != Method::GET
        && methode != Method::HEAD
        && methode != Method::OPTIONS
        && chemin != "/connexion"
        && chemin != "/deconnexion";
    if !concerne {
        return suite.run(requete).await;
    }

    // Qui agit : l'identifiant du cookie de session, relu comme le fait `session.rs`.
    let (mut parties, corps) = requete.into_parts();
    let jar = PrivateCookieJar::<Key>::from_request_parts(&mut parties, &etat)
        .await
        .unwrap_or_else(|impossible| match impossible {});
    let utilisateur_id = jar
        .get(COOKIE_SESSION)
        .and_then(|c| lire_valeur_cookie(c.value(), chrono::Utc::now().timestamp()));
    let requete = Request::from_parts(parties, corps);

    let reponse = suite.run(requete).await;

    let statut = reponse.status();
    let reussie = if chemin.starts_with("/api/") {
        statut.is_success()
    } else {
        statut.is_redirection()
    };
    if let (true, Some(id)) = (reussie, utilisateur_id) {
        let login = match utilisateurs::obtenir(&etat.pool, id).await {
            Ok(u) => u.login,
            Err(_) => format!("#{id}"),
        };
        tracing::info!(
            login = %login,
            methode = %methode,
            chemin = %chemin,
            statut = statut.as_u16(),
            "écriture"
        );
    }
    reponse
}
