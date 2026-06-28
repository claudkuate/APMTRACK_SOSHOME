//! Rapports quotidiens automatisés au Maire (vision G-APM).
//!
//! Un scheduler léger (boucle `tokio::time`) déclenche chaque jour, à l'heure UTC
//! configurée (`DAILY_REPORT_HOUR_UTC`), la génération d'un récapitulatif par
//! commune active, envoyé au Maire (`communes.maire_email`) ou, à défaut, aux
//! administrateurs de la commune. Entièrement optionnel : sans SMTP configuré ni
//! `DAILY_REPORT_ENABLED=true`, le scheduler n'est pas démarré.

use std::time::Duration;

use chrono::{Timelike, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::modules::mailer;
use crate::state::AppState;

/// Démarre le scheduler en tâche de fond si la configuration le permet.
pub fn spawn_if_enabled(state: AppState) {
    if !state.config.daily_report_enabled {
        return;
    }
    if state.config.smtp.is_none() {
        tracing::warn!(
            "DAILY_REPORT_ENABLED=true mais SMTP non configuré — rapports quotidiens désactivés"
        );
        return;
    }
    tokio::spawn(async move { run_scheduler(state).await });
}

async fn run_scheduler(state: AppState) {
    let hour = state.config.daily_report_hour_utc;
    tracing::info!(hour_utc = hour, "scheduler rapports quotidiens démarré");
    loop {
        let wait = seconds_until_next_run(hour);
        tokio::time::sleep(Duration::from_secs(wait)).await;

        if let Err(error) = build_and_send_daily_reports(&state).await {
            tracing::warn!(%error, "échec de la génération des rapports quotidiens");
        }
        // Évite une seconde exécution dans la même heure cible.
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Nombre de secondes jusqu'à la prochaine occurrence de `target_hour:00` UTC.
fn seconds_until_next_run(target_hour: u32) -> u64 {
    let now = Utc::now();
    let today_target = now
        .with_hour(target_hour)
        .and_then(|d| d.with_minute(0))
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(now);
    let next = if today_target > now {
        today_target
    } else {
        today_target + chrono::Duration::days(1)
    };
    (next - now).num_seconds().max(1) as u64
}

async fn build_and_send_daily_reports(state: &AppState) -> anyhow::Result<()> {
    let Some(smtp) = state.config.smtp.as_ref() else {
        return Ok(());
    };

    let communes = sqlx::query(
        "SELECT id, nom, code, maire_email FROM communes \
         WHERE deleted_at IS NULL AND active = true",
    )
    .fetch_all(&state.db)
    .await?;

    let mut sent = 0_u32;
    for row in communes {
        let commune_id: Uuid = row.get("id");
        let nom: String = row.get("nom");
        let code: String = row.get("code");
        let maire_email: Option<String> = row.get("maire_email");

        let recipients = resolve_recipients(state, commune_id, maire_email).await?;
        if recipients.is_empty() {
            continue;
        }

        let body = build_report_body(state, commune_id, &nom).await?;
        let subject = format!("[G-APM] Rapport quotidien — {nom}");
        for to in &recipients {
            match mailer::send_email(smtp, to, &subject, body.clone()).await {
                Ok(()) => sent += 1,
                Err(error) => {
                    tracing::warn!(%error, commune = %code, recipient = %to, "envoi rapport échoué")
                }
            }
        }
    }

    tracing::info!(emails = sent, "rapports quotidiens envoyés");
    Ok(())
}

/// Maire en priorité, sinon les ADMIN_COMMUNE actifs de la commune.
async fn resolve_recipients(
    state: &AppState,
    commune_id: Uuid,
    maire_email: Option<String>,
) -> anyhow::Result<Vec<String>> {
    if let Some(email) = maire_email {
        let email = email.trim().to_string();
        if !email.is_empty() {
            return Ok(vec![email]);
        }
    }

    let rows = sqlx::query(
        "SELECT DISTINCT u.email \
         FROM users u \
         JOIN user_roles ur ON ur.user_id = u.id \
         JOIN roles r ON r.id = ur.role_id \
         WHERE u.commune_id = $1 AND u.active = true AND u.deleted_at IS NULL \
           AND r.code = 'ADMIN_COMMUNE'",
    )
    .bind(commune_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows.into_iter().map(|r| r.get::<String, _>("email")).collect())
}

/// Récapitulatif texte des indicateurs clés du jour pour une commune.
async fn build_report_body(
    state: &AppState,
    commune_id: Uuid,
    nom: &str,
) -> anyhow::Result<String> {
    let pvs_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pvs \
         WHERE commune_id = $1 AND deleted_at IS NULL \
           AND created_at >= date_trunc('day', now())",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let pvs_pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pvs \
         WHERE commune_id = $1 AND deleted_at IS NULL AND status = 'EN_ATTENTE_PAIEMENT'",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let signalements_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signalements \
         WHERE commune_id = $1 AND created_at >= date_trunc('day', now())",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let signalements_open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signalements \
         WHERE commune_id = $1 AND status IN ('RECU', 'EN_COURS')",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let fourrieres_active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fourrieres \
         WHERE commune_id = $1 AND deleted_at IS NULL AND status = 'EN_FOURRIERE'",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let patrouilles_active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM patrouilles \
         WHERE commune_id = $1 AND deleted_at IS NULL AND status = 'EN_COURS'",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;

    let date = Utc::now().format("%d/%m/%Y");
    Ok(format!(
        "Rapport quotidien G-APM — {nom}\n\
         Date : {date} (UTC)\n\n\
         Procès-verbaux émis aujourd'hui : {pvs_today}\n\
         PV en attente de paiement : {pvs_pending}\n\
         Signalements reçus aujourd'hui : {signalements_today}\n\
         Signalements ouverts (reçus / en cours) : {signalements_open}\n\
         Véhicules en fourrière : {fourrieres_active}\n\
         Patrouilles en cours : {patrouilles_active}\n\n\
         — Généré automatiquement par G-APM (Gestion des Activités de Police Municipale).\n"
    ))
}
