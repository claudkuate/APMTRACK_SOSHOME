use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/zones", axum::routing::get(list_zones).post(create_zone))
        .route(
            "/zones/{id}",
            axum::routing::get(get_zone)
                .patch(patch_zone)
                .delete(delete_zone),
        )
}

const VALID_TYPES: &[&str] = &[
    "QUARTIER",
    "BLOC",
    "SECTEUR",
    "LIEU_DIT",
    "MARCHE",
    "AXE_ROUTIER",
    "ZONE_COMMERCIALE",
    "ZONE_SENSIBLE",
];

#[derive(Debug, Deserialize)]
pub struct ZoneFilterQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub commune_id: Option<Uuid>,
    pub active: Option<bool>,
    pub type_zone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateZoneRequest {
    commune_id: Uuid,
    nom: String,
    type_zone: String,
    parent_id: Option<Uuid>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchZoneRequest {
    nom: Option<String>,
    type_zone: Option<String>,
    parent_id: Option<Uuid>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ZoneResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub nom: String,
    pub type_zone: String,
    pub parent_id: Option<Uuid>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn list_zones(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ZoneFilterQuery>,
) -> Result<Json<Paginated<ZoneResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let base_where = build_filter_clause(commune_filter, query.active, query.type_zone.as_deref());

    let total: i64 = sqlx::query(&format!(
        "SELECT COUNT(*) AS total FROM zones WHERE deleted_at IS NULL {base_where}"
    ))
    .fetch_one(&state.db)
    .await?
    .get("total");

    let rows = sqlx::query(&format!(
        r#"
        SELECT * FROM zones
        WHERE deleted_at IS NULL {base_where}
        ORDER BY nom ASC
        LIMIT $1 OFFSET $2
        "#
    ))
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.db)
    .await?;

    let zones = rows.into_iter().map(row_to_zone).collect();
    Ok(Json(Paginated::new(zones, &pagination, total)))
}

async fn get_zone(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(zone_id): Path<Uuid>,
) -> Result<Json<ZoneResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let zone = load_zone(&state.db, zone_id).await?;
    auth_user.require_commune_access(zone.commune_id)?;
    Ok(Json(zone))
}

async fn create_zone(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateZoneRequest>,
) -> Result<Json<ZoneResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let nom = required_text(payload.nom, "nom")?;
    let type_zone = validate_type_zone(payload.type_zone)?;

    if let Some(parent_id) = payload.parent_id {
        let parent = load_zone(&state.db, parent_id).await?;
        if parent.commune_id != payload.commune_id {
            return Err(ApiError::bad_request(
                "La zone parente doit appartenir a la meme commune",
            ));
        }
    }

    let zone_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO zones (id, commune_id, nom, type_zone, parent_id, active)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(zone_id)
    .bind(payload.commune_id)
    .bind(&nom)
    .bind(&type_zone)
    .bind(payload.parent_id)
    .bind(payload.active.unwrap_or(true))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "ZONE_CREATED",
        "zones",
        Some(zone_id),
        None,
        Some(json!({ "nom": nom, "commune_id": payload.commune_id })),
    )
    .await;

    Ok(Json(load_zone(&state.db, zone_id).await?))
}

async fn patch_zone(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(zone_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchZoneRequest>,
) -> Result<Json<ZoneResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_zone(&state.db, zone_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let nom = match payload.nom {
        Some(v) => required_text(v, "nom")?,
        None => existing.nom.clone(),
    };
    let type_zone = match payload.type_zone {
        Some(v) => validate_type_zone(v)?,
        None => existing.type_zone.clone(),
    };

    if let Some(parent_id) = payload.parent_id {
        if parent_id == zone_id {
            return Err(ApiError::bad_request(
                "Une zone ne peut pas etre sa propre parente",
            ));
        }
        let parent = load_zone(&state.db, parent_id).await?;
        if parent.commune_id != existing.commune_id {
            return Err(ApiError::bad_request(
                "La zone parente doit appartenir a la meme commune",
            ));
        }
    }

    sqlx::query(
        r#"
        UPDATE zones
        SET nom = $2, type_zone = $3, parent_id = $4, active = $5, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(zone_id)
    .bind(&nom)
    .bind(&type_zone)
    .bind(payload.parent_id.or(existing.parent_id))
    .bind(payload.active.unwrap_or(existing.active))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "ZONE_UPDATED",
        "zones",
        Some(zone_id),
        Some(json!({ "nom": existing.nom, "active": existing.active })),
        Some(json!({ "nom": nom, "active": payload.active })),
    )
    .await;

    Ok(Json(load_zone(&state.db, zone_id).await?))
}

async fn delete_zone(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(zone_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_zone(&state.db, zone_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let children: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM zones WHERE parent_id = $1 AND deleted_at IS NULL")
            .bind(zone_id)
            .fetch_one(&state.db)
            .await?
            .get("total");

    if children > 0 {
        return Err(ApiError::conflict(
            "Impossible de supprimer une zone ayant des sous-zones",
        ));
    }

    sqlx::query("UPDATE zones SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(zone_id)
        .execute(&state.db)
        .await?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "ZONE_DELETED",
        "zones",
        Some(zone_id),
        Some(json!({ "nom": existing.nom })),
        None,
    )
    .await;

    Ok(Json(json!({ "deleted": true, "id": zone_id })))
}

pub async fn load_zone(pool: &PgPool, zone_id: Uuid) -> Result<ZoneResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM zones WHERE id = $1 AND deleted_at IS NULL")
        .bind(zone_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Zone introuvable"))?;
    Ok(row_to_zone(row))
}

fn row_to_zone(row: sqlx::postgres::PgRow) -> ZoneResponse {
    ZoneResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        nom: row.get("nom"),
        type_zone: row.get("type_zone"),
        parent_id: row.get("parent_id"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
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

fn build_filter_clause(
    commune_filter: Option<Uuid>,
    active: Option<bool>,
    type_zone: Option<&str>,
) -> String {
    let mut clauses = Vec::new();
    if let Some(id) = commune_filter {
        clauses.push(format!("AND commune_id = '{id}'"));
    }
    if let Some(a) = active {
        clauses.push(format!("AND active = {a}"));
    }
    if let Some(t) = type_zone {
        let safe = t.replace('\'', "''");
        clauses.push(format!("AND type_zone = '{safe}'"));
    }
    clauses.join(" ")
}

fn validate_type_zone(value: String) -> Result<String, ApiError> {
    let upper = value.trim().to_ascii_uppercase();
    if VALID_TYPES.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(ApiError::bad_request(format!(
            "type_zone invalide. Valeurs acceptees : {}",
            VALID_TYPES.join(", ")
        )))
    }
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}
