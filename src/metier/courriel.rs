//! L'envoi d'un courriel par SMTP (#176), avec les réglages de `.env` (`config::Courriel`).
//!
//! Un seul usage pour l'instant, le rappel quotidien ; le texte est brut, sans pièce jointe. La
//! bibliothèque `lettre` fait le SMTP sur tokio avec rustls : rien de natif à installer.

use anyhow::Context;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{Courriel, Securite};

/// Envoie `corps` (texte brut) aux `destinataires` (`metier::rappels::destinataires`). Une
/// erreur dit ce qui a manqué (adresse invalide, connexion refusée, authentification) sans jamais
/// citer le mot de passe.
pub async fn envoyer(
    config: &Courriel,
    destinataires: &[String],
    sujet: &str,
    corps: &str,
) -> anyhow::Result<()> {
    let mut message = Message::builder()
        .from(
            config
                .expediteur
                .parse()
                .with_context(|| format!("COURRIEL_EXPEDITEUR invalide : {}", config.expediteur))?,
        )
        .subject(sujet)
        .header(ContentType::TEXT_PLAIN);
    for destinataire in destinataires {
        message = message.to(destinataire
            .parse()
            .with_context(|| format!("destinataire : adresse invalide « {destinataire} »"))?);
    }
    let message = message
        .body(corps.to_string())
        .context("construction du courriel")?;

    let transport = match config.securite {
        Securite::Aucune => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.hote),
        Securite::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.hote)
            .with_context(|| format!("serveur SMTP {} (STARTTLS)", config.hote))?,
        Securite::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.hote)
            .with_context(|| format!("serveur SMTP {} (TLS)", config.hote))?,
    }
    .port(config.port);
    let transport = match (&config.identifiant, &config.mot_de_passe) {
        (Some(identifiant), Some(mot_de_passe)) => {
            transport.credentials(Credentials::new(identifiant.clone(), mot_de_passe.clone()))
        }
        _ => transport,
    };
    transport
        .build()
        .send(message)
        .await
        .with_context(|| format!("envoi par {}:{}", config.hote, config.port))?;
    Ok(())
}
