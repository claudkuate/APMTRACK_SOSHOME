use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
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
        .route(
            "/pvs/{id}",
            axum::routing::get(get_pv).patch(patch_pv).delete(cancel_pv),
        )
        .route("/pvs/{id}/status", axum::routing::patch(patch_pv_status))
        .route("/pvs/{id}/qr", axum::routing::get(get_pv_qr))
        .route("/pvs/{id}/pdf", axum::routing::get(get_pv_pdf))
        .route(
            "/pvs/{id}/photos",
            axum::routing::get(list_pv_photos)
                .post(upload_pv_photo)
                .layer(DefaultBodyLimit::max(MAX_PHOTO_BYTES)),
        )
        .route(
            "/pvs/{id}/photos/{photo_id}",
            axum::routing::get(get_pv_photo_content).delete(delete_pv_photo),
        )
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
    pub interventions: Vec<PvInterventionResponse>,
    pub subject_type: String,
    pub subject_kind: String,
    pub raison_sociale: Option<String>,
    pub zone_id: Option<Uuid>,
    pub verbalized_name: Option<String>,
    pub verbalized_identifier: Option<String>,
    pub verbalized_first_name: Option<String>,
    pub verbalized_last_name: Option<String>,
    pub verbalized_identity_type: Option<String>,
    pub verbalized_identity_number: Option<String>,
    pub verbalized_phone: Option<String>,
    pub verbalized_address: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_registration_card_number: Option<String>,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_color: Option<String>,
    pub vehicle_owner_name: Option<String>,
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

