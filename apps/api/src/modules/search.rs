use axum::{
    extract::{Query, State},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::helpers::{is_agent_only, resolve_commune_filter};
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::state::AppState;

const DEFAULT_LIMIT: i64 = 24;
const MAX_LIMIT: i64 = 50;

pub fn router() -> Router<AppState> {
    Router::new().route("/search", axum::routing::get(search))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    commune_id: Option<Uuid>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub module: &'static str,
    pub id: Uuid,
    pub title: String,
    pub detail: String,
    pub status: Option<String>,
    pub route: String,
}

async fn search(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let term = query.q.trim();
    if term.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let per_module_limit = (limit / 3).max(6);
    let pattern = format!("%{term}%");
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let agent_only = is_agent_only(&auth_user);
    let agent_filter = if agent_only {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM agents WHERE user_id = $1 AND deleted_at IS NULL LIMIT 1",
        )
        .bind(auth_user.id)
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };

    let mut results = Vec::new();

    if auth_user.has_role(Role::SuperAdmin)
        || auth_user.has_role(Role::AdminCommune)
        || auth_user.has_role(Role::Superviseur)
    {
        results.extend(search_agents(&state.db, &pattern, commune_filter, per_module_limit).await?);
    }

    if (auth_user.has_role(Role::SuperAdmin)
        || auth_user.has_role(Role::AdminCommune)
        || auth_user.has_role(Role::Superviseur)
        || auth_user.has_role(Role::Receveur)
        || auth_user.has_role(Role::ApmAgent))
        && (!agent_only || agent_filter.is_some())
    {
        results.extend(
            search_pvs(
                &state.db,
                &pattern,
                commune_filter,
                agent_filter,
                per_module_limit,
            )
            .await?,
        );
    }

    if auth_user.has_role(Role::SuperAdmin)
        || auth_user.has_role(Role::AdminCommune)
        || auth_user.has_role(Role::Superviseur)
    {
        results.extend(
            search_signalements(&state.db, &pattern, commune_filter, per_module_limit).await?,
        );
    }

    if auth_user.has_role(Role::SuperAdmin)
        || auth_user.has_role(Role::AdminCommune)
        || auth_user.has_role(Role::Receveur)
        || auth_user.has_role(Role::Superviseur)
    {
        results
            .extend(search_payments(&state.db, &pattern, commune_filter, per_module_limit).await?);
    }

    results.truncate(limit as usize);
    Ok(Json(results))
}

async fn search_agents(
    pool: &sqlx::PgPool,
    pattern: &str,
    commune_filter: Option<Uuid>,
    limit: i64,
) -> Result<Vec<SearchResult>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT id, matricule, full_name, grade, status
        FROM agents
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
          AND (
            matricule ILIKE $2
            OR full_name ILIKE $2
            OR grade ILIKE $2
            OR status ILIKE $2
          )
        ORDER BY updated_at DESC
        LIMIT $3
        "#,
    )
    .bind(commune_filter)
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let matricule: String = row.get("matricule");
            let full_name: String = row.get("full_name");
            let grade: String = row.get("grade");
            let status: String = row.get("status");

            SearchResult {
                module: "Agent",
                id,
                title: format!("{matricule} - {full_name}"),
                detail: grade,
                status: Some(status),
                route: "/agents".to_string(),
            }
        })
        .collect())
}

async fn search_pvs(
    pool: &sqlx::PgPool,
    pattern: &str,
    commune_filter: Option<Uuid>,
    agent_filter: Option<Uuid>,
    limit: i64,
) -> Result<Vec<SearchResult>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id, pv_number, status, vehicle_plate, vehicle_registration_card_number,
            verbalized_name, verbalized_identity_number, amount_initial_fcfa
        FROM pvs
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
          AND ($2::uuid IS NULL OR agent_id = $2)
          AND (
            pv_number ILIKE $3
            OR COALESCE(vehicle_plate, '') ILIKE $3
            OR COALESCE(vehicle_registration_card_number, '') ILIKE $3
            OR COALESCE(verbalized_name, '') ILIKE $3
            OR COALESCE(verbalized_identity_number, '') ILIKE $3
            OR COALESCE(verbalized_identifier, '') ILIKE $3
            OR status ILIKE $3
          )
        ORDER BY created_at DESC
        LIMIT $4
        "#,
    )
    .bind(commune_filter)
    .bind(agent_filter)
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let pv_number: String = row.get("pv_number");
            let status: String = row.get("status");
            let plate = row
                .get::<Option<String>, _>("vehicle_plate")
                .or_else(|| row.get::<Option<String>, _>("vehicle_registration_card_number"))
                .unwrap_or_else(|| "Vehicule non renseigne".to_string());
            let verbalized = row
                .get::<Option<String>, _>("verbalized_name")
                .or_else(|| row.get::<Option<String>, _>("verbalized_identity_number"))
                .unwrap_or_else(|| "Verbalise non renseigne".to_string());
            let amount = row
                .get::<Option<i64>, _>("amount_initial_fcfa")
                .map(|value| format!(" - {value} FCFA"))
                .unwrap_or_default();

            SearchResult {
                module: "PV",
                id,
                title: pv_number,
                detail: format!("{plate} - {verbalized}{amount}"),
                status: Some(status),
                route: "/pvs".to_string(),
            }
        })
        .collect())
}

async fn search_signalements(
    pool: &sqlx::PgPool,
    pattern: &str,
    commune_filter: Option<Uuid>,
    limit: i64,
) -> Result<Vec<SearchResult>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT id, signalement_number, type_incident, location_description, status
        FROM signalements
        WHERE ($1::uuid IS NULL OR commune_id = $1)
          AND (
            signalement_number ILIKE $2
            OR type_incident ILIKE $2
            OR location_description ILIKE $2
            OR description ILIKE $2
            OR status ILIKE $2
          )
        ORDER BY created_at DESC
        LIMIT $3
        "#,
    )
    .bind(commune_filter)
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let number: String = row.get("signalement_number");
            let incident: String = row.get("type_incident");
            let location: String = row.get("location_description");
            let status: String = row.get("status");

            SearchResult {
                module: "Signalement",
                id,
                title: number,
                detail: format!("{incident} - {location}"),
                status: Some(status),
                route: "/signalements".to_string(),
            }
        })
        .collect())
}

async fn search_payments(
    pool: &sqlx::PgPool,
    pattern: &str,
    commune_filter: Option<Uuid>,
    limit: i64,
) -> Result<Vec<SearchResult>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT pay.id, pay.receipt_number, pay.status, pay.amount_paid_fcfa, p.pv_number
        FROM payments pay
        INNER JOIN pvs p ON p.id = pay.pv_id
        WHERE ($1::uuid IS NULL OR pay.commune_id = $1)
          AND (
            COALESCE(pay.receipt_number, '') ILIKE $2
            OR p.pv_number ILIKE $2
            OR pay.status ILIKE $2
          )
        ORDER BY pay.created_at DESC
        LIMIT $3
        "#,
    )
    .bind(commune_filter)
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let receipt = row
                .get::<Option<String>, _>("receipt_number")
                .unwrap_or_else(|| "Recu en attente".to_string());
            let pv_number: String = row.get("pv_number");
            let status: String = row.get("status");
            let amount = row
                .get::<Option<i64>, _>("amount_paid_fcfa")
                .map(|value| format!("{value} FCFA"))
                .unwrap_or_else(|| "Montant non renseigne".to_string());

            SearchResult {
                module: "Paiement",
                id,
                title: receipt,
                detail: format!("{pv_number} - {amount}"),
                status: Some(status),
                route: "/payments".to_string(),
            }
        })
        .collect())
}
