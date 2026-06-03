use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::helpers::is_global_actor;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audit-logs", axum::routing::get(list_audit_logs))
        .route("/audit-logs/{id}", axum::routing::get(get_audit_log))
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
    pub id: Uuid,
    pub commune_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuditLogFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    user_id: Option<Uuid>,
    action: Option<String>,
    entity_type: Option<String>,
    entity_id: Option<Uuid>,
    commune_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_audit_logs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<AuditLogFilterQuery>,
) -> Result<Json<Paginated<AuditLogResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::Superviseur])?;
    let commune_scope = audit_commune_scope(&auth_user, query.commune_id)?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM audit_logs WHERE 1=1");
    apply_filters(&mut count_qb, &query);
    apply_commune_scope(&mut count_qb, commune_scope);
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM audit_logs WHERE 1=1");
    apply_filters(&mut qb, &query);
    apply_commune_scope(&mut qb, commune_scope);
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_audit_log).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_audit_log(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<AuditLogResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::Superviseur])?;

    let row = sqlx::query("SELECT * FROM audit_logs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Log d'audit introuvable"))?;

    let commune_id: Option<Uuid> = row.get("commune_id");
    if !is_global_actor(&auth_user) {
        let user_commune = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        if commune_id != Some(user_commune) {
            return Err(ApiError::forbidden("Acces refuse a ce log d'audit"));
        }
    }

    Ok(Json(row_to_audit_log(row)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn row_to_audit_log(row: sqlx::postgres::PgRow) -> AuditLogResponse {
    AuditLogResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        user_id: row.get("user_id"),
        action: row.get("action"),
        entity_type: row.get("entity_type"),
        entity_id: row.get("entity_id"),
        old_value: row.get("old_value"),
        new_value: row.get("new_value"),
        ip_address: row.get("ip_address"),
        user_agent: row.get("user_agent"),
        created_at: row.get("created_at"),
    }
}

fn apply_filters(qb: &mut QueryBuilder<sqlx::Postgres>, query: &AuditLogFilterQuery) {
    if let Some(id) = query.user_id {
        qb.push(" AND user_id = ").push_bind(id);
    }
    if let Some(ref action) = query.action {
        qb.push(" AND action = ").push_bind(action.clone());
    }
    if let Some(ref entity_type) = query.entity_type {
        qb.push(" AND entity_type = ")
            .push_bind(entity_type.clone());
    }
    if let Some(id) = query.entity_id {
        qb.push(" AND entity_id = ").push_bind(id);
    }
    if let Some(id) = query.commune_id {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(from) = query.from {
        qb.push(" AND created_at >= ").push_bind(from);
    }
    if let Some(to) = query.to {
        qb.push(" AND created_at <= ").push_bind(to);
    }
}

fn audit_commune_scope(
    auth_user: &AuthUser,
    requested: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    if is_global_actor(auth_user) {
        return Ok(requested);
    }
    let user_commune = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
    if let Some(requested) = requested {
        if requested != user_commune {
            return Err(ApiError::forbidden("Acces refuse a cette commune"));
        }
    }
    Ok(Some(user_commune))
}

fn apply_commune_scope(qb: &mut QueryBuilder<sqlx::Postgres>, commune_scope: Option<Uuid>) {
    if let Some(id) = commune_scope {
        qb.push(" AND commune_id = ").push_bind(id);
    }
}
