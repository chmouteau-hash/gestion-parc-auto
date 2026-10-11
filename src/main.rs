//! Point d'entrée : lit la configuration, applique les migrations, puis démarre le serveur ou
//! exécute une sous-commande d'administration.
//!
//! ```text
//! cargo run                                        # démarre le serveur
//! cargo run -- migrer                              # applique les migrations et s'arrête
//! cargo run -- creer-utilisateur LOGIN ROLE        # crée un compte (mot de passe lu sur l'entrée)
//! cargo run -- purger-conducteurs                  # RGPD : supprime les conducteurs inactifs anciens
//! ```

use std::io::{IsTerminal, Write};
use std::net::SocketAddr;

use anyhow::{bail, Context};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing_subscriber::EnvFilter;

use gestion_parc_auto::config::Config;
use gestion_parc_auto::etat::Etat;
use gestion_parc_auto::{metier, migrer, web};

const AIDE: &str = "\
Usage :
  gestion-parc-auto                              démarre le serveur web
  gestion-parc-auto migrer                       applique les migrations et s'arrête
  gestion-parc-auto creer-utilisateur LOGIN ROLE crée un compte ; ROLE = administrateur | lecteur
                                                 (le mot de passe est lu sur l'entrée standard)
  gestion-parc-auto purger-conducteurs           supprime les conducteurs inactifs au-delà de la
                                                 durée de conservation (RGPD)
  gestion-parc-auto aide                         affiche ce message

Configuration par variables d'environnement : voir .env.example.";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `.env` est lu s'il existe (développement) ; en production, l'environnement suffit.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let commande = arguments.first().map(String::as_str);
    if matches!(commande, Some("aide" | "--help" | "-h")) {
        println!("{AIDE}");
        return Ok(());
    }

    let config = Config::depuis_environnement().context("configuration invalide")?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connexion à PostgreSQL impossible (vérifier DATABASE_URL)")?;
    migrer(&pool).await.context("application des migrations")?;

    match commande {
        None => servir(pool, config).await,
        Some("migrer") => {
            println!("Migrations appliquées.");
            Ok(())
        }
        Some("creer-utilisateur") => creer_utilisateur(&pool, &arguments[1..]).await,
        Some("purger-conducteurs") => purger_conducteurs(&pool, &config).await,
        Some(autre) => bail!("commande inconnue « {autre} »\n\n{AIDE}"),
    }
}

async fn servir(pool: PgPool, config: Config) -> anyhow::Result<()> {
    if metier::utilisateurs::compter(&pool).await? == 0 {
        tracing::warn!(
            "aucun compte : créez-en un avec `cargo run -- creer-utilisateur admin administrateur`"
        );
    }
    let adresse = SocketAddr::from(([127, 0, 0, 1], config.port));
    let routeur = web::routeur(Etat::nouveau(pool, config)?);
    let ecoute = tokio::net::TcpListener::bind(adresse)
        .await
        .with_context(|| format!("impossible d'écouter sur {adresse}"))?;
    tracing::info!("serveur prêt sur http://{adresse}");
    axum::serve(ecoute, routeur)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("arrêt demandé");
        })
        .await?;
    Ok(())
}

async fn creer_utilisateur(pool: &PgPool, arguments: &[String]) -> anyhow::Result<()> {
    let [login, role] = arguments else {
        bail!("usage : creer-utilisateur LOGIN ROLE\n\n{AIDE}");
    };
    let mot_de_passe = lire_mot_de_passe()?;
    let utilisateur = metier::utilisateurs::creer(pool, login, &mot_de_passe, role).await?;
    println!(
        "Compte « {} » créé avec le rôle {}.",
        utilisateur.login, utilisateur.role
    );
    Ok(())
}

/// Lit le mot de passe sur l'entrée standard : interactivement (invite affichée) ou depuis un
/// tube (`echo mdp | ...`). Il n'est jamais passé en argument de commande, où il resterait dans
/// l'historique du terminal.
fn lire_mot_de_passe() -> anyhow::Result<String> {
    let entree = std::io::stdin();
    if entree.is_terminal() {
        eprint!("Mot de passe (affiché en clair) : ");
        std::io::stderr().flush()?;
    }
    let mut ligne = String::new();
    entree.read_line(&mut ligne)?;
    Ok(ligne.trim_end_matches(['\r', '\n']).to_string())
}

async fn purger_conducteurs(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    let supprimes = metier::conducteurs::purger_inactifs(
        pool,
        &config.documents_dir,
        chrono::Utc::now(),
        config.conservation_conducteurs_inactifs_jours,
    )
    .await?;
    println!(
        "{supprimes} conducteur(s) inactif(s) depuis plus de {} jours supprimé(s).",
        config.conservation_conducteurs_inactifs_jours
    );
    Ok(())
}
