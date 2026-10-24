//! « Comptes » : l'administrateur crée, désactive, réactive et dépanne les comptes (#172, bloc 2).
//!
//! Pas de suppression : un compte désactivé ne peut plus se connecter, sa session en cours
//! s'arrête à la requête suivante, mais son identifiant reste pour l'historique.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use sqlx::PgPool;

use crate::commun::types::{Choix, Role};
use crate::donnees::utilisateurs::Utilisateur;
use crate::erreurs::{ErreurApp, Resultat};
use crate::etat::Etat;
use crate::metier::utilisateurs::{self, SaisieCompte, SaisieNouveauMotDePasse};
use crate::web::chemin::Identifiant;
use crate::web::fait::{self, Fait};
use crate::web::rendu::{rendre, Contexte};
use crate::web::session::Administrateur;

pub fn routes() -> Router<Etat> {
    Router::new()
        .route("/comptes", get(liste).post(creer))
        .route("/comptes/nouveau", get(nouveau))
        .route("/comptes/{id}/desactiver", post(desactiver))
        .route("/comptes/{id}/reactiver", post(reactiver))
        .route(
            "/comptes/{id}/mot-de-passe",
            get(formulaire_mot_de_passe).post(remplacer_mot_de_passe),
        )
}

/// Une ligne de la liste.
struct LigneCompte {
    u: Utilisateur,
    /// Le compte de l'administrateur connecté : il ne peut pas se désactiver lui-même.
    est_moi: bool,
    role_libelle: &'static str,
    cree_le: String,
}

#[derive(Template)]
#[template(path = "comptes/liste.html")]
struct PageListe {
    ctx: Contexte,
    comptes: Vec<LigneCompte>,
    erreur: Option<String>,
}

async fn page_liste(
    pool: &PgPool,
    admin: &Administrateur,
    erreur: Option<String>,
) -> Resultat<PageListe> {
    let comptes = utilisateurs::lister(pool)
        .await?
        .into_iter()
        .map(|u| LigneCompte {
            est_moi: u.id == admin.utilisateur.id,
            role_libelle: Role::depuis_code(&u.role).map_or("", |r| r.libelle()),
            cree_le: u
                .cree_le
                .with_timezone(&chrono::Local)
                .format("%d/%m/%Y")
                .to_string(),
            u,
        })
        .collect();
    Ok(PageListe {
        ctx: admin.contexte("comptes"),
        comptes,
        erreur,
    })
}

async fn liste(admin: Administrateur, State(etat): State<Etat>) -> Resultat<Html<String>> {
    rendre(page_liste(&etat.pool, &admin, None).await?)
}

#[derive(Template)]
#[template(path = "comptes/formulaire.html")]
struct PageFormulaire {
    ctx: Contexte,
    saisie: SaisieCompte,
    roles: Vec<Choix>,
    erreur: Option<String>,
}

fn page_formulaire(
    admin: &Administrateur,
    saisie: SaisieCompte,
    erreur: Option<String>,
) -> PageFormulaire {
    PageFormulaire {
        ctx: admin.contexte("comptes"),
        saisie,
        roles: Role::choix(),
        erreur,
    }
}

async fn nouveau(admin: Administrateur) -> Resultat<Html<String>> {
    let saisie = SaisieCompte {
        role: Role::Lecteur.code().to_string(),
        ..SaisieCompte::default()
    };
    rendre(page_formulaire(&admin, saisie, None))
}

async fn creer(
    admin: Administrateur,
    State(etat): State<Etat>,
    Form(saisie): Form<SaisieCompte>,
) -> Resultat<Response> {
    match utilisateurs::creer_depuis_formulaire(&etat.pool, &saisie).await {
        Ok(compte) => {
            tracing::info!(login = %compte.login, par = %admin.utilisateur.login, "compte créé");
            Ok(fait::rediriger("/comptes", Fait::CompteCree))
        }
        Err(ErreurApp::Metier(message)) => {
            // Le mot de passe saisi ne revient jamais dans la page.
            let saisie = SaisieCompte {
                mot_de_passe: String::new(),
                confirmation: String::new(),
                ..saisie
            };
            Ok(rendre(page_formulaire(&admin, saisie, Some(message)))?.into_response())
        }
        Err(autre) => Err(autre),
    }
}

/// Un refus (son propre compte, le dernier administrateur) revient sur la liste avec son message.
async fn desactiver(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    match utilisateurs::desactiver(&etat.pool, &admin.utilisateur, id).await {
        Ok(compte) => {
            tracing::info!(login = %compte.login, par = %admin.utilisateur.login, "compte désactivé");
            Ok(fait::rediriger("/comptes", Fait::CompteDesactive))
        }
        Err(ErreurApp::Metier(message)) => {
            Ok(rendre(page_liste(&etat.pool, &admin, Some(message)).await?)?.into_response())
        }
        Err(autre) => Err(autre),
    }
}

async fn reactiver(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Response> {
    let compte = utilisateurs::reactiver(&etat.pool, id).await?;
    tracing::info!(login = %compte.login, par = %admin.utilisateur.login, "compte réactivé");
    Ok(fait::rediriger("/comptes", Fait::CompteReactive))
}

#[derive(Template)]
#[template(path = "comptes/mot_de_passe.html")]
struct PageMotDePasse {
    ctx: Contexte,
    compte: Utilisateur,
    erreur: Option<String>,
}

async fn formulaire_mot_de_passe(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
) -> Resultat<Html<String>> {
    let compte = utilisateurs::obtenir(&etat.pool, id).await?;
    rendre(PageMotDePasse {
        ctx: admin.contexte("comptes"),
        compte,
        erreur: None,
    })
}

async fn remplacer_mot_de_passe(
    admin: Administrateur,
    State(etat): State<Etat>,
    Identifiant(id): Identifiant,
    Form(saisie): Form<SaisieNouveauMotDePasse>,
) -> Resultat<Response> {
    match utilisateurs::remplacer_mot_de_passe(&etat.pool, id, &saisie).await {
        Ok(compte) => {
            tracing::info!(login = %compte.login, par = %admin.utilisateur.login, "mot de passe remplacé");
            Ok(fait::rediriger("/comptes", Fait::MotDePasseRemplace))
        }
        Err(ErreurApp::Metier(message)) => {
            let compte = utilisateurs::obtenir(&etat.pool, id).await?;
            Ok(rendre(PageMotDePasse {
                ctx: admin.contexte("comptes"),
                compte,
                erreur: Some(message),
            })?
            .into_response())
        }
        Err(autre) => Err(autre),
    }
}