#[derive(Debug, Serialize, Clone)]
pub struct PvInterventionResponse {
    pub id: Uuid,
    pub intervention_id: Uuid,
    pub order_index: i32,
    pub nom: String,
    pub sujet_paiement: bool,
    pub montant_fcfa: Option<i64>,
    pub delai_paiement_jours: Option<i32>,
    pub taux_penalite: Option<f64>,
    pub taux_penalite_basis_points: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PvPublicResponse {
    pub pv_number: String,
    pub commune_nom: String,
    pub status: String,
    /// Matricule de l'agent ayant dressé le PV (« Dressé par »).
    pub agent_matricule: Option<String>,
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
    pub intervention_id: Option<Uuid>,
    pub intervention_ids: Option<Vec<Uuid>>,
    pub subject_type: Option<String>,
    pub subject_kind: Option<String>,
    pub raison_sociale: Option<String>,
    pub zone_id: Option<Uuid>,
    pub verbalized_name: Option<String>,
    pub verbalized_identifier: Option<String>,
    pub verbalized_first_name: Option<String>,
    pub verbalized_last_name: Option<String>,
    pub verbalized_identity_type: Option<String>,
    pub verbalized_identity_number: Option<String>,
    pub verbalized_phone: Option<String>,
    pub verbalized_address: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_registration_card_number: Option<String>,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_color: Option<String>,
    pub vehicle_owner_name: Option<String>,
    pub location_description: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub notes_internes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchPvRequest {
    pub intervention_id: Option<Uuid>,
    pub intervention_ids: Option<Vec<Uuid>>,
    pub subject_type: Option<String>,
    pub subject_kind: Option<String>,
    pub raison_sociale: Option<String>,
    pub zone_id: Option<Uuid>,
    pub verbalized_name: Option<String>,
    pub verbalized_identifier: Option<String>,
    pub verbalized_first_name: Option<String>,
    pub verbalized_last_name: Option<String>,
    pub verbalized_identity_type: Option<String>,
    pub verbalized_identity_number: Option<String>,
    pub verbalized_phone: Option<String>,
    pub verbalized_address: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_registration_card_number: Option<String>,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_color: Option<String>,
    pub vehicle_owner_name: Option<String>,
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
            subject_type, subject_kind, raison_sociale,
            verbalized_name, verbalized_identifier,
            verbalized_first_name, verbalized_last_name, verbalized_identity_type,
            verbalized_identity_number, verbalized_phone, verbalized_address,
            vehicle_plate, vehicle_registration_card_number,
            vehicle_make, vehicle_model, vehicle_color, vehicle_owner_name,
            location_description,
            gps_latitude::double precision AS gps_latitude,
            gps_longitude::double precision AS gps_longitude,
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
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut pv = row_to_pv(row);
        pv.interventions = load_pv_interventions(&state.db, pv.id).await?;
        items.push(pv);
    }
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
    let intervention_ids =
        normalize_intervention_ids(payload.intervention_id, payload.intervention_ids)?;
    let verbalized_first_name = clean_optional(payload.verbalized_first_name);
    let verbalized_last_name = clean_optional(payload.verbalized_last_name);
    let verbalized_name = compose_verbalized_name(
        clean_optional(payload.verbalized_name),
        verbalized_first_name.as_deref(),
        verbalized_last_name.as_deref(),
    );
    let explicit_identity_number = clean_optional(payload.verbalized_identity_number);
    let legacy_identifier = clean_optional(payload.verbalized_identifier);
    let identity_from_legacy = explicit_identity_number.is_none() && legacy_identifier.is_some();
    let verbalized_identity_number = explicit_identity_number.or(legacy_identifier);
    let verbalized_identifier = verbalized_identity_number.clone();
    let verbalized_identity_type = normalize_identity_type(
        clean_optional(payload.verbalized_identity_type),
        verbalized_identity_number.as_deref(),
        identity_from_legacy,
    )?;
    let verbalized_phone = clean_optional(payload.verbalized_phone);
    let verbalized_address = clean_optional(payload.verbalized_address);
    let vehicle_plate =
        clean_optional(payload.vehicle_plate).map(|plate| plate.to_ascii_uppercase());
    let vehicle_registration_card_number = clean_optional(payload.vehicle_registration_card_number)
        .map(|number| number.to_ascii_uppercase());
    let vehicle_make = clean_optional(payload.vehicle_make);
    let vehicle_model = clean_optional(payload.vehicle_model);
    let vehicle_color = clean_optional(payload.vehicle_color);
    let vehicle_owner_name = clean_optional(payload.vehicle_owner_name);
    let location_description = clean_optional(payload.location_description);
    let notes_internes = clean_optional(payload.notes_internes);
    let raison_sociale = clean_optional(payload.raison_sociale);
    let subject_kind = normalize_subject_kind(payload.subject_kind.as_deref())?;
    if subject_kind == "MORALE" && raison_sociale.is_none() {
        return Err(ApiError::bad_request(
            "La raison sociale est requise pour une personne morale",
        ));
    }
    // Pour une personne morale, la raison sociale tient lieu de nom du contrevenant.
    let verbalized_name = if subject_kind == "MORALE" {
        raison_sociale.clone()
    } else {
        verbalized_name
    };
    let subject_type = normalize_subject_type(
        payload.subject_type.as_deref(),
        verbalized_name.as_deref(),
        verbalized_identity_number.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
    )?;
    validate_subject_fields(
        &subject_type,
        verbalized_name.as_deref(),
        verbalized_identity_type.as_deref(),
        verbalized_identity_number.as_deref(),
        verbalized_phone.as_deref(),
        verbalized_address.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
        vehicle_make.as_deref(),
        vehicle_model.as_deref(),
        vehicle_color.as_deref(),
        vehicle_owner_name.as_deref(),
    )?;

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

    let interventions =
        load_intervention_snapshots_tx(&mut tx, commune_id, &intervention_ids).await?;
    check_double_verbalisation(
        &mut tx,
        commune_id,
        &intervention_ids,
        verbalized_identity_number.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
        None,
    )
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

    let amount_initial_fcfa = total_amount_fcfa(&interventions);
    let amount_initial = amount_initial_fcfa.map(|amount| amount as f64);
    let initial_status = if amount_initial_fcfa.unwrap_or(0) > 0 {
        "EN_ATTENTE_PAIEMENT"
    } else {
        "NON_PAYANT"
    };

    // Auto-résolution de la zone par point-dans-polygone si non fournie explicitement.
    let resolved_zone_id = match payload.zone_id {
        Some(zone_id) => Some(zone_id),
        None => {
            resolve_zone_from_point(
                &mut tx,
                commune_id,
                payload.gps_latitude,
                payload.gps_longitude,
            )
            .await?
        }
    };

    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO pvs (
            id, commune_id, agent_id, pv_number, intervention_id, subject_type, zone_id,
            verbalized_name, verbalized_identifier,
            verbalized_first_name, verbalized_last_name, verbalized_identity_type,
            verbalized_identity_number, verbalized_phone, verbalized_address,
            vehicle_plate, vehicle_registration_card_number,
            vehicle_make, vehicle_model, vehicle_color, vehicle_owner_name,
            location_description, gps_latitude, gps_longitude,
            amount_initial, amount_initial_fcfa, status, qr_code_svg, notes_internes, created_by,
            subject_kind, raison_sociale
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            $8, $9,
            $10, $11, $12,
            $13, $14, $15,
            $16, $17,
            $18, $19, $20, $21,
            $22, $23, $24,
            $25, $26, $27, $28, $29, $30,
            $31, $32
        )
        "#,
    )
    .bind(id)
    .bind(commune_id)
    .bind(agent_id)
    .bind(&pv_number)
    .bind(intervention_ids[0])
    .bind(&subject_type)
    .bind(resolved_zone_id)
    .bind(verbalized_name.clone())
    .bind(verbalized_identifier.clone())
    .bind(verbalized_first_name.clone())
    .bind(verbalized_last_name.clone())
    .bind(verbalized_identity_type.clone())
    .bind(verbalized_identity_number.clone())
    .bind(verbalized_phone.clone())
    .bind(verbalized_address.clone())
    .bind(vehicle_plate.clone())
    .bind(vehicle_registration_card_number.clone())
    .bind(vehicle_make.clone())
    .bind(vehicle_model.clone())
    .bind(vehicle_color.clone())
    .bind(vehicle_owner_name.clone())
    .bind(location_description.clone())
    .bind(payload.gps_latitude)
    .bind(payload.gps_longitude)
    .bind(amount_initial)
    .bind(amount_initial_fcfa)
    .bind(initial_status)
    .bind(&qr_svg)
    .bind(notes_internes.clone())
    .bind(auth_user.id)
    .bind(&subject_kind)
    .bind(raison_sociale.clone())
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    replace_pv_interventions_tx(&mut tx, id, &interventions).await?;
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
            "intervention_ids": intervention_ids,
            "subject_type": subject_type,
            "status": initial_status
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(load_pv(&state.db, id).await?)))
}

