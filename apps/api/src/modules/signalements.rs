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
use crate::modules::whatsapp;
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
        .route(
            "/signalements/{id}/escalate",
            axum::routing::post(escalate_signalement),
        )
}

/// Autorités de tutelle vers lesquelles un signalement peut être escaladé.
const ESCALATION_TARGETS: [&str; 4] = ["MAIRIE", "NASLA", "MINDDEVEL", "MINAT"];

/// Types d'action contestée par un signalement (plainte contre un agent APM).
/// Liste fixe — le citoyen choisit l'une de ces valeurs, stockées telles quelles
/// dans `type_incident` (libellé canonique FR).
const COMPLAINT_TYPES: [&str; 5] = [
    "Amende",
    "Verbalisation",
    "Mise sous scellé",
    "Mise en fourrière",
    "Autre",
];

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
    pub reported_agent_matricule: Option<String>,
    pub reported_agent_nom: Option<String>,
    pub incident_datetime: Option<DateTime<Utc>>,
    pub pv_number_ref: Option<String>,
    pub contact_anonyme: bool,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_info: Option<String>,
    pub status: String,
    pub escalation_target: Option<String>,
    pub escalated_at: Option<DateTime<Utc>>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Colonnes exposées par les SELECT (lat/lon castées en double precision).
const SIGNALEMENT_COLUMNS: &str = "id, commune_id, zone_id, signalement_number, type_incident, \
    location_description, lieu_dit, description, reported_agent_matricule, reported_agent_nom, \
    incident_datetime, pv_number_ref, contact_anonyme, contact_name, contact_phone, \
    contact_info, status, escalation_target, escalated_at, \
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
    pub reported_agent_matricule: Option<String>,
    pub reported_agent_nom: Option<String>,
    pub incident_datetime: Option<DateTime<Utc>>,
    pub pv_number_ref: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct EscalateSignalementRequest {
    pub target: String,
    pub note: Option<String>,
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

    let type_incident = validate_complaint_type(payload.type_incident)?;
    let description = required_text(payload.description, "description")?;
    let lieu_dit = clean_optional(payload.lieu_dit);
    let reported_agent_matricule = clean_optional(payload.reported_agent_matricule);
    let reported_agent_nom = clean_optional(payload.reported_agent_nom);
    let pv_number_ref = clean_optional(payload.pv_number_ref);
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
            location_description, lieu_dit, description,
            reported_agent_matricule, reported_agent_nom, incident_datetime, pv_number_ref,
            contact_anonyme, contact_name, contact_phone, contact_info,
            gps_latitude, gps_longitude
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
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
    .bind(reported_agent_matricule.as_deref())
    .bind(reported_agent_nom.as_deref())
    .bind(payload.incident_datetime)
    .bind(pv_number_ref.as_deref())
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

    // Notification WhatsApp du numéro de suivi (best-effort, sans bloquer la
    // réponse) : uniquement si la fonctionnalité est configurée et que le
    // plaignant a fourni un contact non anonyme.
    if let (Some(cfg), Some(phone)) = (state.config.whatsapp.clone(), contact_phone.clone()) {
        let body = format!(
            "APMTRACK : votre signalement a bien été reçu. Numéro de suivi : {number}. \
             Suivez son avancement sur {}/public/signalement-suivi",
            state.config.public_web_url
        );
        tokio::spawn(async move {
            if let Err(error) = whatsapp::send_text(&cfg, &phone, &body).await {
                tracing::warn!(%error, "envoi WhatsApp du numéro de suivi échoué");
            }
        });
    }

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

    if let Some(assigned_to) = payload.assigned_to {
        validate_assignee(&state.db, assigned_to, existing.commune_id).await?;
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
        Some(json!({ "status": payload.status, "assigned_to": payload.assigned_to })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_signalement(&state.db, id).await?))
}

