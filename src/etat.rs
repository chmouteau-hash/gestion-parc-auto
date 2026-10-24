//! État partagé par toutes les requêtes : pool de connexions, configuration, clé de session.
//!
//! Axum clone cet état pour chaque requête ; il doit donc être bon marché à cloner. `PgPool` et
//! `Key` sont déjà des poignées partagées, et la configuration est derrière un `Arc`.

use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use sqlx::PgPool;

use crate::config::Config;
use crate::metier::frein::Frein;

#[derive(Clone)]
pub struct Etat {
    pub pool: PgPool,
    pub config: Arc<Config>,
    /// Le frein aux essais de mot de passe, partagé par toutes les requêtes.
    pub frein: Arc<Frein>,
    cle_session: Key,
}

impl Etat {
    pub fn nouveau(pool: PgPool, config: Config) -> anyhow::Result<Etat> {
        // La clé de chiffrement et de signature du cookie est construite directement à partir du
        // secret, qui doit faire au moins 64 octets (vérifié dans `Config`, revérifié ici).
        let cle_session = Key::try_from(config.session_secret.as_bytes())
            .map_err(|e| anyhow::anyhow!("SESSION_SECRET inutilisable : {e}"))?;
        Ok(Etat {
            pool,
            config: Arc::new(config),
            frein: Arc::new(Frein::new()),
            cle_session,
        })
    }
}

/// Permet à `PrivateCookieJar` de retrouver la clé de chiffrement depuis l'état.
impl FromRef<Etat> for Key {
    fn from_ref(etat: &Etat) -> Key {
        etat.cle_session.clone()
    }
}