async fn patch_pv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchPvRequest>,
) -> Result<Json<PvResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
    ])?;

    validate_gps(payload.gps_latitude, payload.gps_longitude)?;

    let mut tx = state.db.begin().await?;
    let existing = load_pv_for_update_tx(&mut tx, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;
    require_pv_write_access(&mut tx, &auth_user, &existing).await?;
    ensure_pv_editable(&existing.status)?;

    let intervention_ids = match payload.intervention_ids {
        Some(ids) => normalize_intervention_ids(payload.intervention_id, Some(ids))?,
        None => match payload.intervention_id {
            Some(id) => normalize_intervention_ids(Some(id), None)?,
            None => existing
                .interventions
                .iter()
                .map(|item| item.intervention_id)
                .collect(),
        },
    };
    if intervention_ids.is_empty() {
        return Err(ApiError::bad_request("Au moins une infraction est requise"));
    }

    let verbalized_first_name = clean_optional(payload.verbalized_first_name);
    let verbalized_last_name = clean_optional(payload.verbalized_last_name);
    let verbalized_name = compose_verbalized_name(
        clean_optional(payload.verbalized_name),
        verbalized_first_name.as_deref(),
        verbalized_last_name.as_deref(),
    );
    let explicit_identity_number = clean_optional(payload.verbalized_identity_number);
    let legacy_identifier = clean_optional(payload.verbalized_identifier);
    let identity_from_legacy = explicit_identity_number.is_none() && legacy_identifier.is_some();
    let verbalized_identity_number = explicit_identity_number.or(legacy_identifier);
    let verbalized_identifier = verbalized_identity_number.clone();
    let verbalized_identity_type = normalize_identity_type(
        clean_optional(payload.verbalized_identity_type),
        verbalized_identity_number.as_deref(),
        identity_from_legacy,
    )?;
    let verbalized_phone = clean_optional(payload.verbalized_phone);
    let verbalized_address = clean_optional(payload.verbalized_address);
    let vehicle_plate =
        clean_optional(payload.vehicle_plate).map(|plate| plate.to_ascii_uppercase());
    let vehicle_registration_card_number = clean_optional(payload.vehicle_registration_card_number)
        .map(|number| number.to_ascii_uppercase());
    let vehicle_make = clean_optional(payload.vehicle_make);
    let vehicle_model = clean_optional(payload.vehicle_model);
    let vehicle_color = clean_optional(payload.vehicle_color);
    let vehicle_owner_name = clean_optional(payload.vehicle_owner_name);
    let location_description = clean_optional(payload.location_description);
    let notes_internes = clean_optional(payload.notes_internes);
    let raison_sociale = clean_optional(payload.raison_sociale);
    let subject_kind = normalize_subject_kind(payload.subject_kind.as_deref())?;
    if subject_kind == "MORALE" && raison_sociale.is_none() {
        return Err(ApiError::bad_request(
            "La raison sociale est requise pour une personne morale",
        ));
    }
    // Pour une personne morale, la raison sociale tient lieu de nom du contrevenant.
    let verbalized_name = if subject_kind == "MORALE" {
        raison_sociale.clone()
    } else {
        verbalized_name
    };
    let subject_type = normalize_subject_type(
        payload.subject_type.as_deref(),
        verbalized_name.as_deref(),
        verbalized_identity_number.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
    )?;
    validate_subject_fields(
        &subject_type,
        verbalized_name.as_deref(),
        verbalized_identity_type.as_deref(),
        verbalized_identity_number.as_deref(),
        verbalized_phone.as_deref(),
        verbalized_address.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
        vehicle_make.as_deref(),
        vehicle_model.as_deref(),
        vehicle_color.as_deref(),
        vehicle_owner_name.as_deref(),
    )?;

    let interventions =
        load_intervention_snapshots_tx(&mut tx, existing.commune_id, &intervention_ids).await?;
    check_double_verbalisation(
        &mut tx,
        existing.commune_id,
        &intervention_ids,
        verbalized_identity_number.as_deref(),
        vehicle_plate.as_deref(),
        vehicle_registration_card_number.as_deref(),
        Some(id),
    )
    .await?;
    let amount_initial_fcfa = total_amount_fcfa(&interventions);
    let amount_initial = amount_initial_fcfa.map(|amount| amount as f64);
    let next_status = if amount_initial_fcfa.unwrap_or(0) > 0 {
        match existing.status.as_str() {
            "NON_PAYANT" => "EN_ATTENTE_PAIEMENT",
            current => current,
        }
    } else {
        "NON_PAYANT"
    };

    let resolved_zone_id = match payload.zone_id {
        Some(zone_id) => Some(zone_id),
        None => {
            resolve_zone_from_point(
                &mut tx,
                existing.commune_id,
                payload.gps_latitude,
                payload.gps_longitude,
            )
            .await?
        }
    };

    sqlx::query(
        r#"
        UPDATE pvs
        SET intervention_id = $2,
            subject_type = $3,
            zone_id = $4,
            verbalized_name = $5,
            verbalized_identifier = $6,
            verbalized_first_name = $7,
            verbalized_last_name = $8,
            verbalized_identity_type = $9,
            verbalized_identity_number = $10,
            verbalized_phone = $11,
            verbalized_address = $12,
            vehicle_plate = $13,
            vehicle_registration_card_number = $14,
            vehicle_make = $15,
            vehicle_model = $16,
            vehicle_color = $17,
            vehicle_owner_name = $18,
            location_description = $19,
            gps_latitude = $20,
            gps_longitude = $21,
            amount_initial = $22,
            amount_initial_fcfa = $23,
            status = $24,
            notes_internes = $25,
            subject_kind = $26,
            raison_sociale = $27,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(intervention_ids[0])
    .bind(&subject_type)
    .bind(resolved_zone_id)
    .bind(verbalized_name.clone())
    .bind(verbalized_identifier.clone())
    .bind(verbalized_first_name.clone())
    .bind(verbalized_last_name.clone())
    .bind(verbalized_identity_type.clone())
    .bind(verbalized_identity_number.clone())
    .bind(verbalized_phone.clone())
    .bind(verbalized_address.clone())
    .bind(vehicle_plate.clone())
    .bind(vehicle_registration_card_number.clone())
    .bind(vehicle_make.clone())
    .bind(vehicle_model.clone())
    .bind(vehicle_color.clone())
    .bind(vehicle_owner_name.clone())
    .bind(location_description.clone())
    .bind(payload.gps_latitude)
    .bind(payload.gps_longitude)
    .bind(amount_initial)
    .bind(amount_initial_fcfa)
    .bind(next_status)
    .bind(notes_internes.clone())
    .bind(&subject_kind)
    .bind(raison_sociale.clone())
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    replace_pv_interventions_tx(&mut tx, id, &interventions).await?;
    if next_status != existing.status {
        record_status_change_tx(
            &mut tx,
            id,
            Some(&existing.status),
            next_status,
            auth_user.id,
            Some("Mise a jour des infractions"),
        )
        .await;
    }

    audit::record_for_commune_tx(
        &mut tx,
        Some(existing.commune_id),
        Some(auth_user.id),
        "PV_UPDATED",
        "pvs",
        Some(id),
        Some(json!({
            "status": existing.status,
            "amount_initial_fcfa": existing.amount_initial_fcfa
        })),
        Some(json!({
            "intervention_ids": intervention_ids,
            "subject_type": subject_type,
            "amount_initial_fcfa": amount_initial_fcfa,
            "status": next_status
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    tx.commit().await?;
    Ok(Json(load_pv(&state.db, id).await?))
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
            p.status,
            p.amount_initial::DOUBLE PRECISION AS amount_initial,
            p.amount_initial_fcfa, p.created_at,
            c.nom AS commune_nom,
            a.matricule AS agent_matricule
        FROM pvs p
        INNER JOIN communes c ON c.id = p.commune_id
        LEFT JOIN agents a ON a.id = p.agent_id
        WHERE p.pv_number = $1 AND p.deleted_at IS NULL
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(&pv_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("PV introuvable"))?;

    Ok(Json(PvPublicResponse {
        pv_number,
        commune_nom: row.get("commune_nom"),
        status: row.get("status"),
        agent_matricule: row.get("agent_matricule"),
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
            subject_type, subject_kind, raison_sociale,
            verbalized_name, verbalized_identifier,
            verbalized_first_name, verbalized_last_name, verbalized_identity_type,
            verbalized_identity_number, verbalized_phone, verbalized_address,
            vehicle_plate, vehicle_registration_card_number,
            vehicle_make, vehicle_model, vehicle_color, vehicle_owner_name,
            location_description,
            gps_latitude::double precision AS gps_latitude,
            gps_longitude::double precision AS gps_longitude,
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
    let mut pv = row_to_pv(row);
    pv.interventions = load_pv_interventions(pool, pv.id).await?;
    Ok(pv)
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

/// Trouve la zone (de la commune) contenant le point GPS, si les coordonnées sont fournies
/// et qu'une zone dotée d'un contour les englobe. Renvoie `None` sinon.
async fn resolve_zone_from_point(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<Option<Uuid>, ApiError> {
    let (Some(lat), Some(lon)) = (latitude, longitude) else {
        return Ok(None);
    };
    let zone_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM zones
        WHERE commune_id = $1
          AND deleted_at IS NULL
          AND boundary IS NOT NULL
          AND ST_Contains(boundary, ST_SetSRID(ST_MakePoint($2, $3), 4326))
        ORDER BY ST_Area(boundary) ASC
        LIMIT 1
        "#,
    )
    .bind(commune_id)
    .bind(lon)
    .bind(lat)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(zone_id)
}

fn row_to_pv(row: sqlx::postgres::PgRow) -> PvResponse {
    PvResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        agent_id: row.get("agent_id"),
        pv_number: row.get("pv_number"),
        intervention_id: row.get("intervention_id"),
        interventions: Vec::new(),
        subject_type: row.get("subject_type"),
        subject_kind: row.get("subject_kind"),
        raison_sociale: row.get("raison_sociale"),
        zone_id: row.get("zone_id"),
        verbalized_name: row.get("verbalized_name"),
        verbalized_identifier: row.get("verbalized_identifier"),
        verbalized_first_name: row.get("verbalized_first_name"),
        verbalized_last_name: row.get("verbalized_last_name"),
        verbalized_identity_type: row.get("verbalized_identity_type"),
        verbalized_identity_number: row.get("verbalized_identity_number"),
        verbalized_phone: row.get("verbalized_phone"),
        verbalized_address: row.get("verbalized_address"),
        vehicle_plate: row.get("vehicle_plate"),
        vehicle_registration_card_number: row.get("vehicle_registration_card_number"),
        vehicle_make: row.get("vehicle_make"),
        vehicle_model: row.get("vehicle_model"),
        vehicle_color: row.get("vehicle_color"),
        vehicle_owner_name: row.get("vehicle_owner_name"),
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
#[derive(Debug, Clone)]
struct InterventionSnapshot {
    intervention_id: Uuid,
    order_index: i32,
    nom: String,
    sujet_paiement: bool,
    montant_fcfa: Option<i64>,
    delai_paiement_jours: Option<i32>,
    taux_penalite: Option<f64>,
    taux_penalite_basis_points: Option<i32>,
}

async fn load_pv_for_update_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<PvResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT
            id, commune_id, agent_id, pv_number, intervention_id, zone_id,
            subject_type, subject_kind, raison_sociale,
            verbalized_name, verbalized_identifier,
            verbalized_first_name, verbalized_last_name, verbalized_identity_type,
            verbalized_identity_number, verbalized_phone, verbalized_address,
            vehicle_plate, vehicle_registration_card_number,
            vehicle_make, vehicle_model, vehicle_color, vehicle_owner_name,
            location_description,
            gps_latitude::double precision AS gps_latitude,
            gps_longitude::double precision AS gps_longitude,
            amount_initial::DOUBLE PRECISION AS amount_initial,
            amount_initial_fcfa, status, notes_internes, created_by,
            created_at, updated_at
        FROM pvs
        WHERE id = $1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ApiError::not_found("PV introuvable"))?;

    let mut pv = row_to_pv(row);
    pv.interventions = load_pv_interventions_tx(tx, id).await?;
    Ok(pv)
}

async fn load_pv_interventions(
    pool: &PgPool,
    pv_id: Uuid,
) -> Result<Vec<PvInterventionResponse>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, intervention_id, order_index, nom, sujet_paiement, montant_fcfa,
            delai_paiement_jours,
            taux_penalite::DOUBLE PRECISION AS taux_penalite,
            taux_penalite_basis_points
        FROM pv_interventions
        WHERE pv_id = $1
        ORDER BY order_index ASC
        "#,
    )
    .bind(pv_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_pv_intervention).collect())
}

async fn load_pv_interventions_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pv_id: Uuid,
) -> Result<Vec<PvInterventionResponse>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, intervention_id, order_index, nom, sujet_paiement, montant_fcfa,
            delai_paiement_jours,
            taux_penalite::DOUBLE PRECISION AS taux_penalite,
            taux_penalite_basis_points
        FROM pv_interventions
        WHERE pv_id = $1
        ORDER BY order_index ASC
        "#,
    )
    .bind(pv_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().map(row_to_pv_intervention).collect())
}

fn row_to_pv_intervention(row: sqlx::postgres::PgRow) -> PvInterventionResponse {
    PvInterventionResponse {
        id: row.get("id"),
        intervention_id: row.get("intervention_id"),
        order_index: row.get("order_index"),
        nom: row.get("nom"),
        sujet_paiement: row.get("sujet_paiement"),
        montant_fcfa: row.get("montant_fcfa"),
        delai_paiement_jours: row.get("delai_paiement_jours"),
        taux_penalite: row.get("taux_penalite"),
        taux_penalite_basis_points: row.get("taux_penalite_basis_points"),
    }
}

async fn load_intervention_snapshots_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    intervention_ids: &[Uuid],
) -> Result<Vec<InterventionSnapshot>, ApiError> {
    let mut snapshots = Vec::with_capacity(intervention_ids.len());
    for (index, intervention_id) in intervention_ids.iter().enumerate() {
        let row = sqlx::query(
            r#"
            SELECT
                id, commune_id, nom, sujet_paiement, montant_fcfa,
                delai_paiement_jours,
                taux_penalite::DOUBLE PRECISION AS taux_penalite,
                taux_penalite_basis_points,
                active
            FROM interventions
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(intervention_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::not_found("Intervention introuvable"))?;

        let interv_commune: Uuid = row.get("commune_id");
        if interv_commune != commune_id {
            return Err(ApiError::forbidden(
                "Une intervention n'appartient pas a votre commune",
            ));
        }
        let active: bool = row.get("active");
        if !active {
            return Err(ApiError::bad_request(
                "Une intervention selectionnee est inactive",
            ));
        }

        snapshots.push(InterventionSnapshot {
            intervention_id: row.get("id"),
            order_index: index as i32,
            nom: row.get("nom"),
            sujet_paiement: row.get("sujet_paiement"),
            montant_fcfa: row.get("montant_fcfa"),
            delai_paiement_jours: row.get("delai_paiement_jours"),
            taux_penalite: row.get("taux_penalite"),
            taux_penalite_basis_points: row.get("taux_penalite_basis_points"),
        });
    }
    Ok(snapshots)
}

async fn replace_pv_interventions_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pv_id: Uuid,
    interventions: &[InterventionSnapshot],
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM pv_interventions WHERE pv_id = $1")
        .bind(pv_id)
        .execute(&mut **tx)
        .await?;

    for item in interventions {
        sqlx::query(
            r#"
            INSERT INTO pv_interventions (
                pv_id, intervention_id, order_index, nom, sujet_paiement,
                montant_fcfa, delai_paiement_jours, taux_penalite,
                taux_penalite_basis_points
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(pv_id)
        .bind(item.intervention_id)
        .bind(item.order_index)
        .bind(&item.nom)
        .bind(item.sujet_paiement)
        .bind(item.montant_fcfa)
        .bind(item.delai_paiement_jours)
        .bind(item.taux_penalite)
        .bind(item.taux_penalite_basis_points)
        .execute(&mut **tx)
        .await
        .map_err(map_database_error)?;
    }
    Ok(())
}

fn total_amount_fcfa(interventions: &[InterventionSnapshot]) -> Option<i64> {
    let total = interventions
        .iter()
        .filter(|item| item.sujet_paiement)
        .map(|item| item.montant_fcfa.unwrap_or(0))
        .sum::<i64>();
    (total > 0).then_some(total)
}

fn normalize_intervention_ids(
    primary: Option<Uuid>,
    ids: Option<Vec<Uuid>>,
) -> Result<Vec<Uuid>, ApiError> {
    let source = ids.unwrap_or_else(|| primary.into_iter().collect());
    let mut normalized = Vec::with_capacity(source.len());
    for id in source {
        if !normalized.contains(&id) {
            normalized.push(id);
        }
    }
    if normalized.is_empty() {
        return Err(ApiError::bad_request("Au moins une infraction est requise"));
    }
    Ok(normalized)
}

const IDENTITY_TYPES: &[&str] = &[
    "CNI",
    "PASSEPORT",
    "PERMIS_CONDUIRE",
    "CARTE_SEJOUR",
    "NIU",
    "AUTRE",
];

fn compose_verbalized_name(
    legacy_name: Option<String>,
    first_name: Option<&str>,
    last_name: Option<&str>,
) -> Option<String> {
    let composed = [first_name, last_name]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if composed.is_empty() {
        legacy_name
    } else {
        Some(composed)
    }
}

fn normalize_identity_type(
    value: Option<String>,
    identity_number: Option<&str>,
    identity_from_legacy: bool,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        if identity_number.is_some() {
            if identity_from_legacy {
                return Ok(Some("AUTRE".to_string()));
            }
            return Err(ApiError::bad_request(
                "Le type d'identite est requis avec le numero d'identite",
            ));
        }
        return Ok(None);
    };
    let normalized = value
        .trim()
        .to_ascii_uppercase()
        .replace(' ', "_")
        .replace('-', "_");
    if IDENTITY_TYPES.contains(&normalized.as_str()) {
        Ok(Some(normalized))
    } else {
        Err(ApiError::bad_request(format!(
            "type d'identite invalide: {value}"
        )))
    }
}

fn has_person_identity(verbalized_name: Option<&str>, identity_number: Option<&str>) -> bool {
    verbalized_name.is_some() || identity_number.is_some()
}

fn has_vehicle_identity(
    vehicle_plate: Option<&str>,
    registration_card_number: Option<&str>,
) -> bool {
    vehicle_plate.is_some() || registration_card_number.is_some()
}

fn has_vehicle_any(
    vehicle_plate: Option<&str>,
    registration_card_number: Option<&str>,
    make: Option<&str>,
    model: Option<&str>,
    color: Option<&str>,
    owner_name: Option<&str>,
) -> bool {
    vehicle_plate.is_some()
        || registration_card_number.is_some()
        || make.is_some()
        || model.is_some()
        || color.is_some()
        || owner_name.is_some()
}

/// Normalise le type de personne du contrevenant (physique par défaut / morale).
fn normalize_subject_kind(requested: Option<&str>) -> Result<String, ApiError> {
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("PHYSIQUE") => Ok("PHYSIQUE".to_string()),
        Some("MORALE") => Ok("MORALE".to_string()),
        Some(other) => Err(ApiError::bad_request(format!(
            "subject_kind invalide: {other}"
        ))),
    }
}

fn normalize_subject_type(
    requested: Option<&str>,
    verbalized_name: Option<&str>,
    verbalized_identity_number: Option<&str>,
    vehicle_plate: Option<&str>,
    vehicle_registration_card_number: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(value) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return match value {
            "PERSON_ONLY" | "VEHICLE_ONLY" | "PERSON_WITH_VEHICLE" => Ok(value.to_string()),
            other => Err(ApiError::bad_request(format!(
                "subject_type invalide: {other}"
            ))),
        };
    }

    let has_person = has_person_identity(verbalized_name, verbalized_identity_number);
    let has_vehicle = has_vehicle_identity(vehicle_plate, vehicle_registration_card_number);
    match (has_person, has_vehicle) {
        (true, true) => Ok("PERSON_WITH_VEHICLE".to_string()),
        (true, false) => Ok("PERSON_ONLY".to_string()),
        (false, true) => Ok("VEHICLE_ONLY".to_string()),
        (false, false) => Err(ApiError::bad_request(
            "Un PV doit identifier un usager ou un vehicule",
        )),
    }
}