/// Escalade vers une autorité de tutelle (Mairie / NASLA / MINDDEVEL / MINAT).
/// La transmission reste tracée via `audit_logs` ; la cible et l'horodatage
/// sont conservés sur le signalement.
async fn escalate_signalement(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<EscalateSignalementRequest>,
) -> Result<Json<SignalementResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_signalement(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let target = payload.target.trim().to_uppercase();
    if !ESCALATION_TARGETS.contains(&target.as_str()) {
        return Err(ApiError::bad_request(format!(
            "Cible d'escalade invalide: {} (attendu: {})",
            payload.target,
            ESCALATION_TARGETS.join(", ")
        )));
    }
    let note = clean_optional(payload.note);

    sqlx::query(
        r#"
        UPDATE signalements
        SET escalation_target = $2, escalated_at = now(), escalated_by = $3,
            escalation_note = COALESCE($4, escalation_note), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&target)
    .bind(auth_user.id)
    .bind(note.as_deref())
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "SIGNALEMENT_ESCALATED",
        "signalements",
        Some(id),
        Some(json!({ "escalation_target": existing.escalation_target })),
        Some(json!({ "escalation_target": target, "note": note })),
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
        reported_agent_matricule: row.get("reported_agent_matricule"),
        reported_agent_nom: row.get("reported_agent_nom"),
        incident_datetime: row.get("incident_datetime"),
        pv_number_ref: row.get("pv_number_ref"),
        contact_anonyme: row.get("contact_anonyme"),
        contact_name: row.get("contact_name"),
        contact_phone: row.get("contact_phone"),
        contact_info: row.get("contact_info"),
        status: row.get("status"),
        escalation_target: row.get("escalation_target"),
        escalated_at: row.get("escalated_at"),
        gps_latitude: row.get("gps_latitude"),
        gps_longitude: row.get("gps_longitude"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Règle métier : l'utilisateur affecté à un signalement doit être actif,
/// non supprimé et (a) rattaché à la commune du signalement, ou (b) superviseur
/// global (SUPER_ADMIN / SUPERVISEUR sans commune — profils NASLA/MINISTÈRE).
async fn validate_assignee(
    pool: &PgPool,
    user_id: Uuid,
    signalement_commune_id: Uuid,
) -> Result<(), ApiError> {
    let row = sqlx::query(
        r#"
        SELECT u.active, u.commune_id,
               EXISTS (
                   SELECT 1 FROM user_roles ur
                   JOIN roles r ON r.id = ur.role_id
                   WHERE ur.user_id = u.id
                     AND r.code IN ('SUPER_ADMIN', 'SUPERVISEUR')
               ) AS has_global_role
        FROM users u
        WHERE u.id = $1 AND u.deleted_at IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::bad_request("Utilisateur affecté introuvable"))?;

    if !row.get::<bool, _>("active") {
        return Err(ApiError::bad_request(
            "Impossible d'affecter un utilisateur inactif",
        ));
    }
    match row.get::<Option<Uuid>, _>("commune_id") {
        Some(commune_id) if commune_id != signalement_commune_id => {
            Err(ApiError::bad_request(
                "L'utilisateur affecté n'appartient pas à la commune du signalement",
            ))
        }
        None if !row.get::<bool, _>("has_global_role") => Err(ApiError::bad_request(
            "Seul un superviseur global (SUPER_ADMIN ou SUPERVISEUR) peut être affecté hors commune",
        )),
        _ => Ok(()),
    }
}

/// Valide le type d'action contestée contre la liste fixe `COMPLAINT_TYPES`
/// et renvoie le libellé canonique (comparaison insensible à la casse).
fn validate_complaint_type(value: String) -> Result<String, ApiError> {
    let trimmed = value.trim();
    COMPLAINT_TYPES
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(trimmed))
        .map(|candidate| candidate.to_string())
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Type de signalement invalide (attendu: {})",
                COMPLAINT_TYPES.join(", ")
            ))
        })
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
