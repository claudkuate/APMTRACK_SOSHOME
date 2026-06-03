use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn record(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
    ip_address: Option<String>,
    user_agent: Option<String>,
) {
    record_for_commune(
        pool,
        None,
        user_id,
        action,
        entity_type,
        entity_id,
        old_value,
        new_value,
        ip_address,
        user_agent,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
pub async fn record_for_commune(
    pool: &PgPool,
    commune_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
    ip_address: Option<String>,
    user_agent: Option<String>,
) {
    let audit_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, commune_id, user_id, action, entity_type, entity_id,
            old_value, new_value, ip_address, user_agent
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(audit_id)
    .bind(commune_id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_value)
    .bind(new_value)
    .bind(ip_address)
    .bind(user_agent)
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, action, entity_type, "audit log write failed");
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn record_for_commune_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
    ip_address: Option<String>,
    user_agent: Option<String>,
) {
    let audit_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, commune_id, user_id, action, entity_type, entity_id,
            old_value, new_value, ip_address, user_agent
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(audit_id)
    .bind(commune_id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_value)
    .bind(new_value)
    .bind(ip_address)
    .bind(user_agent)
    .execute(&mut **tx)
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, action, entity_type, "audit log write failed");
    }
}
