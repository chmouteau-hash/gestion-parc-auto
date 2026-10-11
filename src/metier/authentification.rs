//! Hachage des mots de passe et vérification à la connexion.
//!
//! Argon2id est volontairement lent (quelques dizaines de millisecondes) : c'est ce qui rend une
//! attaque par essais massifs impraticable. Le calcul se fait dans `spawn_blocking` pour ne pas
//! bloquer le serveur pendant ce temps.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sqlx::PgPool;

use crate::donnees::utilisateurs::{self, Utilisateur};
use crate::erreurs::{ErreurApp, Resultat};

pub const LONGUEUR_MIN_MOT_DE_PASSE: usize = 12;

/// Empreinte « PHC string » (`$argon2id$v=19$m=...$sel$empreinte`) : contient tout ce qu'il faut
/// pour vérifier plus tard, y compris le sel aléatoire.
pub fn hacher(mot_de_passe: &str) -> anyhow::Result<String> {
    let sel = SaltString::generate(&mut OsRng);
    let empreinte = Argon2::default()
        .hash_password(mot_de_passe.as_bytes(), &sel)
        .map_err(|e| anyhow::anyhow!("hachage du mot de passe : {e}"))?;
    Ok(empreinte.to_string())
}

/// `true` si le mot de passe correspond à l'empreinte. Une empreinte illisible vaut `false`.
pub fn verifier(empreinte: &str, mot_de_passe: &str) -> bool {
    match PasswordHash::new(empreinte) {
        Ok(hash) => Argon2::default()
            .verify_password(mot_de_passe.as_bytes(), &hash)
            .is_ok(),
        Err(_) => false,
    }
}

/// Vérifie un couple identifiant / mot de passe. Le message d'erreur est le même que
/// l'identifiant existe ou non, pour ne pas révéler la liste des comptes.
pub async fn connecter(pool: &PgPool, login: &str, mot_de_passe: &str) -> Resultat<Utilisateur> {
    let refus = || ErreurApp::metier("Identifiant ou mot de passe incorrect.");
    let utilisateur = utilisateurs::obtenir_par_login(pool, login.trim())
        .await?
        .ok_or_else(refus)?;
    let empreinte = utilisateur.mot_de_passe_hash.clone();
    let mot_de_passe = mot_de_passe.to_string();
    let valide = tokio::task::spawn_blocking(move || verifier(&empreinte, &mot_de_passe))
        .await
        .map_err(|e| anyhow::anyhow!("tâche de vérification interrompue : {e}"))?;
    if valide {
        Ok(utilisateur)
    } else {
        Err(refus())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hachage_et_verification() {
        let empreinte = hacher("un mot de passe solide").unwrap();
        assert!(empreinte.starts_with("$argon2id$"));
        assert!(verifier(&empreinte, "un mot de passe solide"));
        assert!(!verifier(&empreinte, "un autre"));
        assert!(!verifier("pas une empreinte", "un mot de passe solide"));
    }

    #[test]
    fn deux_hachages_different() {
        // Le sel est aléatoire : la même entrée ne donne jamais deux fois la même empreinte.
        assert_ne!(hacher("abc").unwrap(), hacher("abc").unwrap());
    }
}