fn validate_subject_fields(
    subject_type: &str,
    verbalized_name: Option<&str>,
    verbalized_identity_type: Option<&str>,
    verbalized_identity_number: Option<&str>,
    verbalized_phone: Option<&str>,
    _verbalized_address: Option<&str>,
    vehicle_plate: Option<&str>,
    vehicle_registration_card_number: Option<&str>,
    vehicle_make: Option<&str>,
    vehicle_model: Option<&str>,
    vehicle_color: Option<&str>,
    vehicle_owner_name: Option<&str>,
) -> Result<(), ApiError> {
    if verbalized_name.is_none() {
        return Err(ApiError::bad_request(
            "Le nom du contrevenant est requis pour creer un PV",
        ));
    }
    if verbalized_phone.is_none() {
        return Err(ApiError::bad_request(
            "Le telephone du contrevenant est requis pour creer un PV",
        ));
    }
    if verbalized_identity_number.is_some() && verbalized_identity_type.is_none() {
        return Err(ApiError::bad_request(
            "Le type d'identite est requis avec le numero d'identite",
        ));
    }
    let has_person_identity = has_person_identity(verbalized_name, verbalized_identity_number);
    let has_vehicle_identity =
        has_vehicle_identity(vehicle_plate, vehicle_registration_card_number);
    let has_vehicle_any = has_vehicle_any(
        vehicle_plate,
        vehicle_registration_card_number,
        vehicle_make,
        vehicle_model,
        vehicle_color,
        vehicle_owner_name,
    );
    match subject_type {
        "VEHICLE_ONLY" => Err(ApiError::bad_request(
            "Un PV requiert toujours un contrevenant; utilisez un PV usager avec vehicule",
        )),
        "PERSON_ONLY" if !has_person_identity => Err(ApiError::bad_request(
            "Un PV usager sans vehicule requiert un nom ou un numero d'identite",
        )),
        "PERSON_ONLY" if has_vehicle_any => Err(ApiError::bad_request(
            "Un PV usager sans vehicule ne doit pas contenir de donnees vehicule",
        )),
        "PERSON_WITH_VEHICLE" if !has_person_identity || !has_vehicle_identity => {
            Err(ApiError::bad_request(
                "Un PV usager avec vehicule requiert un contrevenant et une plaque ou carte grise",
            ))
        }
        "PERSON_ONLY" | "PERSON_WITH_VEHICLE" => Ok(()),
        _ => Err(ApiError::bad_request("subject_type invalide")),
    }
}

