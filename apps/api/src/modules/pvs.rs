use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{is_agent_only, resolve_commune_filter, validate_gps};
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::sequences::{next_document_sequence, SEQUENCE_PV};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pvs", axum::routing::get(list_pvs).post(create_pv))
        .route("/pvs/{id}", axum::routing::get(get_pv).delete(cancel_pv))
        .route("/pvs/{id}/status", axum::routing::patch(patch_pv_status))
        .route("/pvs/{id}/qr", axum::routing::get(get_pv_qr))
        .route("/pvs/{id}/pdf", axum::routing::get(get_pv_pdf))
}

pub fn public_router() -> Router<AppState> {
    Router::new().route("/pvs/{pv_number}", axum::routing::get(verify_pv_public))
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PvResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub agent_id: Uuid,
    pub pv_number: String,
    pub intervention_id: Uuid,
    pub zone_id: Option<Uuid>,
    pub verbalized_name: Option<String>,
    pub verbalized_identifier: Option<String>,
    pub vehicle_plate: Option<String>,
    pub location_description: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub amount_initial: Option<f64>,
    pub amount_initial_fcfa: Option<i64>,
    pub status: String,
    pub notes_internes: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PvPublicResponse {
    pub pv_number: String,
    pub commune_id: Uuid,
    pub status: String,
    pub amount_initial: Option<f64>,
    pub amount_initial_fcfa: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PvStatusHistoryEntry {
    pub id: Uuid,
    pub old_status: Option<String>,
    pub new_status: String,
    pub changed_by: Uuid,
    pub changed_at: DateTime<Utc>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PvFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    agent_id: Option<Uuid>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePvRequest {
    pub intervention_id: Uuid,
    pub zone_id: Option<Uuid>,
    pub verbalized_name: Option<String>,
    pub verbalized_identifier: Option<String>,
    pub vehicle_plate: Option<String>,
    pub location_description: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub notes_internes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchStatusRequest {
    pub status: String,
    pub reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_pvs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PvFilterQuery>,
) -> Result<Json<Paginated<PvResponse>>, ApiError> {
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

    let agent_filter = if is_agent_only(&auth_user) {
        let agent_id = active_agent_id_for_user(&state.db, &auth_user).await?;
        if agent_id.is_none() {
            return Ok(Json(Paginated::new(Vec::new(), &pagination, 0)));
        }
        agent_id
    } else {
        query.agent_id
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM pvs WHERE deleted_at IS NULL");
    apply_pv_filters(
        &mut count_qb,
        commune_filter,
        agent_filter,
        query.status.as_deref(),
    );
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT
            id, commune_id, agent_id, pv_number, intervention_id, zone_id,
            verbalized_name, verbalized_identifier, vehicle_plate,
            location_description, gps_latitude, gps_longitude,
            amount_initial::DOUBLE PRECISION AS amount_initial,
            amount_initial_fcfa, status, notes_internes, created_by,
            created_at, updated_at
        FROM pvs
        WHERE deleted_at IS NULL
        "#,
    );
    apply_pv_filters(
        &mut qb,
        commune_filter,
        agent_filter,
        query.status.as_deref(),
    );
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_pv).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_pv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PvResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;
    require_pv_read_access(&state.db, &auth_user, &pv).await?;
    Ok(Json(pv))
}

async fn create_pv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreatePvRequest>,
) -> Result<(StatusCode, Json<PvResponse>), ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent])?;

    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Agent non rattache a une commune"))?;
    validate_gps(payload.gps_latitude, payload.gps_longitude)?;

    let mut tx = state.db.begin().await?;

    let agent_row = sqlx::query(
        "SELECT id, status FROM agents WHERE user_id = $1 AND commune_id = $2 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::forbidden("Agent introuvable pour cet utilisateur et cette commune"))?;

    let agent_status: String = agent_row.get("status");
    if agent_status != "ACTIF" {
        return Err(ApiError::forbidden("Seul un agent actif peut creer un PV"));
    }
    let agent_id: Uuid = agent_row.get("id");

    let intervention = sqlx::query(
        r#"
        SELECT
            id, commune_id, sujet_paiement,
            montant::DOUBLE PRECISION AS montant,
            montant_fcfa, delai_paiement_jours,
            taux_penalite::DOUBLE PRECISION AS taux_penalite,
            active
        FROM interventions
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(payload.intervention_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::not_found("Intervention introuvable"))?;

    let interv_commune: Uuid = intervention.get("commune_id");
    if interv_commune != commune_id {
        return Err(ApiError::forbidden(
            "L'intervention n'appartient pas a votre commune",
        ));
    }

    let active: bool = intervention.get("active");
    if !active {
        return Err(ApiError::bad_request(
            "L'intervention selectionnee est inactive",
        ));
    }

    let sujet_paiement: bool = intervention.get("sujet_paiement");
    let montant: Option<f64> = intervention.get("montant");
    let montant_fcfa: Option<i64> = intervention.get("montant_fcfa");

    let verb_id = payload.verbalized_identifier.as_deref();
    let plate = payload.vehicle_plate.as_deref();
    check_double_verbalisation(&mut tx, commune_id, payload.intervention_id, verb_id, plate)
        .await?;

    let commune_code: String = sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
        .bind(commune_id)
        .fetch_one(&mut *tx)
        .await?;
    let (year, seq) = next_document_sequence(&mut tx, commune_id, SEQUENCE_PV).await?;
    let pv_number = format!(
        "PV-{}-{}-{:06}",
        commune_code.to_uppercase().replace(' ', "-"),
        year,
        seq
    );

    let public_url = format!(
        "{}/api/v1/public/pvs/{}",
        state.config.public_api_url, pv_number
    );
    let qr_svg = generate_qr_svg(&public_url)?;

    let initial_status = if sujet_paiement {
        "EN_ATTENTE_PAIEMENT"
    } else {
        "NON_PAYANT"
    };

    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO pvs (
            id, commune_id, agent_id, pv_number, intervention_id, zone_id,
            verbalized_name, verbalized_identifier, vehicle_plate,
            location_description, gps_latitude, gps_longitude,
            amount_initial, amount_initial_fcfa, status, qr_code_svg, notes_internes, created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            $10, $11, $12,
            $13, $14, $15, $16, $17, $18
        )
        "#,
    )
    .bind(id)
    .bind(commune_id)
    .bind(agent_id)
    .bind(&pv_number)
    .bind(payload.intervention_id)
    .bind(payload.zone_id)
    .bind(clean_optional(payload.verbalized_name))
    .bind(clean_optional(payload.verbalized_identifier))
    .bind(clean_optional(payload.vehicle_plate))
    .bind(clean_optional(payload.location_description))
    .bind(payload.gps_latitude)
    .bind(payload.gps_longitude)
    .bind(montant)
    .bind(montant_fcfa)
    .bind(initial_status)
    .bind(&qr_svg)
    .bind(clean_optional(payload.notes_internes))
    .bind(auth_user.id)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    record_status_change_tx(&mut tx, id, None, initial_status, auth_user.id, None).await;

    audit::record_for_commune_tx(
        &mut tx,
        Some(commune_id),
        Some(auth_user.id),
        "PV_CREATED",
        "pvs",
        Some(id),
        None,
        Some(json!({
            "pv_number": pv_number,
            "commune_id": commune_id,
            "agent_id": agent_id,
            "intervention_id": payload.intervention_id,
            "status": initial_status
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(load_pv(&state.db, id).await?)))
}
async fn patch_pv_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchStatusRequest>,
) -> Result<Json<PvResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;

    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;

    validate_status_transition(&pv.status, &payload.status)?;

    sqlx::query(
        "UPDATE pvs SET status = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(&payload.status)
    .execute(&state.db)
    .await?;

    record_status_change(
        &state.db,
        id,
        Some(&pv.status),
        &payload.status,
        auth_user.id,
        payload.reason.as_deref(),
    )
    .await;

    audit::record_for_commune(
        &state.db,
        Some(pv.commune_id),
        Some(auth_user.id),
        "PV_STATUS_CHANGED",
        "pvs",
        Some(id),
        Some(json!({ "status": pv.status })),
        Some(json!({ "status": payload.status, "reason": payload.reason })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_pv(&state.db, id).await?))
}

async fn cancel_pv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;

    if pv.status == "PAYE" {
        return Err(ApiError::conflict("Un PV payé ne peut pas être annulé"));
    }
    if pv.status == "ANNULE" {
        return Err(ApiError::conflict("Ce PV est déjà annulé"));
    }

    sqlx::query(
        "UPDATE pvs SET status = 'ANNULE', deleted_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    record_status_change(
        &state.db,
        id,
        Some(&pv.status),
        "ANNULE",
        auth_user.id,
        None,
    )
    .await;

    audit::record_for_commune(
        &state.db,
        Some(pv.commune_id),
        Some(auth_user.id),
        "PV_CANCELLED",
        "pvs",
        Some(id),
        Some(json!({ "status": pv.status })),
        Some(json!({ "status": "ANNULE" })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "cancelled": true, "id": id })))
}

