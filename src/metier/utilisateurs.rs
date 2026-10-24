//! Comptes de connexion : création et remplacement d'un mot de passe oublié (ligne de commande et
//! page « Comptes » de l'administrateur), changement de son propre mot de passe (« Mon compte »),
//! désactivation et réactivation (#172).

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

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Utilisateur> {
    utilisateurs::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Utilisateur>> {
    Ok(utilisateurs::lister(pool).await?)
}

/// Ce que le formulaire « Nouveau compte » envoie.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SaisieCompte {
    #[serde(default)]
    pub login: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub mot_de_passe: String,
    #[serde(default)]
    pub confirmation: String,
}

/// Même création qu'en ligne de commande, avec la confirmation du mot de passe en plus.
pub async fn creer_depuis_formulaire(
    pool: &PgPool,
    saisie: &SaisieCompte,
) -> Resultat<Utilisateur> {
    if saisie.mot_de_passe != saisie.confirmation {
        return Err(ErreurApp::metier(
            "La confirmation ne correspond pas au mot de passe.",
        ));
    }
    creer(pool, &saisie.login, &saisie.mot_de_passe, &saisie.role).await
}

/// Désactive un compte : il ne peut plus se connecter, sa session s'arrête à la requête
/// suivante, son identifiant reste. Deux interdits : son propre compte (on ne se ferme pas la
/// porte), et le dernier administrateur actif (plus personne ne pourrait gérer les comptes).
pub async fn desactiver(pool: &PgPool, acteur: &Utilisateur, id: i64) -> Resultat<Utilisateur> {
    let cible = obtenir(pool, id).await?;
    if cible.id == acteur.id {
        return Err(ErreurApp::metier(
            "Vous ne pouvez pas désactiver votre propre compte.",
        ));
    }
    if !cible.actif {
        return Ok(cible);
    }
    if cible.role == Role::Administrateur.code()
        && utilisateurs::compter_administrateurs_actifs(pool).await? <= 1
    {
        return Err(ErreurApp::metier(
            "Impossible de désactiver le dernier administrateur actif.",
        ));
    }
    utilisateurs::definir_actif(pool, id, false).await?;
    obtenir(pool, id).await
}

pub async fn reactiver(pool: &PgPool, id: i64) -> Resultat<Utilisateur> {
    obtenir(pool, id).await?;
    utilisateurs::definir_actif(pool, id, true).await?;
    obtenir(pool, id).await
}

/// Ce que le formulaire « Nouveau mot de passe » de l'administrateur envoie.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SaisieNouveauMotDePasse {
    #[serde(default)]
    pub nouveau: String,
    #[serde(default)]
    pub confirmation: String,
}

/// L'administrateur remplace le mot de passe d'un compte qui l'a oublié : l'ancien n'est pas
/// demandé. C'est l'équivalent de `changer-mot-de-passe` en ligne de commande.
pub async fn remplacer_mot_de_passe(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieNouveauMotDePasse,
) -> Resultat<Utilisateur> {
    let compte = obtenir(pool, id).await?;
    verifier_mot_de_passe(&saisie.nouveau)?;
    if saisie.nouveau != saisie.confirmation {
        return Err(ErreurApp::metier(
            "La confirmation ne correspond pas au nouveau mot de passe.",
        ));
    }
    let empreinte = hacher(&saisie.nouveau)?;
    utilisateurs::changer_mot_de_passe_par_id(pool, id, &empreinte).await?;
    Ok(compte)
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
