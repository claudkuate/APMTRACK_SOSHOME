use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Duration, Utc};
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
        .route("/pvs", axum::routing::get(list_pvs).post(create_pv))
        .route(
            "/pvs/{id}",
            axum::routing::get(get_pv).delete(cancel_pv),
        )
        .route("/pvs/{id}/status", axum::routing::patch(patch_pv_status))
        .route("/pvs/{id}/qr", axum::routing::get(get_pv_qr))
        .route("/pvs/{id}/pdf", axum::routing::get(get_pv_pdf))
}

pub fn public_router() -> Router<AppState> {
    Router::new().route(
        "/pvs/{pv_number}",
        axum::routing::get(verify_pv_public),
    )
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

    // APM_AGENT voit seulement ses propres PV
    let agent_filter = if auth_user.has_role(Role::ApmAgent)
        && !auth_user.has_role(Role::SuperAdmin)
        && !auth_user.has_role(Role::AdminCommune)
        && !auth_user.has_role(Role::Superviseur)
        && !auth_user.has_role(Role::Receveur)
    {
        // On doit résoudre l'agent_id lié à cet utilisateur
        let agent_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM agents WHERE user_id = $1 AND deleted_at IS NULL LIMIT 1",
        )
        .bind(auth_user.id)
        .fetch_optional(&state.db)
        .await?;
        agent_id
    } else {
        query.agent_id
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM pvs WHERE deleted_at IS NULL");
    apply_pv_filters(&mut count_qb, commune_filter, agent_filter, query.status.as_deref());
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM pvs WHERE deleted_at IS NULL");
    apply_pv_filters(&mut qb, commune_filter, agent_filter, query.status.as_deref());
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

    // Vérifier que l'agent est actif
    let agent_row = sqlx::query(
        "SELECT id, status FROM agents WHERE user_id = $1 AND commune_id = $2 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::forbidden("Agent introuvable pour cet utilisateur et cette commune"))?;

    let agent_status: String = agent_row.get("status");
    if agent_status != "ACTIF" {
        return Err(ApiError::forbidden(
            "Seul un agent actif peut creer un PV",
        ));
    }
    let agent_id: Uuid = agent_row.get("id");

    // Charger l'intervention (doit appartenir à la commune)
    let intervention = sqlx::query(
        "SELECT id, commune_id, sujet_paiement, montant, delai_paiement_jours, taux_penalite, active FROM interventions WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(payload.intervention_id)
    .fetch_optional(&state.db)
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
        return Err(ApiError::bad_request("L'intervention selectionnee est inactive"));
    }

    let sujet_paiement: bool = intervention.get("sujet_paiement");
    let montant: Option<f64> = intervention.get("montant");

    // Vérifier double verbalisation
    let verb_id = payload.verbalized_identifier.as_deref();
    let plate = payload.vehicle_plate.as_deref();
    check_double_verbalisation(&state.db, commune_id, payload.intervention_id, verb_id, plate)
        .await?;

    // Générer le numéro PV
    let commune_code: String =
        sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
            .bind(commune_id)
            .fetch_one(&state.db)
            .await?;
    let pv_number = generate_pv_number(&state.db, &commune_code, commune_id).await?;

    // Générer le QR code (SVG)
    let public_url = format!(
        "{}/api/v1/public/pvs/{}",
        state.config.public_api_url,
        pv_number
    );
    let qr_svg = generate_qr_svg(&public_url)?;

    // Statut initial
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
            amount_initial, status, qr_code_svg, notes_internes, created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            $10, $11, $12,
            $13, $14, $15, $16, $17
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
    .bind(initial_status)
    .bind(&qr_svg)
    .bind(clean_optional(payload.notes_internes))
    .bind(auth_user.id)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    // Enregistrer l'historique initial
    record_status_change(&state.db, id, None, initial_status, auth_user.id, None).await;

    audit::record(
        &state.db,
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

    audit::record(
        &state.db,
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

    record_status_change(&state.db, id, Some(&pv.status), "ANNULE", auth_user.id, None).await;

    audit::record(
        &state.db,
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

    let svg: String = sqlx::query_scalar("SELECT qr_code_svg FROM pvs WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    use axum::http::header;
    use axum::response::IntoResponse;
    Ok((
        [(header::CONTENT_TYPE, "image/svg+xml")],
        svg,
    )
        .into_response())
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
    Path(pv_number): Path<String>,
) -> Result<Json<PvPublicResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT id, commune_id, status, amount_initial, created_at FROM pvs WHERE pv_number = $1 AND deleted_at IS NULL",
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
        created_at: row.get("created_at"),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_pv(pool: &PgPool, id: Uuid) -> Result<PvResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM pvs WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("PV introuvable"))?;
    Ok(row_to_pv(row))
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
        status: row.get("status"),
        notes_internes: row.get("notes_internes"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Génère un numéro PV unique : PV-{CODE}-{YEAR}-{SEQ:06}
async fn generate_pv_number(pool: &PgPool, commune_code: &str, commune_id: Uuid) -> Result<String, ApiError> {
    let year = Utc::now().year();
    let seq: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM pvs WHERE commune_id = $1 AND EXTRACT(YEAR FROM created_at) = $2",
    )
    .bind(commune_id)
    .bind(year as i64)
    .fetch_one(pool)
    .await?;

    let code = commune_code.to_uppercase().replace(' ', "-");
    Ok(format!("PV-{}-{}-{:06}", code, year, seq))
}

/// Génère un SVG de QR code sans dépendance externe lourde.
fn generate_qr_svg(data: &str) -> Result<String, ApiError> {
    use qrcode::{QrCode, EcLevel};
    use qrcode::render::svg;

    let code = QrCode::with_error_correction_level(data.as_bytes(), EcLevel::M)
        .map_err(|e| ApiError::internal(format!("QR code generation failed: {e}")))?;

    let svg_string = code
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .build();

    Ok(svg_string)
}

/// Vérifie qu'aucun PV actif similaire n'existe (double verbalisation).
async fn check_double_verbalisation(
    pool: &PgPool,
    commune_id: Uuid,
    intervention_id: Uuid,
    verbalized_identifier: Option<&str>,
    vehicle_plate: Option<&str>,
) -> Result<(), ApiError> {
    // Vérifier la config commune
    let bloquant: bool = sqlx::query_scalar(
        "SELECT double_verbalisation_bloquant FROM communes WHERE id = $1",
    )
    .bind(commune_id)
    .fetch_one(pool)
    .await?;

    if !bloquant {
        return Ok(());
    }

    if let Some(vid) = verbalized_identifier {
        if !vid.trim().is_empty() {
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
            .fetch_optional(pool)
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
            .fetch_optional(pool)
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

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// Ré-export pour payments
pub fn pv_due_date(created_at: DateTime<Utc>, delai_jours: i32) -> DateTime<Utc> {
    created_at + Duration::days(delai_jours as i64)
}
