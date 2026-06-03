use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/agents",
            axum::routing::get(list_agents).post(create_agent),
        )
        .route(
            "/agents/{id}",
            axum::routing::get(get_agent).patch(patch_agent),
        )
        .route("/agents/{id}/suspend", axum::routing::post(suspend_agent))
        .route(
            "/agents/{id}/reactivate",
            axum::routing::post(reactivate_agent),
        )
        .route("/agents/{id}/retire", axum::routing::post(retire_agent))
}

pub fn public_router() -> Router<AppState> {
    Router::new().route(
        "/agents/verify/{matricule}",
        axum::routing::get(verify_agent_public),
    )
}

#[derive(Debug, Deserialize)]
struct AgentFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
    commune_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    grade: String,
    date_prise_fonction: Option<NaiveDate>,
    formation_nasla: Option<bool>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct PatchAgentRequest {
    matricule: Option<String>,
    full_name: Option<String>,
    commune_id: Option<Uuid>,
    grade: Option<String>,
    status: Option<String>,
    date_prise_fonction: Option<NaiveDate>,
    formation_nasla: Option<bool>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    user_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentResponse {
    id: Uuid,
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    grade: String,
    status: String,
    date_prise_fonction: Option<NaiveDate>,
    formation_nasla: bool,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    user_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PublicAgentVerification {
    matricule: String,
    full_name: String,
    commune_code: String,
    commune_nom: String,
    grade: String,
    status: String,
    active: bool,
}

async fn get_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;
    Ok(Json(agent))
}

async fn list_agents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<AgentFilterQuery>,
) -> Result<Json<Paginated<AgentResponse>>, ApiError> {
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

    let status_filter = match query.status {
        Some(ref s) => Some(validate_agent_status(s)?),
        None => None,
    };

    let commune_filter: Option<Uuid> = if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        query.commune_id
    } else {
        let user_commune = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        if let Some(req) = query.commune_id {
            if req != user_commune {
                return Err(ApiError::forbidden("Acces refuse a cette commune"));
            }
        }
        Some(user_commune)
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM agents WHERE deleted_at IS NULL");
    if let Some(id) = commune_filter {
        count_qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = status_filter {
        count_qb.push(" AND status = ").push_bind(s.clone());
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM agents WHERE deleted_at IS NULL");
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = status_filter {
        qb.push(" AND status = ").push_bind(s.clone());
    }
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let agents = rows.into_iter().map(row_to_agent).collect::<Vec<_>>();
    Ok(Json(Paginated::new(agents, &pagination, total)))
}

async fn create_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let agent_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, matricule, full_name, commune_id, grade, status,
            date_prise_fonction, formation_nasla, photo_url, telephone, email, user_id
        )
        VALUES ($1, $2, $3, $4, $5, 'ACTIF', $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(agent_id)
    .bind(required_text(payload.matricule, "matricule")?)
    .bind(required_text(payload.full_name, "full_name")?)
    .bind(payload.commune_id)
    .bind(required_text(payload.grade, "grade")?)
    .bind(payload.date_prise_fonction)
    .bind(payload.formation_nasla.unwrap_or(false))
    .bind(clean_optional(payload.photo_url))
    .bind(clean_optional(payload.telephone))
    .bind(clean_optional(payload.email))
    .bind(payload.user_id)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "AGENT_CREATED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({ "id": agent_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_agent(&state.db, agent_id).await?))
}

async fn patch_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchAgentRequest>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;
    let commune_id = payload.commune_id.unwrap_or(existing.commune_id);
    auth_user.require_commune_access(commune_id)?;
    let status = match payload.status {
        Some(status) => validate_agent_status(&status)?,
        None => existing.status.clone(),
    };

    sqlx::query(
        r#"
        UPDATE agents
        SET matricule = $2,
            full_name = $3,
            commune_id = $4,
            grade = $5,
            status = $6,
            date_prise_fonction = $7,
            formation_nasla = $8,
            photo_url = $9,
            telephone = $10,
            email = $11,
            user_id = $12,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(match payload.matricule {
        Some(value) => required_text(value, "matricule")?,
        None => existing.matricule.clone(),
    })
    .bind(match payload.full_name {
        Some(value) => required_text(value, "full_name")?,
        None => existing.full_name.clone(),
    })
    .bind(commune_id)
    .bind(match payload.grade {
        Some(value) => required_text(value, "grade")?,
        None => existing.grade.clone(),
    })
    .bind(&status)
    .bind(payload.date_prise_fonction.or(existing.date_prise_fonction))
    .bind(payload.formation_nasla.unwrap_or(existing.formation_nasla))
    .bind(payload.photo_url.or(existing.photo_url.clone()))
    .bind(payload.telephone.or(existing.telephone.clone()))
    .bind(payload.email.or(existing.email.clone()))
    .bind(payload.user_id.or(existing.user_id))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "AGENT_UPDATED",
        "agents",
        Some(agent_id),
        Some(json!({ "status": existing.status, "commune_id": existing.commune_id })),
        Some(json!({ "status": status, "commune_id": commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_agent(&state.db, agent_id).await?))
}

async fn suspend_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "SUSPENDU", "AGENT_SUSPENDED").await
}

