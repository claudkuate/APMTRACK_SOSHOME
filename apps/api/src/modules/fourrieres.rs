//! Module Fourrières — mise en fourrière de véhicules.
//!
//! Conforme à la vision G-APM : la « gestion des fourrières » sécurise une niche
//! de recettes communales. Multi-tenant (isolation par commune), soft-delete,
//! traçabilité via `audit_logs`. Le numéro est généré côté serveur au format
//! `FOUR-{COMMUNE_CODE}-{YEAR}-{SEQ:06}`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{clean_optional, is_agent_only, required_text, resolve_commune_filter};
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::sequences::{next_document_sequence, SEQUENCE_FOURRIERE};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/fourrieres",
            axum::routing::get(list_fourrieres).post(create_fourriere),
        )
        .route("/fourrieres/{id}", axum::routing::get(get_fourriere))
        .route(
            "/fourrieres/{id}/status",
            axum::routing::patch(patch_fourriere_status),
        )
}

/// Statuts du cycle de vie d'une mise en fourrière.
const VALID_STATUSES: [&str; 4] = ["EN_FOURRIERE", "RESTITUE", "VENDU", "DETRUIT"];

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FourriereResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub pv_id: Option<Uuid>,
    pub fourriere_number: String,
    pub item_type: String,
    pub designation: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_type: Option<String>,
    pub vehicle_details: Option<String>,
    pub motif: String,
    pub lieu_enlevement: Option<String>,
    pub status: String,
    pub daily_fee_fcfa: i64,
    /// Frais de gardiennage estimés = `daily_fee_fcfa × jours détenus` (min. 1 jour).
    pub frais_gardiennage_fcfa: i64,
    pub entered_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub released_to: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const FOURRIERE_COLUMNS: &str = "id, commune_id, pv_id, fourriere_number, item_type, \
    designation, vehicle_plate, vehicle_type, vehicle_details, motif, lieu_enlevement, \
    status, daily_fee_fcfa, entered_at, released_at, released_to, created_at, updated_at";

#[derive(Debug, Deserialize)]
pub struct FourriereFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    status: Option<String>,
    vehicle_plate: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFourriereRequest {
    pub commune_id: Option<Uuid>,
    pub pv_id: Option<Uuid>,
    pub item_type: Option<String>,
    pub designation: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_type: Option<String>,
    pub vehicle_details: Option<String>,
    pub motif: String,
    pub lieu_enlevement: Option<String>,
    pub daily_fee_fcfa: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PatchFourriereStatusRequest {
    pub status: String,
    pub released_to: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_fourrieres(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<FourriereFilterQuery>,
) -> Result<Json<Paginated<FourriereResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
        Role::Receveur,
        Role::ApmAgent,
    ])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let plate = clean_optional(query.vehicle_plate).map(|p| p.to_ascii_uppercase());

    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) AS total FROM fourrieres WHERE deleted_at IS NULL",
    );
    apply_filters(&mut count_qb, commune_filter, query.status.as_deref(), plate.as_deref());
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT {FOURRIERE_COLUMNS} FROM fourrieres WHERE deleted_at IS NULL"
    ));
    apply_filters(&mut qb, commune_filter, query.status.as_deref(), plate.as_deref());
    qb.push(" ORDER BY entered_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_fourriere).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_fourriere(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FourriereResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
        Role::Receveur,
        Role::ApmAgent,
    ])?;
    let item = load_fourriere(&state.db, id).await?;
    auth_user.require_commune_access(item.commune_id)?;
    Ok(Json(item))
}

