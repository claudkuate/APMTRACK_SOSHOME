use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{resolve_commune_filter, validate_gps};
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::sequences::{next_document_sequence, SEQUENCE_SIGNALEMENT};
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
    pub zone_id: Option<Uuid>,
    pub signalement_number: String,
    pub type_incident: String,
    pub location_description: String,
    pub lieu_dit: Option<String>,
    pub description: String,
    pub contact_anonyme: bool,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_info: Option<String>,
    pub status: String,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Colonnes exposées par les SELECT (lat/lon castées en double precision).
const SIGNALEMENT_COLUMNS: &str = "id, commune_id, zone_id, signalement_number, type_incident, \
    location_description, lieu_dit, description, contact_anonyme, contact_name, contact_phone, \
    contact_info, status, \
    gps_latitude::double precision AS gps_latitude, \
    gps_longitude::double precision AS gps_longitude, \
    created_at, updated_at";

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
    pub zone_id: Option<Uuid>,
    pub type_incident: String,
    pub location_description: Option<String>,
    pub lieu_dit: Option<String>,
    pub description: String,
    pub contact_anonyme: Option<bool>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_info: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SignalementPublicTrackResponse {
    pub signalement_number: String,
    pub commune_nom: String,
    pub commune_code: String,
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
    headers: HeaderMap,
    Path(numero_suivi): Path<String>,
) -> Result<Json<SignalementPublicTrackResponse>, ApiError> {
    state.rate_limiter.check(
        "public:signalements:track",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let row = sqlx::query(
        r#"
        SELECT s.signalement_number, s.type_incident, s.status,
               s.created_at, s.updated_at,
               c.nom AS commune_nom, c.code AS commune_code
        FROM signalements s
        INNER JOIN communes c ON c.id = s.commune_id
        WHERE s.signalement_number = $1
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(&numero_suivi)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Signalement introuvable"))?;

    Ok(Json(SignalementPublicTrackResponse {
        signalement_number: row.get("signalement_number"),
        commune_nom: row.get("commune_nom"),
        commune_code: row.get("commune_code"),
        type_incident: row.get("type_incident"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
}

/// Création publique — sans authentification.
async fn create_signalement_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<CreateSignalementRequest>,
) -> Result<(StatusCode, Json<SignalementCreatedResponse>), ApiError> {
    state.rate_limiter.check(
        "public:signalements:create",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let incident_input = required_text(payload.type_incident, "type_incident")?;
    let description = required_text(payload.description, "description")?;
    let lieu_dit = clean_optional(payload.lieu_dit);
    validate_gps(payload.gps_latitude, payload.gps_longitude)?;

    let mut tx = state.db.begin().await?;

    let commune_code: Option<String> =
        sqlx::query_scalar(
            r#"
            SELECT code
            FROM communes
            WHERE id = $1
              AND deleted_at IS NULL
              AND active = true
              AND subscription_status IN ('ACTIVE', 'TRIAL')
              AND (subscription_expires_at IS NULL OR subscription_expires_at >= now())
            "#,
        )
            .bind(payload.commune_id)
            .fetch_optional(&mut *tx)
            .await?;
    let commune_code = commune_code.ok_or_else(|| ApiError::not_found("Commune introuvable"))?;

    let type_incident = resolve_public_incident_type(&mut tx, payload.commune_id, &incident_input)
        .await?;
    let (zone_id, location) =
        resolve_public_signalement_location(&mut tx, payload.commune_id, payload.zone_id, payload.location_description, lieu_dit.as_deref())
            .await?;

    let anonyme = payload.contact_anonyme.unwrap_or(false);
    let (contact_name, contact_phone, contact_info) = if anonyme {
        (None, None, None)
    } else {
        let contact_name = required_optional_text(payload.contact_name, "contact_name")?;
        let contact_phone = required_optional_text(payload.contact_phone, "contact_phone")?;
        let contact_info = clean_optional(payload.contact_info)
            .or_else(|| Some(format!("{contact_name} - {contact_phone}")));
        (Some(contact_name), Some(contact_phone), contact_info)
    };

    let number = generate_signalement_number(&mut tx, &commune_code, payload.commune_id).await?;
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO signalements (
            id, commune_id, zone_id, signalement_number, type_incident,
            location_description, lieu_dit, description, contact_anonyme,
            contact_name, contact_phone, contact_info,
            gps_latitude, gps_longitude
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(zone_id)
    .bind(&number)
    .bind(&type_incident)
    .bind(&location)
    .bind(lieu_dit.as_deref())
    .bind(&description)
    .bind(anonyme)
    .bind(contact_name.as_deref())
    .bind(contact_phone.as_deref())
    .bind(contact_info)
    .bind(payload.gps_latitude)
    .bind(payload.gps_longitude)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(SignalementCreatedResponse {
            id,
            signalement_number: number,
            status: "RECU".to_string(),
            message: "Signalement recu et en attente de traitement",
        }),
    ))
}
async fn list_signalements(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<SignalementFilterQuery>,
) -> Result<Json<Paginated<SignalementResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM signalements WHERE 1=1");
    apply_filters(&mut count_qb, commune_filter, query.status.as_deref());
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT {SIGNALEMENT_COLUMNS} FROM signalements WHERE 1=1"
    ));
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

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
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
    let row = sqlx::query(&format!(
        "SELECT {SIGNALEMENT_COLUMNS} FROM signalements WHERE id = $1"
    ))
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
        zone_id: row.get("zone_id"),
        signalement_number: row.get("signalement_number"),
        type_incident: row.get("type_incident"),
        location_description: row.get("location_description"),
        lieu_dit: row.get("lieu_dit"),
        description: row.get("description"),
        contact_anonyme: row.get("contact_anonyme"),
        contact_name: row.get("contact_name"),
        contact_phone: row.get("contact_phone"),
        contact_info: row.get("contact_info"),
        status: row.get("status"),
        gps_latitude: row.get("gps_latitude"),
        gps_longitude: row.get("gps_longitude"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn resolve_public_incident_type(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    input: &str,
) -> Result<String, ApiError> {
    let incident_id = Uuid::parse_str(input).ok();
    let type_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT it.nom
        FROM intervention_types it
        INNER JOIN intervention_categories c ON c.id = it.category_id
        WHERE it.commune_id = $1
          AND it.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND it.active = true
          AND c.active = true
          AND (it.id = $2 OR lower(it.nom) = lower($3))
        LIMIT 1
        "#,
    )
    .bind(commune_id)
    .bind(incident_id)
    .bind(input)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(name) = type_name {
        return Ok(name);
    }

    let active_types: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM intervention_types it
        INNER JOIN intervention_categories c ON c.id = it.category_id
        WHERE it.commune_id = $1
          AND it.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND it.active = true
          AND c.active = true
        "#,
    )
    .bind(commune_id)
    .fetch_one(&mut **tx)
    .await?;

    if active_types > 0 {
        Err(ApiError::bad_request(
            "Type d'incident invalide pour cette commune",
        ))
    } else {
        Ok(input.to_string())
    }
}

async fn resolve_public_signalement_location(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    zone_id: Option<Uuid>,
    legacy_location: Option<String>,
    lieu_dit: Option<&str>,
) -> Result<(Option<Uuid>, String), ApiError> {
    if let Some(zone_id) = zone_id {
        let zone_name: Option<String> = sqlx::query_scalar(
            r#"
            SELECT nom
            FROM zones
            WHERE id = $1
              AND commune_id = $2
              AND active = true
              AND deleted_at IS NULL
            "#,
        )
        .bind(zone_id)
        .bind(commune_id)
        .fetch_optional(&mut **tx)
        .await?;
        let zone_name = zone_name.ok_or_else(|| {
            ApiError::bad_request("Zone ou quartier invalide pour cette commune")
        })?;
        let location = match lieu_dit {
            Some(value) => format!("{zone_name} - {value}"),
            None => zone_name,
        };
        return Ok((Some(zone_id), location));
    }

    let location = required_optional_text(legacy_location, "location_description")?;
    Ok((None, location))
}

async fn generate_signalement_number(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_code: &str,
    commune_id: Uuid,
) -> Result<String, ApiError> {
    let (year, seq) = next_document_sequence(tx, commune_id, SEQUENCE_SIGNALEMENT).await?;
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

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

fn required_optional_text(value: Option<String>, field: &'static str) -> Result<String, ApiError> {
    clean_optional(value).ok_or_else(|| ApiError::bad_request(format!("{field} est requis")))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
