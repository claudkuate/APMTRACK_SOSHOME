use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::Paginated;
use crate::state::AppState;

/// Prédicat SQL d'une commune visible publiquement (active + abonnement valide).
const VISIBLE_COMMUNE_PREDICATE: &str = "active = true \
    AND subscription_status IN ('ACTIVE', 'TRIAL') \
    AND (subscription_expires_at IS NULL OR subscription_expires_at >= now()) \
    AND deleted_at IS NULL";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/geography/regions", axum::routing::get(list_regions))
        .route(
            "/geography/departements",
            axum::routing::get(list_departements),
        )
}

/// Routes publiques (sans authentification) — cascade Région → Département →
/// Commune pour le formulaire de signalement citoyen. Filtrées aux entités
/// contenant au moins une commune visible afin d'éviter les impasses.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/geography/regions", axum::routing::get(public_regions))
        .route(
            "/geography/regions/{id}/departements",
            axum::routing::get(public_departements),
        )
        .route(
            "/geography/departements/{id}/communes",
            axum::routing::get(public_communes),
        )
}

#[derive(Debug, Serialize)]
struct RegionOption {
    id: Uuid,
    code: String,
    nom: String,
}

#[derive(Debug, Serialize)]
struct DepartementOption {
    id: Uuid,
    region_id: Uuid,
    nom: String,
}

#[derive(Debug, Serialize)]
struct PublicCommuneOption {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
}

#[derive(Debug, Deserialize)]
struct DepartementQuery {
    region_id: Option<Uuid>,
}

// --- Référentiel authentifié (formulaire commune côté admin) -----------------

async fn list_regions(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Paginated<RegionOption>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let rows = sqlx::query(
        r#"
        SELECT id, code, nom
        FROM regions
        WHERE deleted_at IS NULL
        ORDER BY nom
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let items: Vec<RegionOption> = rows.into_iter().map(row_to_region).collect();
    Ok(Json(single_page(items)))
}

async fn list_departements(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DepartementQuery>,
) -> Result<Json<Paginated<DepartementOption>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let rows = sqlx::query(
        r#"
        SELECT id, region_id, nom
        FROM departements
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR region_id = $1)
        ORDER BY nom
        "#,
    )
    .bind(query.region_id)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<DepartementOption> = rows.into_iter().map(row_to_departement).collect();
    Ok(Json(single_page(items)))
}

/// Enveloppe une liste complète (petites tables de référence) en une page unique
/// compatible avec le service de relations du back-office.
fn single_page<T>(items: Vec<T>) -> Paginated<T> {
    let total = items.len() as i64;
    Paginated {
        items,
        page: 1,
        page_size: total.max(1),
        total,
    }
}

// --- Cascade publique (signalement citoyen) ----------------------------------

async fn public_regions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RegionOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:regions")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT r.id, r.code, r.nom
        FROM regions r
        WHERE r.deleted_at IS NULL
          AND EXISTS (
            SELECT 1 FROM communes c
            WHERE c.region_id = r.id AND {VISIBLE_COMMUNE_PREDICATE}
          )
        ORDER BY r.nom
        "#
    ))
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(row_to_region).collect()))
}

async fn public_departements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(region_id): Path<Uuid>,
) -> Result<Json<Vec<DepartementOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:departements")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT d.id, d.region_id, d.nom
        FROM departements d
        WHERE d.deleted_at IS NULL
          AND d.region_id = $1
          AND EXISTS (
            SELECT 1 FROM communes c
            WHERE c.departement_id = d.id AND {VISIBLE_COMMUNE_PREDICATE}
          )
        ORDER BY d.nom
        "#
    ))
    .bind(region_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(row_to_departement).collect()))
}

async fn public_communes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(departement_id): Path<Uuid>,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:communes")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT id, code, nom, region, departement
        FROM communes c
        WHERE c.departement_id = $1 AND {VISIBLE_COMMUNE_PREDICATE}
        ORDER BY nom
        "#
    ))
    .bind(departement_id)
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

fn rate_limit(state: &AppState, headers: &HeaderMap, key: &str) -> Result<(), ApiError> {
    state.rate_limiter.check(
        key,
        headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )
}

fn row_to_region(row: sqlx::postgres::PgRow) -> RegionOption {
    RegionOption {
        id: row.get("id"),
        code: row.get("code"),
        nom: row.get("nom"),
    }
}

fn row_to_departement(row: sqlx::postgres::PgRow) -> DepartementOption {
    DepartementOption {
        id: row.get("id"),
        region_id: row.get("region_id"),
        nom: row.get("nom"),
    }
}