async fn reactivate_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "ACTIF", "AGENT_REACTIVATED").await
}

async fn retire_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "RETRAITE", "AGENT_RETIRED").await
}

async fn change_agent_status(
    state: &AppState,
    auth_user: &AuthUser,
    agent_id: Uuid,
    status: &'static str,
    action: &'static str,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    sqlx::query(
        r#"
        UPDATE agents
        SET status = $2, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(status)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        action,
        "agents",
        Some(agent_id),
        Some(json!({ "status": existing.status })),
        Some(json!({ "status": status })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_agent(&state.db, agent_id).await?))
}

async fn verify_agent_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(matricule): Path<String>,
) -> Result<Json<PublicAgentVerification>, ApiError> {
    state.rate_limiter.check(
        "public:agents:verify",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let matricule = required_text(matricule, "matricule")?;
    let row = sqlx::query(
        r#"
        SELECT
            a.matricule,
            a.full_name,
            a.grade,
            a.status,
            c.code AS commune_code,
            c.nom AS commune_nom
        FROM agents a
        INNER JOIN communes c ON c.id = a.commune_id
        WHERE lower(a.matricule) = lower($1)
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(&matricule)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Agent introuvable"))?;

    let status: String = row.get("status");
    Ok(Json(PublicAgentVerification {
        matricule: row.get("matricule"),
        full_name: row.get("full_name"),
        commune_code: row.get("commune_code"),
        commune_nom: row.get("commune_nom"),
        grade: row.get("grade"),
        active: status == "ACTIF",
        status,
    }))
}

pub async fn load_agent(pool: &PgPool, agent_id: Uuid) -> Result<AgentResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Agent introuvable"))?;

    Ok(row_to_agent(row))
}

fn row_to_agent(row: sqlx::postgres::PgRow) -> AgentResponse {
    AgentResponse {
        id: row.get("id"),
        matricule: row.get("matricule"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        grade: row.get("grade"),
        status: row.get("status"),
        date_prise_fonction: row.get("date_prise_fonction"),
        formation_nasla: row.get("formation_nasla"),
        photo_url: row.get("photo_url"),
        telephone: row.get("telephone"),
        email: row.get("email"),
        user_id: row.get("user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn validate_agent_status(value: &str) -> Result<String, ApiError> {
    let status = value.trim().to_ascii_uppercase();
    if matches!(
        status.as_str(),
        "ACTIF" | "SUSPENDU" | "RETRAITE" | "INACTIF"
    ) {
        Ok(status)
    } else {
        Err(ApiError::bad_request(
            "Statut agent invalide. Valeurs acceptees: ACTIF, SUSPENDU, RETRAITE, INACTIF",
        ))
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
