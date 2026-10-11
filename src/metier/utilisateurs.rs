//! Création des comptes de connexion (ligne de commande uniquement en v1).

use sqlx::PgPool;

use crate::commun::types::Role;
use crate::donnees::utilisateurs::{self, Utilisateur};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_UNICITE};
use crate::metier::authentification::{hacher, LONGUEUR_MIN_MOT_DE_PASSE};
use crate::metier::validation::{code_parmi, texte_obligatoire};

pub async fn creer(
    pool: &PgPool,
    login: &str,
    mot_de_passe: &str,
    role: &str,
) -> Resultat<Utilisateur> {
    let login = texte_obligatoire(login, "identifiant")?;
    if login.chars().count() < 3 || login.chars().any(char::is_whitespace) {
        return Err(ErreurApp::metier(
            "L'identifiant doit faire au moins 3 caractères, sans espace.",
        ));
    }
    if mot_de_passe.chars().count() < LONGUEUR_MIN_MOT_DE_PASSE {
        return Err(ErreurApp::metier(format!(
            "Le mot de passe doit faire au moins {LONGUEUR_MIN_MOT_DE_PASSE} caractères."
        )));
    }
    let role = code_parmi(role, "rôle", Role::depuis_code)?;
    let empreinte = hacher(mot_de_passe)?;

    utilisateurs::inserer(pool, &login, &empreinte, role.code())
        .await
        .map_err(|e| {
            if a_le_code(&e, CODE_VIOLATION_UNICITE) {
                ErreurApp::metier(format!("L'identifiant « {login} » existe déjà."))
            } else {
                e.into()
            }
        })
}

pub async fn compter(pool: &PgPool) -> Resultat<i64> {
    Ok(utilisateurs::compter(pool).await?)
}