async fn get_pv_qr(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;
    require_pv_read_access(&state.db, &auth_user, &pv).await?;

    let svg: String = sqlx::query_scalar("SELECT qr_code_svg FROM pvs WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    use axum::http::header;
    use axum::response::IntoResponse;
    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

async fn get_pv_pdf(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;
    require_pv_read_access(&state.db, &auth_user, &pv).await?;

    let pdf_bytes = crate::modules::pdf::generate_pv_pdf(&state.db, &pv).await?;

    use axum::http::header;
    use axum::response::IntoResponse;
    let disposition = format!("attachment; filename=\"PV-{}.pdf\"", pv.pv_number);
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        pdf_bytes,
    )
        .into_response())
}

async fn verify_pv_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(pv_number): Path<String>,
) -> Result<Json<PvPublicResponse>, ApiError> {
    state.rate_limiter.check(
        "public:pvs:verify",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let row = sqlx::query(
        r#"
        SELECT
            id, commune_id, status,
            amount_initial::DOUBLE PRECISION AS amount_initial,
            amount_initial_fcfa, created_at
        FROM pvs
        WHERE pv_number = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(&pv_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("PV introuvable"))?;

    Ok(Json(PvPublicResponse {
        pv_number,
        commune_id: row.get("commune_id"),
        status: row.get("status"),
        amount_initial: row.get("amount_initial"),
        amount_initial_fcfa: row.get("amount_initial_fcfa"),
        created_at: row.get("created_at"),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_pv(pool: &PgPool, id: Uuid) -> Result<PvResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT
            id, commune_id, agent_id, pv_number, intervention_id, zone_id,
            verbalized_name, verbalized_identifier, vehicle_plate,
            location_description, gps_latitude, gps_longitude,
            amount_initial::DOUBLE PRECISION AS amount_initial,
            amount_initial_fcfa, status, notes_internes, created_by,
            created_at, updated_at
        FROM pvs
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("PV introuvable"))?;
    Ok(row_to_pv(row))
}

async fn require_pv_read_access(
    pool: &PgPool,
    auth_user: &AuthUser,
    pv: &PvResponse,
) -> Result<(), ApiError> {
    if !is_agent_only(auth_user) {
        return Ok(());
    }

    let agent_id = active_agent_id_for_user(pool, auth_user)
        .await?
        .ok_or_else(|| ApiError::forbidden("Agent actif introuvable pour cet utilisateur"))?;
    if pv.agent_id != agent_id {
        return Err(ApiError::forbidden("Acces refuse a ce PV"));
    }
    Ok(())
}

async fn active_agent_id_for_user(
    pool: &PgPool,
    auth_user: &AuthUser,
) -> Result<Option<Uuid>, ApiError> {
    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Agent non rattache a une commune"))?;
    let agent_id = sqlx::query_scalar(
        r#"
        SELECT id
        FROM agents
        WHERE user_id = $1
          AND commune_id = $2
          AND status = 'ACTIF'
          AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent_id)
}

fn row_to_pv(row: sqlx::postgres::PgRow) -> PvResponse {
    PvResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        agent_id: row.get("agent_id"),
        pv_number: row.get("pv_number"),
        intervention_id: row.get("intervention_id"),
        zone_id: row.get("zone_id"),
        verbalized_name: row.get("verbalized_name"),
        verbalized_identifier: row.get("verbalized_identifier"),
        vehicle_plate: row.get("vehicle_plate"),
        location_description: row.get("location_description"),
        gps_latitude: row.get("gps_latitude"),
        gps_longitude: row.get("gps_longitude"),
        amount_initial: row.get("amount_initial"),
        amount_initial_fcfa: row.get("amount_initial_fcfa"),
        status: row.get("status"),
        notes_internes: row.get("notes_internes"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Génère un numéro PV unique : PV-{CODE}-{YEAR}-{SEQ:06}
fn generate_qr_svg(data: &str) -> Result<String, ApiError> {
    use qrcode::render::svg;
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(data.as_bytes(), EcLevel::M)
        .map_err(|e| ApiError::internal(format!("QR code generation failed: {e}")))?;

    let svg_string = code.render::<svg::Color>().min_dimensions(200, 200).build();

    Ok(svg_string)
}

/// Vérifie qu'aucun PV actif similaire n'existe (double verbalisation).
async fn check_double_verbalisation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    intervention_id: Uuid,
    verbalized_identifier: Option<&str>,
    vehicle_plate: Option<&str>,
) -> Result<(), ApiError> {
    let bloquant: bool =
        sqlx::query_scalar("SELECT double_verbalisation_bloquant FROM communes WHERE id = $1")
            .bind(commune_id)
            .fetch_one(&mut **tx)
            .await?;

    if !bloquant {
        return Ok(());
    }

    if let Some(vid) = verbalized_identifier {
        if !vid.trim().is_empty() {
            lock_double_verbalisation(tx, commune_id, intervention_id, "identifier", vid).await?;
            let existing: Option<String> = sqlx::query_scalar(
                r#"
                SELECT pv_number FROM pvs
                WHERE commune_id = $1
                  AND intervention_id = $2
                  AND verbalized_identifier = $3
                  AND status NOT IN ('PAYE', 'ANNULE', 'NON_PAYANT')
                  AND deleted_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(commune_id)
            .bind(intervention_id)
            .bind(vid)
            .fetch_optional(&mut **tx)
            .await?;

            if let Some(pv_num) = existing {
                return Err(ApiError::conflict(format!(
                    "Double verbalisation detectee: PV {} existe deja pour cet identifiant",
                    pv_num
                )));
            }
        }
    }

    if let Some(plate) = vehicle_plate {
        if !plate.trim().is_empty() {
            lock_double_verbalisation(tx, commune_id, intervention_id, "plate", plate).await?;
            let existing: Option<String> = sqlx::query_scalar(
                r#"
                SELECT pv_number FROM pvs
                WHERE commune_id = $1
                  AND intervention_id = $2
                  AND vehicle_plate = $3
                  AND status NOT IN ('PAYE', 'ANNULE', 'NON_PAYANT')
                  AND deleted_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(commune_id)
            .bind(intervention_id)
            .bind(plate)
            .fetch_optional(&mut **tx)
            .await?;

            if let Some(pv_num) = existing {
                return Err(ApiError::conflict(format!(
                    "Double verbalisation detectee: PV {} existe deja pour cette plaque",
                    pv_num
                )));
            }
        }
    }

    Ok(())
}

async fn lock_double_verbalisation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    intervention_id: Uuid,
    kind: &str,
    value: &str,
) -> Result<(), ApiError> {
    let key = format!(
        "double-verbalisation:{commune_id}:{intervention_id}:{kind}:{}",
        value.trim().to_ascii_uppercase()
    );
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
fn validate_status_transition(current: &str, next: &str) -> Result<(), ApiError> {
    let allowed: &[&str] = match current {
        "BROUILLON" => &["EMIS", "ANNULE"],
        "EMIS" => &["EN_ATTENTE_PAIEMENT", "ANNULE"],
        "EN_ATTENTE_PAIEMENT" => &["PAYE", "EN_RETARD", "ANNULE", "CONTESTE"],
        "EN_RETARD" => &["PAYE", "ANNULE", "CONTESTE"],
        "CONTESTE" => &["EN_ATTENTE_PAIEMENT", "ANNULE"],
        "PAYE" | "ANNULE" | "NON_PAYANT" => &[],
        _ => &[],
    };

    if !allowed.contains(&next) {
        return Err(ApiError::bad_request(format!(
            "Transition de statut '{current}' → '{next}' non autorisée"
        )));
    }
    Ok(())
}

pub async fn record_status_change(
    pool: &PgPool,
    pv_id: Uuid,
    old_status: Option<&str>,
    new_status: &str,
    changed_by: Uuid,
    reason: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO pv_status_history (pv_id, old_status, new_status, changed_by, reason)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(pv_id)
    .bind(old_status)
    .bind(new_status)
    .bind(changed_by)
    .bind(reason)
    .execute(pool)
    .await;
}

pub async fn record_status_change_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pv_id: Uuid,
    old_status: Option<&str>,
    new_status: &str,
    changed_by: Uuid,
    reason: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO pv_status_history (pv_id, old_status, new_status, changed_by, reason)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(pv_id)
    .bind(old_status)
    .bind(new_status)
    .bind(changed_by)
    .bind(reason)
    .execute(&mut **tx)
    .await;
}

fn apply_pv_filters(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    agent_filter: Option<Uuid>,
    status: Option<&str>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(id) = agent_filter {
        qb.push(" AND agent_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// Ré-export pour payments
pub fn pv_due_date(created_at: DateTime<Utc>, delai_jours: i32) -> DateTime<Utc> {
    created_at + Duration::days(delai_jours as i64)
}
