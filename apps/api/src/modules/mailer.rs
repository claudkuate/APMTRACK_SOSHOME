//! Envoi d'e-mails sortants (rapports quotidiens au Maire) via SMTP.
//!
//! Optionnel : activé seulement si `SmtpConfig` est présent (variables `SMTP_*`).
//! Utilise `lettre` en mode async (Tokio) + STARTTLS rustls.

use anyhow::Context;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;

/// Envoie un e-mail texte. Renvoie une erreur en cas d'échec (le scheduler la log).
pub async fn send_email(
    smtp: &SmtpConfig,
    to: &str,
    subject: &str,
    body: String,
) -> anyhow::Result<()> {
    let email = Message::builder()
        .from(smtp.from.parse().context("SMTP_FROM invalide")?)
        .to(to
            .parse()
            .with_context(|| format!("destinataire invalide: {to}"))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .context("construction du message e-mail")?;

    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
        .context("initialisation du transport SMTP")?
        .port(smtp.port);
    if let (Some(username), Some(password)) = (smtp.username.clone(), smtp.password.clone()) {
        builder = builder.credentials(Credentials::new(username, password));
    }

    builder
        .build()
        .send(email)
        .await
        .context("envoi SMTP")?;
    Ok(())
}
