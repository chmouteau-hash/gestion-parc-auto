//! Point d'entrée : lit la configuration, applique les migrations, puis démarre le serveur ou
//! exécute une sous-commande d'administration.
//!
//! ```text
//! cargo run                                        # démarre le serveur
//! cargo run -- migrer                              # applique les migrations et s'arrête
//! cargo run -- creer-utilisateur LOGIN ROLE        # crée un compte (mot de passe lu sur l'entrée)
//! cargo run -- changer-mot-de-passe LOGIN          # remplace un mot de passe oublié
//! cargo run -- purger-conducteurs                 # RGPD : supprime les conducteurs inactifs anciens
//! cargo run -- envoyer-rappels [--afficher]        # le rappel du jour par courriel (#176)
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
  gestion-parc-auto changer-mot-de-passe LOGIN   remplace le mot de passe d'un compte (oubli) ;
                                                 le nouveau est lu sur l'entrée standard
  gestion-parc-auto purger-conducteurs          supprime les conducteurs inactifs au-delà de la
                                                 durée de conservation (RGPD)
  gestion-parc-auto envoyer-rappels [--afficher] envoie par courriel le rappel du jour (alertes,
                                                 compteurs à relever) s'il y a quelque chose à dire ;
                                                 --afficher l'écrit ici sans l'envoyer
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
        Some("changer-mot-de-passe") => changer_mot_de_passe(&pool, &arguments[1..]).await,
        Some("purger-conducteurs") => purger_conducteurs(&pool, &config).await,
        Some("envoyer-rappels") => envoyer_rappels(&pool, &config, &arguments[1..]).await,
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
    let profil = config.profil;
    let routeur = web::routeur(Etat::nouveau(pool, config)?);
    let ecoute = tokio::net::TcpListener::bind(adresse)
        .await
        .with_context(|| format!("impossible d'écouter sur {adresse}"))?;
    tracing::info!("profil : {} (réglage PROFIL)", profil.code());
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

async fn changer_mot_de_passe(pool: &PgPool, arguments: &[String]) -> anyhow::Result<()> {
    let [login] = arguments else {
        bail!("usage : changer-mot-de-passe LOGIN\n\n{AIDE}");
    };
    let mot_de_passe = lire_mot_de_passe()?;
    metier::utilisateurs::changer_mot_de_passe(pool, login, &mot_de_passe).await?;
    println!("Mot de passe du compte « {} » remplacé.", login.trim());
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

/// Le rappel du jour par courriel (#176) : composé comme la page Alertes, envoyé aux
/// destinataires de `.env`. Rien à dire : rien d'envoyé, sortie 0. `--afficher` l'écrit sur la
/// sortie standard sans l'envoyer ; il ne demande alors aucun réglage de courriel.
async fn envoyer_rappels(
    pool: &PgPool,
    config: &Config,
    arguments: &[String],
) -> anyhow::Result<()> {
    let afficher = arguments.iter().any(|a| a == "--afficher");
    if let Some(inconnu) = arguments.iter().find(|a| *a != "--afficher") {
        bail!("argument inconnu « {inconnu} »\n\n{AIDE}");
    }
    let adresse = config
        .courriel
        .as_ref()
        .map(|c| c.adresse_application.as_str())
        .unwrap_or("");
    let rappel = metier::rappels::du_jour(
        pool,
        gestion_parc_auto::commun::dates::aujourd_hui(),
        config.horizon(),
        config.profil.marque(),
        adresse,
    )
    .await?;
    let Some(rappel) = rappel else {
        println!("Rien à rappeler aujourd'hui.");
        return Ok(());
    };
    // Les comptes qui ont demandé le rappel, plus COURRIEL_DESTINATAIRES (#182).
    let destinataires = metier::rappels::destinataires(pool, config.courriel.as_ref()).await?;
    if afficher {
        println!("Sujet : {}\n\n{}", rappel.sujet, rappel.corps);
        if destinataires.is_empty() {
            println!("\nDestinataires : aucun. {CONSEIL_DESTINATAIRES}");
        } else {
            println!("\nDestinataires : {}", destinataires.join(", "));
        }
        return Ok(());
    }
    let Some(courriel) = &config.courriel else {
        bail!(
            "courriel non configuré : renseigner COURRIEL_SMTP_HOTE et les variables COURRIEL_* \
             (voir .env.example), ou lancer avec --afficher"
        );
    };
    if destinataires.is_empty() {
        bail!("aucun destinataire : {CONSEIL_DESTINATAIRES}");
    }
    metier::courriel::envoyer(courriel, &destinataires, &rappel.sujet, &rappel.corps)
        .await
        .context("envoi du rappel")?;
    tracing::info!(
        points = rappel.nombre,
        destinataires = destinataires.len(),
        "rappel envoyé"
    );
    println!(
        "Rappel envoyé à {} destinataire(s) : {}",
        destinataires.len(),
        rappel.sujet
    );
    Ok(())
}

const CONSEIL_DESTINATAIRES: &str = "cochez « Recevoir le rappel du matin » dans « Mon compte » \
                                     d'un compte qui a une adresse, ou renseignez COURRIEL_DESTINATAIRES";
