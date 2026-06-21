use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::validate_geojson_polygon;
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

/// Routes publiques (sans authentification) — recherche de communes pour les
/// formulaires citoyens (ex. dépôt de signalement). N'expose qu'un sous-ensemble
/// minimal de colonnes, limité aux communes actives.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/communes", axum::routing::get(search_communes_public))
        .route(
            "/communes/{id}/signalement-options",
            axum::routing::get(public_signalement_options),
        )
}

#[derive(Debug, Deserialize)]
struct PublicCommuneSearchQuery {
    search: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct PublicCommuneOption {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
}

#[derive(Debug, Serialize)]
struct PublicIncidentTypeOption {
    id: Uuid,
    nom: String,
    category_id: Uuid,
    category_nom: String,
}

#[derive(Debug, Serialize)]
struct PublicZoneOption {
    id: Uuid,
    nom: String,
    type_zone: String,
}

#[derive(Debug, Serialize)]
struct PublicSignalementOptions {
    incident_types: Vec<PublicIncidentTypeOption>,
    zones: Vec<PublicZoneOption>,
}

/// Recherche publique de communes actives par nom ou code (autocomplete citoyen).
async fn search_communes_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PublicCommuneSearchQuery>,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    state.rate_limiter.check(
        "public:communes:search",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let search = clean_optional(query.search);
    let limit = query.limit.unwrap_or(400).clamp(1, 400);

    let rows = sqlx::query(
        r#"
        SELECT id, code, nom, region, departement
        FROM communes
        WHERE deleted_at IS NULL
          AND active = true
          AND subscription_status IN ('ACTIVE', 'TRIAL')
          AND (subscription_expires_at IS NULL OR subscription_expires_at >= now())
          AND (
            $1::text IS NULL
            OR nom ILIKE '%' || $1 || '%'
            OR code ILIKE '%' || $1 || '%'
          )
        ORDER BY nom
        LIMIT $2
        "#,
    )
    .bind(search.as_deref())
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let options = rows
        .into_iter()
        .map(|row| PublicCommuneOption {
            id: row.get("id"),
            code: row.get("code"),
            nom: row.get("nom"),
            region: row.get("region"),
            departement: row.get("departement"),
        })
        .collect();

    Ok(Json(options))
}

async fn public_signalement_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(commune_id): Path<Uuid>,
) -> Result<Json<PublicSignalementOptions>, ApiError> {
    state.rate_limiter.check(
        "public:communes:signalement-options",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    ensure_public_commune_visible(&state.db, commune_id).await?;

    let type_rows = sqlx::query(
        r#"
        SELECT
            it.id,
            it.nom,
            it.category_id,
            c.nom AS category_nom
        FROM intervention_types it
        INNER JOIN intervention_categories c ON c.id = it.category_id
        WHERE it.commune_id = $1
          AND it.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND it.active = true
          AND c.active = true
        ORDER BY c.nom, it.nom
        "#,
    )
    .bind(commune_id)
    .fetch_all(&state.db)
    .await?;

    let zone_rows = sqlx::query(
        r#"
        SELECT id, nom, type_zone
        FROM zones
        WHERE commune_id = $1
          AND deleted_at IS NULL
          AND active = true
        ORDER BY nom
        "#,
    )
    .bind(commune_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(PublicSignalementOptions {
        incident_types: type_rows
            .into_iter()
            .map(|row| PublicIncidentTypeOption {
                id: row.get("id"),
                nom: row.get("nom"),
                category_id: row.get("category_id"),
                category_nom: row.get("category_nom"),
            })
            .collect(),
        zones: zone_rows
            .into_iter()
            .map(|row| PublicZoneOption {
                id: row.get("id"),
                nom: row.get("nom"),
                type_zone: row.get("type_zone"),
            })
            .collect(),
    }))
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
    subscription_status: Option<String>,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    /// Contour GeoJSON (Polygon ou MultiPolygon) optionnel.
    boundary: Option<serde_json::Value>,
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
    subscription_status: Option<String>,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    /// Contour GeoJSON (Polygon ou MultiPolygon) optionnel — remplace l'existant si fourni.
    boundary: Option<serde_json::Value>,
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
    subscription_status: String,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    subscription_active: bool,
    public_visible: bool,
    /// Contour GeoJSON (MultiPolygon) ou null.
    boundary: Option<serde_json::Value>,
    /// Centre GeoJSON (Point) calculé depuis le contour, ou null.
    centre: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Colonnes exposées par les SELECT (le contour est converti en GeoJSON texte).
const COMMUNE_COLUMNS: &str = "id, code, nom, region, departement, adresse, telephone, email, \
    site_web, logo_url, theme_color, active, subscription_status, subscription_started_at, \
    subscription_expires_at, \
    (subscription_status IN ('ACTIVE', 'TRIAL') AND \
        (subscription_expires_at IS NULL OR subscription_expires_at >= now())) AS subscription_active, \
    (active = true AND subscription_status IN ('ACTIVE', 'TRIAL') AND \
        (subscription_expires_at IS NULL OR subscription_expires_at >= now())) AS public_visible, \
    ST_AsGeoJSON(boundary) AS boundary_geojson, ST_AsGeoJSON(centre) AS centre_geojson, \
    created_at, updated_at";

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
        let rows = sqlx::query(&format!(
            r#"
            SELECT {COMMUNE_COLUMNS}
            FROM communes
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#
        ))
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
        let rows = sqlx::query(&format!(
            r#"
            SELECT {COMMUNE_COLUMNS}
            FROM communes
            WHERE id = $1 AND deleted_at IS NULL
            LIMIT $2 OFFSET $3
            "#
        ))
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

    let boundary_json = prepare_boundary(payload.boundary)?;
    let subscription_status = validate_subscription_status(
        payload.subscription_status.as_deref().unwrap_or("ACTIVE"),
    )?;
    let commune_id = Uuid::new_v4();
    // $13 (contour GeoJSON) alimente `boundary` (forcé MultiPolygon) et le `centre` (centroïde).
    sqlx::query(
        r#"
        INSERT INTO communes (
            id, code, nom, region, departement, adresse, telephone,
            email, site_web, logo_url, theme_color, active,
            subscription_status, subscription_started_at, subscription_expires_at,
            boundary, centre
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
            $13, $14, $15,
            ST_Multi(ST_SetSRID(ST_GeomFromGeoJSON($16), 4326)),
            ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($16), 4326))
        )
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
    .bind(&subscription_status)
    .bind(payload.subscription_started_at)
    .bind(payload.subscription_expires_at)
    .bind(&boundary_json)
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
    let subscription_status = match payload.subscription_status.as_deref() {
        Some(value) => validate_subscription_status(value)?,
        None => existing.subscription_status.clone(),
    };
    let boundary_json = prepare_boundary(payload.boundary)?;

    // Le contour ($13) n'est mis à jour que s'il est fourni (COALESCE conserve l'existant sinon).
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
            subscription_status = $13,
            subscription_started_at = $14,
            subscription_expires_at = $15,
            boundary = COALESCE(ST_Multi(ST_SetSRID(ST_GeomFromGeoJSON($16), 4326)), boundary),
            centre = COALESCE(ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($16), 4326)), centre),
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
    .bind(&subscription_status)
    .bind(payload.subscription_started_at.or(existing.subscription_started_at))
    .bind(payload.subscription_expires_at.or(existing.subscription_expires_at))
    .bind(&boundary_json)
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
    let row = sqlx::query(&format!(
        r#"
        SELECT {COMMUNE_COLUMNS}
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#
    ))
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
        subscription_status: row.get("subscription_status"),
        subscription_started_at: row.get("subscription_started_at"),
        subscription_expires_at: row.get("subscription_expires_at"),
        subscription_active: row.get("subscription_active"),
        public_visible: row.get("public_visible"),
        boundary: row
            .get::<Option<String>, _>("boundary_geojson")
            .and_then(|s| serde_json::from_str(&s).ok()),
        centre: row
            .get::<Option<String>, _>("centre_geojson")
            .and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Valide un contour GeoJSON optionnel et le sérialise en texte pour `ST_GeomFromGeoJSON`.
