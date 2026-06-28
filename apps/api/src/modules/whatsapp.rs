//! Envoi de messages WhatsApp sortants via l'API Meta Cloud (Graph API).
//!
//! Optionnel : activé seulement si `WhatsAppConfig` est présent (variables
//! `WHATSAPP_*`). Sert à transmettre au plaignant son numéro de suivi après
//! le dépôt d'un signalement.

use anyhow::{bail, Context};
use serde_json::json;

use crate::config::WhatsAppConfig;

/// Indicatif pays par défaut (Cameroun) appliqué aux numéros locaux.
const DEFAULT_COUNTRY_CODE: &str = "237";

/// Envoie un message texte WhatsApp. Renvoie une erreur en cas d'échec —
/// l'appelant la log sans interrompre le flux principal.
pub async fn send_text(cfg: &WhatsAppConfig, to: &str, body: &str) -> anyhow::Result<()> {
    let recipient = normalize_phone(to).with_context(|| format!("numéro WhatsApp invalide: {to}"))?;
    let url = format!(
        "{}/{}/messages",
        cfg.api_base_url.trim_end_matches('/'),
        cfg.phone_number_id
    );

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(&cfg.access_token)
        .json(&json!({
            "messaging_product": "whatsapp",
            "to": recipient,
            "type": "text",
            "text": { "body": body },
        }))
        .send()
        .await
        .context("appel API WhatsApp")?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        bail!("API WhatsApp a renvoyé {status}: {detail}");
    }
    Ok(())
}

/// Normalise un numéro au format E.164 sans « + » (attendu par l'API Meta) :
/// retire les caractères non numériques, gère le préfixe international « 00 »
/// et applique l'indicatif pays par défaut aux numéros locaux.
fn normalize_phone(raw: &str) -> anyhow::Result<String> {
    let mut digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if let Some(rest) = digits.strip_prefix("00") {
        digits = rest.to_string();
    }
    if digits.len() < 8 {
        bail!("numéro trop court");
    }
    if !digits.starts_with(DEFAULT_COUNTRY_CODE) && digits.len() <= 9 {
        digits = format!("{DEFAULT_COUNTRY_CODE}{digits}");
    }
    Ok(digits)
}
