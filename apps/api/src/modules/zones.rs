use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{resolve_commune_filter, validate_geojson_polygon};
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
    /// Contour GeoJSON (Polygon) optionnel.
    boundary: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PatchZoneRequest {
    nom: Option<String>,
    type_zone: Option<String>,
    parent_id: Option<Uuid>,
    active: Option<bool>,
    /// Contour GeoJSON (Polygon) optionnel — remplace le contour existant si fourni.
    boundary: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ZoneResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub nom: String,
    pub type_zone: String,
    pub parent_id: Option<Uuid>,
    pub active: bool,
    /// Contour GeoJSON (Polygon) ou null.
    pub boundary: Option<serde_json::Value>,
    /// Centre GeoJSON (Point) calculé depuis le contour, ou null.
    pub centre: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Colonnes exposées par les SELECT (le contour est converti en GeoJSON texte).
const ZONE_COLUMNS: &str = "id, commune_id, nom, type_zone, parent_id, active, \
    ST_AsGeoJSON(boundary) AS boundary_geojson, ST_AsGeoJSON(centre) AS centre_geojson, \
    created_at, updated_at";

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

    let type_filter = match query.type_zone {
        Some(ref t) => Some(validate_type_zone(t.clone())?),
        None => None,
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM zones WHERE deleted_at IS NULL");
    if let Some(id) = commune_filter {
        count_qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(a) = query.active {
        count_qb.push(" AND active = ").push_bind(a);
    }
    if let Some(ref t) = type_filter {
        count_qb.push(" AND type_zone = ").push_bind(t.clone());
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT {ZONE_COLUMNS} FROM zones WHERE deleted_at IS NULL"
    ));
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(a) = query.active {
        qb.push(" AND active = ").push_bind(a);
    }
    if let Some(ref t) = type_filter {
        qb.push(" AND type_zone = ").push_bind(t.clone());
    }
    qb.push(" ORDER BY nom ASC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
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
    let boundary_json = prepare_boundary(payload.boundary)?;

    if let Some(parent_id) = payload.parent_id {
        let parent = load_zone(&state.db, parent_id).await?;
        if parent.commune_id != payload.commune_id {
            return Err(ApiError::bad_request(
                "La zone parente doit appartenir a la meme commune",
            ));
        }
    }

    let zone_id = Uuid::new_v4();
    // Pas de vérification de cycle ici car la zone n'existe pas encore.
    // $7 (contour GeoJSON) alimente à la fois `boundary` et le `centre` (centroïde).
    sqlx::query(
        r#"
        INSERT INTO zones (id, commune_id, nom, type_zone, parent_id, active, boundary, centre)
        VALUES (
            $1, $2, $3, $4, $5, $6,
            ST_SetSRID(ST_GeomFromGeoJSON($7), 4326),
            ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($7), 4326))
        )
        "#,
    )
    .bind(zone_id)
    .bind(payload.commune_id)
    .bind(&nom)
    .bind(&type_zone)
    .bind(payload.parent_id)
    .bind(payload.active.unwrap_or(true))
    .bind(&boundary_json)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "ZONE_CREATED",
        "zones",
        Some(zone_id),
        None,
        Some(json!({ "nom": nom, "commune_id": payload.commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
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
    let boundary_json = prepare_boundary(payload.boundary)?;

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
        // Vérification de cycle profond via CTE récursive
        check_zone_cycle(&state.db, zone_id, parent_id).await?;
    }

    // Le contour ($6) n'est mis à jour que s'il est fourni (COALESCE conserve l'existant sinon).
    sqlx::query(
        r#"
        UPDATE zones
        SET nom = $2,
            type_zone = $3,
            parent_id = $4,
            active = $5,
            boundary = COALESCE(ST_SetSRID(ST_GeomFromGeoJSON($6), 4326), boundary),
            centre = COALESCE(ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($6), 4326)), centre),
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(zone_id)
    .bind(&nom)
    .bind(&type_zone)
    .bind(payload.parent_id.or(existing.parent_id))
    .bind(payload.active.unwrap_or(existing.active))
    .bind(&boundary_json)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "ZONE_UPDATED",
        "zones",
        Some(zone_id),
        Some(json!({ "nom": existing.nom, "active": existing.active })),
        Some(json!({ "nom": nom, "active": payload.active })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
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

    let children: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM zones WHERE parent_id = $1 AND deleted_at IS NULL",
    )
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

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "ZONE_DELETED",
        "zones",
        Some(zone_id),
        Some(json!({ "nom": existing.nom })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "deleted": true, "id": zone_id })))
}

pub async fn load_zone(pool: &PgPool, zone_id: Uuid) -> Result<ZoneResponse, ApiError> {
    let row = sqlx::query(&format!(
        "SELECT {ZONE_COLUMNS} FROM zones WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(zone_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Zone introuvable"))?;
    Ok(row_to_zone(row))
}

fn parse_geojson_column(row: &sqlx::postgres::PgRow, column: &str) -> Option<serde_json::Value> {
    row.get::<Option<String>, _>(column)
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn row_to_zone(row: sqlx::postgres::PgRow) -> ZoneResponse {
    ZoneResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        nom: row.get("nom"),
        type_zone: row.get("type_zone"),
        parent_id: row.get("parent_id"),
        active: row.get("active"),
        boundary: parse_geojson_column(&row, "boundary_geojson"),
        centre: parse_geojson_column(&row, "centre_geojson"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Valide un contour GeoJSON optionnel et le sérialise en texte pour `ST_GeomFromGeoJSON`.
/// Renvoie `None` si aucun contour n'est fourni.
fn prepare_boundary(boundary: Option<serde_json::Value>) -> Result<Option<String>, ApiError> {
    match boundary {
        Some(value) if !value.is_null() => {
            validate_geojson_polygon(&value)?;
            Ok(Some(value.to_string()))
        }
        _ => Ok(None),
    }
}

fn validate_type_zone(value: String) -> Result<String, ApiError> {
    let upper = value.trim().to_ascii_uppercase();
    if VALID_TYPES.contains(&upper.as_str()) {
        Ok(upper)
    } else {
        Err(ApiError::bad_request(format!(
            "type_zone invalide. Valeurs acceptees: {}",
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

/// Vérifie qu'assigner `new_parent_id` comme parent de `zone_id` ne crée pas de cycle.
async fn check_zone_cycle(
    pool: &sqlx::PgPool,
    zone_id: Uuid,
    new_parent_id: Uuid,
) -> Result<(), ApiError> {
    let cycle_exists: bool = sqlx::query_scalar(
        r#"
        WITH RECURSIVE ancestors AS (
            SELECT id, parent_id FROM zones WHERE id = $1
            UNION ALL
            SELECT z.id, z.parent_id FROM zones z
            JOIN ancestors a ON z.id = a.parent_id
            WHERE z.deleted_at IS NULL
        )
        SELECT EXISTS(SELECT 1 FROM ancestors WHERE id = $2)
        "#,
    )
    .bind(new_parent_id)
    .bind(zone_id)
    .fetch_one(pool)
    .await?;

    if cycle_exists {
        return Err(ApiError::bad_request(
            "Cette relation de parente creerait un cycle dans la hierarchie des zones",
        ));
    }
    Ok(())
}
