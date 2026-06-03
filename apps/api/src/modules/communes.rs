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
        .route(
            "/communes",
            axum::routing::get(list_communes).post(create_commune),
        )
        .route(
            "/communes/{id}",
            axum::routing::get(get_commune).patch(patch_commune),
        )
}

#[derive(Debug, Deserialize)]
struct CreateCommuneRequest {
    code: String,
    nom: String,
    region: String,
    departement: String,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchCommuneRequest {
    code: Option<String>,
    nom: Option<String>,
    region: Option<String>,
    departement: Option<String>,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CommuneResponse {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn get_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
) -> Result<Json<CommuneResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let commune = load_commune(&state.db, commune_id).await?;
    auth_user.require_commune_access(commune.id)?;
    Ok(Json(commune))
}

async fn list_communes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Paginated<CommuneResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let pagination = Pagination::from_query(query)?;

    let (rows, total) = if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        let total = sqlx::query("SELECT COUNT(*) AS total FROM communes WHERE deleted_at IS NULL")
            .fetch_one(&state.db)
            .await?
            .get("total");
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM communes
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&state.db)
        .await?;
        (rows, total)
    } else {
        let commune_id = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        let total = sqlx::query(
            "SELECT COUNT(*) AS total FROM communes WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(commune_id)
        .fetch_one(&state.db)
        .await?
        .get("total");
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM communes
            WHERE id = $1 AND deleted_at IS NULL
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(commune_id)
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&state.db)
        .await?;
        (rows, total)
    };

    let communes = rows.into_iter().map(row_to_commune).collect::<Vec<_>>();
    Ok(Json(Paginated::new(communes, &pagination, total)))
}

async fn create_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateCommuneRequest>,
) -> Result<Json<CommuneResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;

    let commune_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO communes (
            id, code, nom, region, departement, adresse, telephone,
            email, site_web, logo_url, theme_color, active
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(commune_id)
    .bind(required_text(payload.code, "code")?)
    .bind(required_text(payload.nom, "nom")?)
    .bind(required_text(payload.region, "region")?)
    .bind(required_text(payload.departement, "departement")?)
    .bind(clean_optional(payload.adresse))
    .bind(clean_optional(payload.telephone))
    .bind(clean_optional(payload.email))
    .bind(clean_optional(payload.site_web))
    .bind(clean_optional(payload.logo_url))
    .bind(clean_optional(payload.theme_color))
    .bind(payload.active.unwrap_or(true))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "COMMUNE_CREATED",
        "communes",
        Some(commune_id),
        None,
        Some(json!({ "id": commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_commune(&state.db, commune_id).await?))
}

async fn patch_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchCommuneRequest>,
) -> Result<Json<CommuneResponse>, ApiError> {
    if !auth_user.has_role(Role::SuperAdmin) {
        auth_user.require_any_role(&[Role::AdminCommune])?;
        auth_user.require_commune_access(commune_id)?;
    }

    let existing = load_commune(&state.db, commune_id).await?;
    let code = payload.code.map_or(Ok(existing.code.clone()), |value| {
        required_text(value, "code")
    })?;
    let nom = payload.nom.map_or(Ok(existing.nom.clone()), |value| {
        required_text(value, "nom")
    })?;
    let region = payload
        .region
        .map_or(Ok(existing.region.clone()), |value| {
            required_text(value, "region")
        })?;
    let departement = payload
        .departement
        .map_or(Ok(existing.departement.clone()), |value| {
            required_text(value, "departement")
        })?;

    sqlx::query(
        r#"
        UPDATE communes
        SET code = $2,
            nom = $3,
            region = $4,
            departement = $5,
            adresse = $6,
            telephone = $7,
            email = $8,
            site_web = $9,
            logo_url = $10,
            theme_color = $11,
            active = $12,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .bind(&code)
    .bind(&nom)
    .bind(&region)
    .bind(&departement)
    .bind(payload.adresse.or(existing.adresse.clone()))
    .bind(payload.telephone.or(existing.telephone.clone()))
    .bind(payload.email.or(existing.email.clone()))
    .bind(payload.site_web.or(existing.site_web.clone()))
    .bind(payload.logo_url.or(existing.logo_url.clone()))
    .bind(payload.theme_color.or(existing.theme_color.clone()))
    .bind(payload.active.unwrap_or(existing.active))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "COMMUNE_UPDATED",
        "communes",
        Some(commune_id),
        Some(json!({ "code": existing.code, "nom": existing.nom })),
        Some(json!({ "code": code, "nom": nom })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_commune(&state.db, commune_id).await?))
}

pub async fn load_commune(pool: &PgPool, commune_id: Uuid) -> Result<CommuneResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT *
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Commune introuvable"))?;

    Ok(row_to_commune(row))
}

fn row_to_commune(row: sqlx::postgres::PgRow) -> CommuneResponse {
    CommuneResponse {
        id: row.get("id"),
        code: row.get("code"),
        nom: row.get("nom"),
        region: row.get("region"),
        departement: row.get("departement"),
        adresse: row.get("adresse"),
        telephone: row.get("telephone"),
        email: row.get("email"),
        site_web: row.get("site_web"),
        logo_url: row.get("logo_url"),
        theme_color: row.get("theme_color"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
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
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}
