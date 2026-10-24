//! Comptes de connexion : création et remplacement d'un mot de passe oublié (ligne de commande),
//! changement de son propre mot de passe (page « Mon compte », #172).

use serde::Deserialize;
use sqlx::PgPool;

use crate::commun::types::Role;
use crate::donnees::utilisateurs::{self, Utilisateur};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_UNICITE};
use crate::metier::authentification::{hacher, verifier, LONGUEUR_MIN_MOT_DE_PASSE};
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
    verifier_mot_de_passe(mot_de_passe)?;
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

/// Remplace le mot de passe d'un compte existant : c'est la réponse à un mot de passe oublié.
///
/// L'ancien mot de passe n'est pas demandé, et ne peut pas l'être : la base n'en garde qu'une
/// empreinte. La commande est donc réservée à qui a la main sur la machine du serveur. Une
/// session déjà ouverte avec l'ancien mot de passe reste valable jusqu'à son expiration.
pub async fn changer_mot_de_passe(pool: &PgPool, login: &str, mot_de_passe: &str) -> Resultat<()> {
    let login = login.trim();
    verifier_mot_de_passe(mot_de_passe)?;
    let empreinte = hacher(mot_de_passe)?;
    if utilisateurs::changer_mot_de_passe(pool, login, &empreinte).await? {
        Ok(())
    } else {
        Err(ErreurApp::metier(format!(
            "Aucun compte ne porte l'identifiant « {login} »."
        )))
    }
}

/// Ce que le formulaire « Mon compte » envoie.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SaisieMotDePasse {
    #[serde(default)]
    pub actuel: String,
    #[serde(default)]
    pub nouveau: String,
    #[serde(default)]
    pub confirmation: String,
}

/// Change le mot de passe du compte connecté, qui donne l'actuel : c'est ce qui autorise un
/// lecteur à écrire ici, et seulement ici. L'actuel se vérifie comme à la connexion (Argon2id,
/// dans un fil bloquant). Les sessions déjà ouvertes restent valables jusqu'à leur expiration.
pub async fn changer_son_mot_de_passe(
    pool: &PgPool,
    utilisateur: &Utilisateur,
    saisie: &SaisieMotDePasse,
) -> Resultat<()> {
    let empreinte = utilisateur.mot_de_passe_hash.clone();
    let actuel = saisie.actuel.clone();
    let valide = tokio::task::spawn_blocking(move || verifier(&empreinte, &actuel))
        .await
        .map_err(|e| anyhow::anyhow!("tâche de vérification interrompue : {e}"))?;
    if !valide {
        return Err(ErreurApp::metier("Le mot de passe actuel est incorrect."));
    }
    verifier_mot_de_passe(&saisie.nouveau)?;
    if saisie.nouveau != saisie.confirmation {
        return Err(ErreurApp::metier(
            "La confirmation ne correspond pas au nouveau mot de passe.",
        ));
    }
    if saisie.nouveau == saisie.actuel {
        return Err(ErreurApp::metier(
            "Le nouveau mot de passe doit être différent de l'actuel.",
        ));
    }
    let empreinte = hacher(&saisie.nouveau)?;
    if utilisateurs::changer_mot_de_passe(pool, &utilisateur.login, &empreinte).await? {
        Ok(())
    } else {
        Err(ErreurApp::metier("Ce compte n'existe plus."))
    }
}

fn verifier_mot_de_passe(mot_de_passe: &str) -> Resultat<()> {
    if mot_de_passe.chars().count() < LONGUEUR_MIN_MOT_DE_PASSE {
        return Err(ErreurApp::metier(format!(
            "Le mot de passe doit faire au moins {LONGUEUR_MIN_MOT_DE_PASSE} caractères."
        )));
    }
    Ok(())
}

pub async fn compter(pool: &PgPool) -> Resultat<i64> {
    Ok(utilisateurs::compter(pool).await?)
}
