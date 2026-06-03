use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn record(
    pool: &PgPool,
    user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
) {
    let audit_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, user_id, action, entity_type, entity_id,
            old_value, new_value, ip_address, user_agent
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL)
        "#,
    )
    .bind(audit_id)
    .bind(user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, action, entity_type, "audit log write failed");
    }
}