fn prepare_boundary(boundary: Option<serde_json::Value>) -> Result<Option<String>, ApiError> {
    match boundary {
        Some(value) if !value.is_null() => {
            validate_geojson_polygon(&value)?;
            Ok(Some(value.to_string()))
        }
        _ => Ok(None),
    }
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

async fn ensure_public_commune_visible(pool: &PgPool, commune_id: Uuid) -> Result<(), ApiError> {
    let visible: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT active = true
           AND subscription_status IN ('ACTIVE', 'TRIAL')
           AND (subscription_expires_at IS NULL OR subscription_expires_at >= now())
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .fetch_optional(pool)
    .await?;

    match visible {
        Some(true) => Ok(()),
        Some(false) => Err(ApiError::forbidden("Commune non disponible")),
        None => Err(ApiError::not_found("Commune introuvable")),
    }
}

fn validate_subscription_status(value: &str) -> Result<String, ApiError> {
    let status = value.trim().to_ascii_uppercase();
    if matches!(status.as_str(), "ACTIVE" | "TRIAL" | "EXPIRED" | "SUSPENDED") {
        Ok(status)
    } else {
        Err(ApiError::bad_request(
            "Statut d'abonnement invalide. Valeurs acceptees: ACTIVE, TRIAL, EXPIRED, SUSPENDED",
        ))
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}
