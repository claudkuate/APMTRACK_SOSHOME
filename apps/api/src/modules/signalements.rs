use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signalements", axum::routing::get(list_signalements))
        .route("/signalements/{id}", axum::routing::get(get_signalement))
        .route(
            "/signalements/{id}/status",
            axum::routing::patch(patch_signalement_status),
        )
}

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route(
            "/signalements",
            axum::routing::post(create_signalement_public),
        )
        .route(
            "/signalements/{numero_suivi}",
            axum::routing::get(track_signalement_public),
        )
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct SignalementResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub signalement_number: String,
    pub type_incident: String,
    pub location_description: String,
    pub description: String,
    pub contact_anonyme: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SignalementCreatedResponse {
    pub id: Uuid,
    pub signalement_number: String,
    pub status: String,
    pub message: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct SignalementFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSignalementRequest {
    pub commune_id: Uuid,
    pub type_incident: String,
    pub location_description: String,
    pub description: String,
    pub contact_anonyme: Option<bool>,
    pub contact_info: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SignalementPublicTrackResponse {
    pub signalement_number: String,
    pub commune_id: Uuid,
    pub type_incident: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PatchSignalementStatusRequest {
    pub status: String,
    pub admin_notes: Option<String>,
    pub assigned_to: Option<Uuid>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Suivi public par numéro — sans authentification, sans données sensibles.
async fn track_signalement_public(
    State(state): State<AppState>,
    Path(numero_suivi): Path<String>,
) -> Result<Json<SignalementPublicTrackResponse>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT signalement_number, commune_id, type_incident, status, created_at, updated_at
        FROM signalements
        WHERE signalement_number = $1
        "#,
    )
    .bind(&numero_suivi)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Signalement introuvable"))?;

    Ok(Json(SignalementPublicTrackResponse {
        signalement_number: row.get("signalement_number"),
        commune_id: row.get("commune_id"),
        type_incident: row.get("type_incident"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

/// Création publique — sans authentification.
async fn create_signalement_public(
    State(state): State<AppState>,
    ApiJson(payload): ApiJson<CreateSignalementRequest>,
) -> Result<(StatusCode, Json<SignalementCreatedResponse>), ApiError> {
    let type_incident = required_text(payload.type_incident, "type_incident")?;
    let location = required_text(payload.location_description, "location_description")?;
    let description = required_text(payload.description, "description")?;

    // Vérifier que la commune existe
    let commune_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM communes WHERE id = $1 AND deleted_at IS NULL)")
            .bind(payload.commune_id)
            .fetch_one(&state.db)
            .await?;
    if !commune_exists {
        return Err(ApiError::not_found("Commune introuvable"));
    }

    let anonyme = payload.contact_anonyme.unwrap_or(false);
    let contact_info = if anonyme {
        None
    } else {
        clean_optional(payload.contact_info)
    };

    let number = generate_signalement_number(&state.db, payload.commune_id).await?;
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO signalements (
            id, commune_id, signalement_number, type_incident,
            location_description, description, contact_anonyme, contact_info
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(&number)
    .bind(&type_incident)
    .bind(&location)
    .bind(&description)
    .bind(anonyme)
    .bind(contact_info)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    Ok((
        StatusCode::CREATED,
        Json(SignalementCreatedResponse {
            id,
            signalement_number: number,
            status: "RECU".to_string(),
            message: "Votre signalement a été enregistré avec succès.",
        }),
    ))
}

async fn list_signalements(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<SignalementFilterQuery>,
) -> Result<Json<Paginated<SignalementResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
    ])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM signalements WHERE 1=1");
    apply_filters(&mut count_qb, commune_filter, query.status.as_deref());
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM signalements WHERE 1=1");
    apply_filters(&mut qb, commune_filter, query.status.as_deref());
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_signalement).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_signalement(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SignalementResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let item = load_signalement(&state.db, id).await?;
    auth_user.require_commune_access(item.commune_id)?;
    Ok(Json(item))
}

async fn patch_signalement_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchSignalementStatusRequest>,
) -> Result<Json<SignalementResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_signalement(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let valid_statuses = ["RECU", "EN_COURS", "TRAITE", "CLASSE", "REJETE"];
    if !valid_statuses.contains(&payload.status.as_str()) {
        return Err(ApiError::bad_request(format!(
            "Statut invalide: {}",
            payload.status
        )));
    }

    sqlx::query(
        r#"
        UPDATE signalements
        SET status = $2, admin_notes = COALESCE($3, admin_notes),
            assigned_to = COALESCE($4, assigned_to), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&payload.status)
    .bind(payload.admin_notes.as_deref())
    .bind(payload.assigned_to)
    .execute(&state.db)
    .await?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "SIGNALEMENT_STATUS_CHANGED",
        "signalements",
        Some(id),
        Some(json!({ "status": existing.status })),
        Some(json!({ "status": payload.status })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_signalement(&state.db, id).await?))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_signalement(pool: &PgPool, id: Uuid) -> Result<SignalementResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM signalements WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Signalement introuvable"))?;
    Ok(row_to_signalement(row))
}

fn row_to_signalement(row: sqlx::postgres::PgRow) -> SignalementResponse {
    SignalementResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        signalement_number: row.get("signalement_number"),
        type_incident: row.get("type_incident"),
        location_description: row.get("location_description"),
        description: row.get("description"),
        contact_anonyme: row.get("contact_anonyme"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn generate_signalement_number(pool: &PgPool, commune_id: Uuid) -> Result<String, ApiError> {
    let year = Utc::now().year();
    let commune_code: String =
        sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
            .bind(commune_id)
            .fetch_one(pool)
            .await?;
    let seq: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM signalements WHERE commune_id = $1 AND EXTRACT(YEAR FROM created_at) = $2",
    )
    .bind(commune_id)
    .bind(year as i64)
    .fetch_one(pool)
    .await?;
    Ok(format!(
        "SIG-{}-{}-{:06}",
        commune_code.to_uppercase(),
        year,
        seq
    ))
}

fn apply_filters(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    status: Option<&str>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
}

fn resolve_commune_filter(
    auth_user: &AuthUser,
    requested: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        return Ok(requested);
    }
    let user_commune = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
    if let Some(req) = requested {
        if req != user_commune {
            return Err(ApiError::forbidden("Acces refuse a cette commune"));
        }
    }
    Ok(Some(user_commune))
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