async fn create_fourriere(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateFourriereRequest>,
) -> Result<(StatusCode, Json<FourriereResponse>), ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent, Role::AdminCommune, Role::SuperAdmin])?;

    let commune_id = resolve_commune_filter(&auth_user, payload.commune_id)?
        .ok_or_else(|| ApiError::bad_request("commune_id est requis"))?;

    let item_type = normalize_item_type(payload.item_type.as_deref())?;
    let motif = required_text(payload.motif, "motif")?;
    let vehicle_plate =
        clean_optional(payload.vehicle_plate).map(|plate| plate.to_ascii_uppercase());
    let designation = clean_optional(payload.designation);
    // Un véhicule est identifié par sa plaque ; les autres objets par une désignation.
    if item_type == "VEHICULE" {
        if vehicle_plate.is_none() {
            return Err(ApiError::bad_request(
                "La plaque est requise pour un véhicule",
            ));
        }
    } else if designation.is_none() {
        return Err(ApiError::bad_request(
            "La désignation est requise pour un objet non-véhicule",
        ));
    }
    let vehicle_type = clean_optional(payload.vehicle_type);
    let vehicle_details = clean_optional(payload.vehicle_details);
    let lieu_enlevement = clean_optional(payload.lieu_enlevement);
    let daily_fee_fcfa = payload.daily_fee_fcfa.unwrap_or(0);
    if daily_fee_fcfa < 0 {
        return Err(ApiError::bad_request(
            "daily_fee_fcfa ne peut pas être négatif",
        ));
    }

    let mut tx = state.db.begin().await?;

    // Seul un agent actif peut enregistrer une mise en fourrière sur le terrain.
    if is_agent_only(&auth_user) {
        let agent_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM agents WHERE user_id = $1 AND commune_id = $2 AND deleted_at IS NULL LIMIT 1",
        )
        .bind(auth_user.id)
        .bind(commune_id)
        .fetch_optional(&mut *tx)
        .await?;
        match agent_status.as_deref() {
            Some("ACTIF") => {}
            Some(_) => {
                return Err(ApiError::forbidden(
                    "Seul un agent actif peut enregistrer une mise en fourrière",
                ))
            }
            None => {
                return Err(ApiError::forbidden(
                    "Agent introuvable pour cet utilisateur et cette commune",
                ))
            }
        }
    }

    if let Some(pv_id) = payload.pv_id {
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pvs WHERE id = $1 AND commune_id = $2 AND deleted_at IS NULL",
        )
        .bind(pv_id)
        .bind(commune_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(ApiError::bad_request("pv_id invalide pour cette commune"));
        }
    }

    let commune_code: String = sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
        .bind(commune_id)
        .fetch_one(&mut *tx)
        .await?;
    let (year, seq) = next_document_sequence(&mut tx, commune_id, SEQUENCE_FOURRIERE).await?;
    let number = format!(
        "FOUR-{}-{}-{:06}",
        commune_code.to_uppercase().replace(' ', "-"),
        year,
        seq
    );

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO fourrieres (
            id, commune_id, pv_id, fourriere_number, item_type, designation, vehicle_plate,
            vehicle_type, vehicle_details, motif, lieu_enlevement, daily_fee_fcfa, created_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(id)
    .bind(commune_id)
    .bind(payload.pv_id)
    .bind(&number)
    .bind(&item_type)
    .bind(designation.as_deref())
    .bind(vehicle_plate.as_deref())
    .bind(vehicle_type.as_deref())
    .bind(vehicle_details.as_deref())
    .bind(&motif)
    .bind(lieu_enlevement.as_deref())
    .bind(daily_fee_fcfa)
    .bind(auth_user.id)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    tx.commit().await?;

    let created = load_fourriere(&state.db, id).await?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "FOURRIERE_CREATED",
        "fourrieres",
        Some(id),
        None,
        Some(json!({
            "fourriere_number": number,
            "item_type": item_type,
            "designation": designation,
            "vehicle_plate": vehicle_plate,
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((StatusCode::CREATED, Json(created)))
}

async fn patch_fourriere_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchFourriereStatusRequest>,
) -> Result<Json<FourriereResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Receveur])?;
    let existing = load_fourriere(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let status = payload.status.trim().to_uppercase();
    if !VALID_STATUSES.contains(&status.as_str()) {
        return Err(ApiError::bad_request(format!(
            "Statut invalide: {} (attendu: {})",
            payload.status,
            VALID_STATUSES.join(", ")
        )));
    }
    let released_to = clean_optional(payload.released_to);
    // La date de sortie est posée dès que le véhicule quitte la fourrière.
    let is_exit = matches!(status.as_str(), "RESTITUE" | "VENDU" | "DETRUIT");

    sqlx::query(
        r#"
        UPDATE fourrieres
        SET status = $2,
            released_to = COALESCE($3, released_to),
            released_at = CASE WHEN $4 THEN COALESCE(released_at, now()) ELSE released_at END,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&status)
    .bind(released_to.as_deref())
    .bind(is_exit)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "FOURRIERE_STATUS_CHANGED",
        "fourrieres",
        Some(id),
        Some(json!({ "status": existing.status })),
        Some(json!({ "status": status, "released_to": released_to })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_fourriere(&state.db, id).await?))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Types d'objets pouvant être mis en fourrière (véhicule par défaut).
const VALID_ITEM_TYPES: [&str; 5] = ["VEHICULE", "ENGIN", "MARCHANDISE", "ANIMAL", "AUTRE"];

fn normalize_item_type(requested: Option<&str>) -> Result<String, ApiError> {
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok("VEHICULE".to_string()),
        Some(value) if VALID_ITEM_TYPES.contains(&value) => Ok(value.to_string()),
        Some(other) => Err(ApiError::bad_request(format!(
            "item_type invalide: {other}"
        ))),
    }
}

async fn load_fourriere(pool: &PgPool, id: Uuid) -> Result<FourriereResponse, ApiError> {
    let row = sqlx::query(&format!(
        "SELECT {FOURRIERE_COLUMNS} FROM fourrieres WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Fourrière introuvable"))?;
    Ok(row_to_fourriere(row))
}

fn row_to_fourriere(row: sqlx::postgres::PgRow) -> FourriereResponse {
    let daily_fee_fcfa: i64 = row.get("daily_fee_fcfa");
    let entered_at: DateTime<Utc> = row.get("entered_at");
    let released_at: Option<DateTime<Utc>> = row.get("released_at");
    let frais_gardiennage_fcfa = daily_fee_fcfa.saturating_mul(days_held(entered_at, released_at));

    FourriereResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        pv_id: row.get("pv_id"),
        fourriere_number: row.get("fourriere_number"),
        item_type: row.get("item_type"),
        designation: row.get("designation"),
        vehicle_plate: row.get("vehicle_plate"),
        vehicle_type: row.get("vehicle_type"),
        vehicle_details: row.get("vehicle_details"),
        motif: row.get("motif"),
        lieu_enlevement: row.get("lieu_enlevement"),
        status: row.get("status"),
        daily_fee_fcfa,
        frais_gardiennage_fcfa,
        entered_at,
        released_at,
        released_to: row.get("released_to"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Nombre de jours de gardiennage facturables (au moins 1 jour entamé).
fn days_held(entered_at: DateTime<Utc>, released_at: Option<DateTime<Utc>>) -> i64 {
    let end = released_at.unwrap_or_else(Utc::now);
    let seconds = (end - entered_at).num_seconds().max(0);
    let days = (seconds as f64 / 86_400.0).ceil() as i64;
    days.max(1)
}

fn apply_filters(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    status: Option<&str>,
    vehicle_plate: Option<&str>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
    if let Some(plate) = vehicle_plate {
        qb.push(" AND vehicle_plate = ").push_bind(plate.to_string());
    }
}