fn ensure_pv_editable(status: &str) -> Result<(), ApiError> {
    if status == "PAYE" || status == "ANNULE" {
        return Err(ApiError::conflict(
            "Un PV paye ou annule est consultable uniquement",
        ));
    }
    Ok(())
}

async fn require_pv_write_access(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    auth_user: &AuthUser,
    pv: &PvResponse,
) -> Result<(), ApiError> {
    if !is_agent_only(auth_user) {
        return Ok(());
    }
    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Agent non rattache a une commune"))?;
    let agent_id: Option<Uuid> = sqlx::query_scalar(
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
    .fetch_optional(&mut **tx)
    .await?;
    if agent_id != Some(pv.agent_id) {
        return Err(ApiError::forbidden("Acces refuse a ce PV"));
    }
    Ok(())
}

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
    intervention_ids: &[Uuid],
    verbalized_identity_number: Option<&str>,
    vehicle_plate: Option<&str>,
    vehicle_registration_card_number: Option<&str>,
    exclude_pv_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let bloquant: bool =
        sqlx::query_scalar("SELECT double_verbalisation_bloquant FROM communes WHERE id = $1")
            .bind(commune_id)
            .fetch_one(&mut **tx)
            .await?;

    if !bloquant {
        return Ok(());
    }

    for intervention_id in intervention_ids {
        if let Some(vid) = verbalized_identity_number {
            if !vid.trim().is_empty() {
                lock_double_verbalisation(tx, commune_id, *intervention_id, "identity", vid)
                    .await?;
                let existing: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT p.pv_number
                    FROM pvs p
                    JOIN pv_interventions pi ON pi.pv_id = p.id
                    WHERE p.commune_id = $1
                      AND pi.intervention_id = $2
                      AND COALESCE(p.verbalized_identity_number, p.verbalized_identifier) = $3
                      AND p.status NOT IN ('PAYE', 'ANNULE', 'NON_PAYANT')
                      AND p.deleted_at IS NULL
                      AND ($4::uuid IS NULL OR p.id <> $4)
                    LIMIT 1
                    "#,
                )
                .bind(commune_id)
                .bind(*intervention_id)
                .bind(vid)
                .bind(exclude_pv_id)
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

        if let Some(card_number) = vehicle_registration_card_number {
            if !card_number.trim().is_empty() {
                lock_double_verbalisation(
                    tx,
                    commune_id,
                    *intervention_id,
                    "registration-card",
                    card_number,
                )
                .await?;
                let existing: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT p.pv_number
                    FROM pvs p
                    JOIN pv_interventions pi ON pi.pv_id = p.id
                    WHERE p.commune_id = $1
                      AND pi.intervention_id = $2
                      AND p.vehicle_registration_card_number = $3
                      AND p.status NOT IN ('PAYE', 'ANNULE', 'NON_PAYANT')
                      AND p.deleted_at IS NULL
                      AND ($4::uuid IS NULL OR p.id <> $4)
                    LIMIT 1
                    "#,
                )
                .bind(commune_id)
                .bind(*intervention_id)
                .bind(card_number)
                .bind(exclude_pv_id)
                .fetch_optional(&mut **tx)
                .await?;

                if let Some(pv_num) = existing {
                    return Err(ApiError::conflict(format!(
                        "Double verbalisation detectee: PV {} existe deja pour cette carte grise",
                        pv_num
                    )));
                }
            }
        }

        if let Some(plate) = vehicle_plate {
            if !plate.trim().is_empty() {
                lock_double_verbalisation(tx, commune_id, *intervention_id, "plate", plate).await?;
                let existing: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT p.pv_number
                    FROM pvs p
                    JOIN pv_interventions pi ON pi.pv_id = p.id
                    WHERE p.commune_id = $1
                      AND pi.intervention_id = $2
                      AND p.vehicle_plate = $3
                      AND p.status NOT IN ('PAYE', 'ANNULE', 'NON_PAYANT')
                      AND p.deleted_at IS NULL
                      AND ($4::uuid IS NULL OR p.id <> $4)
                    LIMIT 1
                    "#,
                )
                .bind(commune_id)
                .bind(*intervention_id)
                .bind(plate)
                .bind(exclude_pv_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_with_vehicle_accepts_registration_card_without_plate() {
        let result = validate_subject_fields(
            "PERSON_WITH_VEHICLE",
            Some("Jean Test"),
            Some("CNI"),
            Some("ID123"),
            Some("+237600000000"),
            None,
            None,
            Some("CG123"),
            None,
            None,
            None,
            None,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn vehicle_only_is_rejected_even_with_full_vehicle_identity() {
        let result = validate_subject_fields(
            "VEHICLE_ONLY",
            Some("Jean Test"),
            None,
            None,
            Some("+237600000000"),
            None,
            Some("CE123AB"),
            Some("CG123"),
            Some("Toyota"),
            None,
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn person_with_vehicle_requires_vehicle_identifier() {
        let result = validate_subject_fields(
            "PERSON_WITH_VEHICLE",
            Some("Jean Test"),
            Some("CNI"),
            Some("ID123"),
            Some("+237600000000"),
            None,
            None,
            None,
            Some("Toyota"),
            None,
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn identity_number_requires_identity_type() {
        let result = validate_subject_fields(
            "PERSON_ONLY",
            Some("Jean Test"),
            None,
            Some("ID123"),
            Some("+237600000000"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Photos preuve (object storage MinIO/S3)
// ─────────────────────────────────────────────────────────────────────────────

const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct PvPhotoResponse {
    pub id: Uuid,
    pub pv_id: Uuid,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

fn photo_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/heic" | "image/heif" => "heic",
        "image/gif" => "gif",
        _ => "jpg",
    }
}

async fn list_pv_photos(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PvPhotoResponse>>, ApiError> {
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

    let rows = sqlx::query(
        r#"
        SELECT id, pv_id, content_type, size_bytes, created_at
        FROM pv_photos
        WHERE pv_id = $1 AND deleted_at IS NULL
        ORDER BY created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| PvPhotoResponse {
            id: row.get("id"),
            pv_id: row.get("pv_id"),
            content_type: row.get("content_type"),
            size_bytes: row.get("size_bytes"),
            created_at: row.get("created_at"),
        })
        .collect();
    Ok(Json(items))
}

async fn upload_pv_photo(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<PvPhotoResponse>), ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent, Role::AdminCommune, Role::SuperAdmin])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;
    require_pv_read_access(&state.db, &auth_user, &pv).await?;
    ensure_pv_editable(&pv.status)?;

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;

    let field = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?
        .ok_or_else(|| ApiError::bad_request("Fichier manquant"))?;

    let content_type = field
        .content_type()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !content_type.starts_with("image/") {
        return Err(ApiError::bad_request("Le fichier doit etre une image"));
    }

    let data = field
        .bytes()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    if data.is_empty() {
        return Err(ApiError::bad_request("Fichier vide"));
    }
    if data.len() > MAX_PHOTO_BYTES {
        return Err(ApiError::bad_request("Image trop volumineuse (max 8 Mo)"));
    }
    let size_bytes = data.len() as i64;

    let photo_id = Uuid::new_v4();
    let object_key = format!(
        "pv/{}/{}.{}",
        pv.id,
        photo_id,
        photo_extension(&content_type)
    );
    storage
        .put(&object_key, data.as_ref(), &content_type)
        .await
        .map_err(|error| {
            tracing::error!(%error, "pv photo upload failed");
            ApiError::internal("Echec de l'enregistrement de la photo")
        })?;

    let created_at: DateTime<Utc> = sqlx::query_scalar(
        r#"
        INSERT INTO pv_photos (id, pv_id, commune_id, object_key, content_type, size_bytes, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING created_at
        "#,
    )
    .bind(photo_id)
    .bind(pv.id)
    .bind(pv.commune_id)
    .bind(&object_key)
    .bind(&content_type)
    .bind(size_bytes)
    .bind(auth_user.id)
    .fetch_one(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(pv.commune_id),
        Some(auth_user.id),
        "PV_PHOTO_UPLOADED",
        "pv_photos",
        Some(photo_id),
        None,
        Some(json!({ "pv_id": pv.id, "content_type": content_type, "size_bytes": size_bytes })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(PvPhotoResponse {
            id: photo_id,
            pv_id: pv.id,
            content_type,
            size_bytes,
            created_at,
        }),
    ))
}

async fn get_pv_photo_content(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((id, photo_id)): Path<(Uuid, Uuid)>,
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

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;

    let row = sqlx::query(
        r#"
        SELECT object_key, content_type
        FROM pv_photos
        WHERE id = $1 AND pv_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(photo_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Photo introuvable"))?;

    let object_key: String = row.get("object_key");
    let content_type: String = row.get("content_type");
    let bytes = storage.get(&object_key).await.map_err(|error| {
        tracing::error!(%error, "pv photo download failed");
        ApiError::internal("Echec du telechargement de la photo")
    })?;

    use axum::http::header;
    use axum::response::IntoResponse;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

async fn delete_pv_photo(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((id, photo_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent, Role::AdminCommune, Role::SuperAdmin])?;
    let pv = load_pv(&state.db, id).await?;
    auth_user.require_commune_access(pv.commune_id)?;
    require_pv_read_access(&state.db, &auth_user, &pv).await?;
    ensure_pv_editable(&pv.status)?;

    let row = sqlx::query(
        r#"
        UPDATE pv_photos
        SET deleted_at = now()
        WHERE id = $1 AND pv_id = $2 AND deleted_at IS NULL
        RETURNING object_key
        "#,
    )
    .bind(photo_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Photo introuvable"))?;

    let object_key: String = row.get("object_key");
    if let Some(storage) = state.storage.as_ref() {
        if let Err(error) = storage.delete(&object_key).await {
            tracing::warn!(%error, "pv photo object delete failed");
        }
    }

    audit::record_for_commune(
        &state.db,
        Some(pv.commune_id),
        Some(auth_user.id),
        "PV_PHOTO_DELETED",
        "pv_photos",
        Some(photo_id),
        None,
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}
