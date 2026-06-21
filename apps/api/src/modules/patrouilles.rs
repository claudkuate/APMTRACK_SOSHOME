use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{ApiError, map_database_error};
use crate::extractors::ApiJson;
use crate::helpers::{
    feature_collection, geo_feature, is_agent_only, resolve_commune_filter, validate_gps,
};
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
        .route(
            "/patrouilles",
            axum::routing::get(list_patrouilles).post(create_patrouille),
        )
        .route(
            "/patrouilles/{id}",
            axum::routing::get(get_patrouille).patch(patch_patrouille),
        )
        .route(
            "/patrouilles/{id}/start",
            axum::routing::post(start_patrouille),
        )
        .route("/patrouilles/{id}/end", axum::routing::post(end_patrouille))
        .route(
            "/patrouilles/{id}/agents",
            axum::routing::get(list_patrouille_agents).post(assign_agent),
        )
        .route(
            "/patrouilles/{id}/agents/{agent_id}",
            axum::routing::delete(remove_agent),
        )
        .route(
            "/patrouilles/{id}/positions",
            axum::routing::post(record_position),
        )
        .route("/patrouilles/{id}/track", axum::routing::get(get_track))
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PatrouilleResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub zone_id: Option<Uuid>,
    pub nom: String,
    pub description: Option<String>,
    pub status: String,
    pub date_debut: Option<DateTime<Utc>>,
    pub date_fin: Option<DateTime<Utc>>,
    pub date_debut_prevue: Option<DateTime<Utc>>,
    pub date_fin_prevue: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PatrouilleAgentResponse {
    pub id: Uuid,
    pub patrouille_id: Uuid,
    pub agent_id: Uuid,
    pub role_patrouille: String,
    pub agent_matricule: Option<String>,
    pub agent_nom: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatrouilleFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePatrouilleRequest {
    pub commune_id: Uuid,
    pub zone_id: Option<Uuid>,
    pub nom: Option<String>,
    pub description: Option<String>,
    pub date_debut_prevue: Option<DateTime<Utc>>,
    pub date_fin_prevue: Option<DateTime<Utc>>,
    /// Agents affectés dès la création (rôle MEMBRE par défaut ; un chef se
    /// désigne ensuite via la gestion de l'effectif).
    pub agent_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct PatchPatrouilleRequest {
    pub zone_id: Option<Uuid>,
    pub nom: Option<String>,
    pub description: Option<String>,
    pub date_debut_prevue: Option<DateTime<Utc>>,
    pub date_fin_prevue: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct AssignAgentRequest {
    pub agent_id: Uuid,
    pub role_patrouille: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPositionRequest {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: Option<f64>,
    /// Agent explicite (admins). Ignoré pour un APM_AGENT (résolu via son compte).
    pub agent_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct PositionRecordedResponse {
    pub id: Uuid,
    pub patrouille_id: Uuid,
    pub recorded_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_patrouilles(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PatrouilleFilterQuery>,
) -> Result<Json<Paginated<PatrouilleResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
    ])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let agent_filter = if is_agent_only(&auth_user) {
        match active_agent_id_for_user(&state.db, &auth_user).await? {
            Some(agent_id) => Some(agent_id),
            None => return Ok(Json(Paginated::new(Vec::new(), &pagination, 0))),
        }
    } else {
        None
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM patrouilles WHERE deleted_at IS NULL");
    apply_filters(
        &mut count_qb,
        commune_filter,
        query.status.as_deref(),
        agent_filter,
    );
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM patrouilles WHERE deleted_at IS NULL");
    apply_filters(
        &mut qb,
        commune_filter,
        query.status.as_deref(),
        agent_filter,
    );
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_patrouille).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PatrouilleResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
    ])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;
    require_agent_patrouille_access(&state.db, &auth_user, id).await?;
    Ok(Json(pat))
}

async fn create_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreatePatrouilleRequest>,
) -> Result<(StatusCode, Json<PatrouilleResponse>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let zone_id = payload
        .zone_id
        .ok_or_else(|| ApiError::bad_request("zone_id est requis"))?;
    validate_planning(payload.date_debut_prevue, payload.date_fin_prevue)?;
    let id = Uuid::new_v4();

    // Dédoublonnage des agents fournis avant insertion.
    let mut agent_ids = payload.agent_ids.unwrap_or_default();
    agent_ids.sort();
    agent_ids.dedup();
    if agent_ids.is_empty() {
        return Err(ApiError::bad_request(
            "Au moins un agent est requis pour creer une patrouille",
        ));
    }

    let mut tx = state.db.begin().await?;
    let zone_name = load_zone_name_for_commune(&mut *tx, payload.commune_id, zone_id).await?;
    let nom = clean_optional(payload.nom)
        .unwrap_or_else(|| default_patrouille_name(&zone_name, payload.date_debut_prevue));

    sqlx::query(
        r#"
        INSERT INTO patrouilles
            (id, commune_id, zone_id, nom, description, date_debut_prevue, date_fin_prevue, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(zone_id)
    .bind(&nom)
    .bind(clean_optional(payload.description))
    .bind(payload.date_debut_prevue)
    .bind(payload.date_fin_prevue)
    .bind(auth_user.id)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    // Affectation initiale de l'effectif. Tout agent invalide annule la création.
    for agent_id in &agent_ids {
        assert_agent_assignable(&mut *tx, payload.commune_id, *agent_id).await?;
        sqlx::query(
            r#"
            INSERT INTO patrouille_agents (patrouille_id, agent_id, role_patrouille)
            VALUES ($1, $2, 'MEMBRE')
            ON CONFLICT (patrouille_id, agent_id) DO NOTHING
            "#,
        )
        .bind(id)
        .bind(*agent_id)
        .execute(&mut *tx)
        .await
        .map_err(map_database_error)?;
    }

    tx.commit().await?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "PATROUILLE_CREATED",
        "patrouilles",
        Some(id),
        None,
        Some(json!({ "nom": nom, "commune_id": payload.commune_id, "agents": agent_ids.len() })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(load_patrouille(&state.db, id).await?),
    ))
}

async fn patch_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchPatrouilleRequest>,
) -> Result<Json<PatrouilleResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    if existing.status == "CLOTUREE" {
        return Err(ApiError::conflict(
            "Une patrouille clôturée ne peut pas être modifiée",
        ));
    }

    let nom = match payload.nom {
        Some(v) => required_text(v, "nom")?,
        None => existing.nom.clone(),
    };

    // Valide l'ordre des dates en tenant compte des valeurs déjà en base.
    validate_planning(
        payload.date_debut_prevue.or(existing.date_debut_prevue),
        payload.date_fin_prevue.or(existing.date_fin_prevue),
    )?;
    if let Some(zone_id) = payload.zone_id {
        load_zone_name_for_commune(&state.db, existing.commune_id, zone_id).await?;
    }

    sqlx::query(
        r#"
        UPDATE patrouilles
        SET nom = $2,
            description = COALESCE($3, description),
            zone_id = COALESCE($4, zone_id),
            date_debut_prevue = COALESCE($5, date_debut_prevue),
            date_fin_prevue = COALESCE($6, date_fin_prevue),
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&nom)
    .bind(payload.description.as_deref())
    .bind(payload.zone_id)
    .bind(payload.date_debut_prevue)
    .bind(payload.date_fin_prevue)
    .execute(&state.db)
    .await?;

    Ok(Json(load_patrouille(&state.db, id).await?))
}

async fn start_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PatrouilleResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;

    if pat.status != "PLANIFIEE" {
        return Err(ApiError::conflict(format!(
            "Impossible de démarrer une patrouille avec le statut '{}'",
            pat.status
        )));
    }

    sqlx::query(
        "UPDATE patrouilles SET status = 'EN_COURS', date_debut = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(pat.commune_id),
        Some(auth_user.id),
        "PATROUILLE_STARTED",
        "patrouilles",
        Some(id),
        Some(json!({ "status": "PLANIFIEE" })),
        Some(json!({ "status": "EN_COURS" })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_patrouille(&state.db, id).await?))
}

async fn end_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PatrouilleResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;

    if pat.status != "EN_COURS" {
        return Err(ApiError::conflict(format!(
            "Impossible de clôturer une patrouille avec le statut '{}'",
            pat.status
        )));
    }

    sqlx::query(
        "UPDATE patrouilles SET status = 'CLOTUREE', date_fin = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(pat.commune_id),
        Some(auth_user.id),
        "PATROUILLE_ENDED",
        "patrouilles",
        Some(id),
        Some(json!({ "status": "EN_COURS" })),
        Some(json!({ "status": "CLOTUREE" })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_patrouille(&state.db, id).await?))
}

async fn list_patrouille_agents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PatrouilleAgentResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
    ])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;
    require_agent_patrouille_access(&state.db, &auth_user, id).await?;

    let rows = sqlx::query(
        r#"
        SELECT pa.id, pa.patrouille_id, pa.agent_id, pa.role_patrouille,
               a.matricule AS agent_matricule, a.full_name AS agent_nom
        FROM patrouille_agents pa
        JOIN agents a ON pa.agent_id = a.id
        WHERE pa.patrouille_id = $1
        ORDER BY pa.role_patrouille DESC, a.full_name ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| PatrouilleAgentResponse {
            id: r.get("id"),
            patrouille_id: r.get("patrouille_id"),
            agent_id: r.get("agent_id"),
            role_patrouille: r.get("role_patrouille"),
            agent_matricule: r.get("agent_matricule"),
            agent_nom: r.get("agent_nom"),
        })
        .collect();

    Ok(Json(items))
}

async fn assign_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<AssignAgentRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;

    if pat.status == "CLOTUREE" {
        return Err(ApiError::conflict(
            "Impossible d'assigner un agent à une patrouille clôturée",
        ));
    }

    // Vérifier que l'agent est actif et appartient à la même commune
    assert_agent_assignable(&state.db, pat.commune_id, payload.agent_id).await?;

    let role = payload
        .role_patrouille
        .as_deref()
        .unwrap_or("MEMBRE")
        .to_uppercase();
    if role != "CHEF" && role != "MEMBRE" {
        return Err(ApiError::bad_request(
            "role_patrouille doit être CHEF ou MEMBRE",
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO patrouille_agents (patrouille_id, agent_id, role_patrouille)
        VALUES ($1, $2, $3)
        ON CONFLICT (patrouille_id, agent_id) DO UPDATE SET role_patrouille = $3
        "#,
    )
    .bind(id)
    .bind(payload.agent_id)
    .bind(&role)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(pat.commune_id),
        Some(auth_user.id),
        "PATROUILLE_AGENT_ASSIGNED",
        "patrouille_agents",
        Some(id),
        None,
        Some(json!({ "agent_id": payload.agent_id, "role": role })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "assigned": true, "agent_id": payload.agent_id, "role": role })),
    ))
}

async fn remove_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;

    if pat.status == "CLOTUREE" {
        return Err(ApiError::conflict(
            "Impossible de modifier une patrouille clôturée",
        ));
    }

    sqlx::query("DELETE FROM patrouille_agents WHERE patrouille_id = $1 AND agent_id = $2")
        .bind(id)
        .bind(agent_id)
        .execute(&state.db)
        .await?;

    audit::record_for_commune(
        &state.db,
        Some(pat.commune_id),
        Some(auth_user.id),
        "PATROUILLE_AGENT_REMOVED",
        "patrouille_agents",
        Some(id),
        Some(json!({ "agent_id": agent_id })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "removed": true, "agent_id": agent_id })))
}

/// Enregistre une position GPS sur la trace de la patrouille (fil d'ariane terrain).
async fn record_position(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<RecordPositionRequest>,
) -> Result<(StatusCode, Json<PositionRecordedResponse>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::ApmAgent])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;
    validate_gps(Some(payload.latitude), Some(payload.longitude))?;

    if pat.status != "EN_COURS" {
        return Err(ApiError::conflict(
            "Seule une patrouille en cours peut recevoir des positions",
        ));
    }

    // Un APM_AGENT enregistre forcément sa propre position ; les admins peuvent cibler un agent.
    let agent_id: Option<Uuid> = if is_agent_only(&auth_user) {
        let agent_id = active_agent_id_for_user(&state.db, &auth_user)
            .await?
            .ok_or_else(|| ApiError::forbidden("Agent actif introuvable pour cet utilisateur"))?;
        require_patrouille_assignment(&state.db, id, agent_id).await?;
        Some(agent_id)
    } else {
        payload.agent_id
    };

    let position_id = Uuid::new_v4();
    let recorded_at: DateTime<Utc> = sqlx::query_scalar(
        r#"
        INSERT INTO patrouille_positions (id, patrouille_id, agent_id, geom, accuracy_m)
        VALUES ($1, $2, $3, ST_SetSRID(ST_MakePoint($4, $5), 4326), $6)
        RETURNING recorded_at
        "#,
    )
    .bind(position_id)
    .bind(id)
    .bind(agent_id)
    .bind(payload.longitude)
    .bind(payload.latitude)
    .bind(payload.accuracy_m)
    .fetch_one(&state.db)
    .await
    .map_err(map_database_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PositionRecordedResponse {
            id: position_id,
            patrouille_id: id,
            recorded_at,
        }),
    ))
}

/// Renvoie la trace de la patrouille : points GeoJSON + ligne reconstruite.
async fn get_track(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
    ])?;
    let pat = load_patrouille(&state.db, id).await?;
    auth_user.require_commune_access(pat.commune_id)?;
    require_agent_patrouille_access(&state.db, &auth_user, id).await?;

    let rows = sqlx::query(
        r#"
        SELECT id, agent_id, accuracy_m::double precision AS accuracy_m, recorded_at,
               ST_AsGeoJSON(geom) AS geojson
        FROM patrouille_positions
        WHERE patrouille_id = $1
        ORDER BY recorded_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let features: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let geometry = row
                .get::<Option<String>, _>("geojson")
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::Value::Null);
            geo_feature(
                geometry,
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "agent_id": row.get::<Option<Uuid>, _>("agent_id"),
                    "accuracy_m": row.get::<Option<f64>, _>("accuracy_m"),
                    "recorded_at": row.get::<DateTime<Utc>, _>("recorded_at"),
                }),
            )
        })
        .collect();

    // Ligne reconstruite (>= 2 points) pour tracer le parcours d'un trait.
    let line: serde_json::Value = if rows.len() >= 2 {
        let line_geojson: Option<String> = sqlx::query_scalar(
            r#"
            SELECT ST_AsGeoJSON(ST_MakeLine(geom ORDER BY recorded_at))
            FROM patrouille_positions
            WHERE patrouille_id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        line_geojson
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    Ok(Json(json!({
        "patrouille_id": id,
        "count": rows.len(),
        "points": feature_collection(features),
        "line": line,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_patrouille(pool: &PgPool, id: Uuid) -> Result<PatrouilleResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM patrouilles WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Patrouille introuvable"))?;
    Ok(row_to_patrouille(row))
}

fn row_to_patrouille(row: sqlx::postgres::PgRow) -> PatrouilleResponse {
    PatrouilleResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        zone_id: row.get("zone_id"),
        nom: row.get("nom"),
        description: row.get("description"),
        status: row.get("status"),
        date_debut: row.get("date_debut"),
        date_fin: row.get("date_fin"),
        date_debut_prevue: row.get("date_debut_prevue"),
        date_fin_prevue: row.get("date_fin_prevue"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn require_agent_patrouille_access(
    pool: &PgPool,
    auth_user: &AuthUser,
    patrouille_id: Uuid,
) -> Result<(), ApiError> {
    if !is_agent_only(auth_user) {
        return Ok(());
    }
    let agent_id = active_agent_id_for_user(pool, auth_user)
        .await?
        .ok_or_else(|| ApiError::forbidden("Agent actif introuvable pour cet utilisateur"))?;
    require_patrouille_assignment(pool, patrouille_id, agent_id).await
}

async fn require_patrouille_assignment(
    pool: &PgPool,
    patrouille_id: Uuid,
    agent_id: Uuid,
) -> Result<(), ApiError> {
    let assigned: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM patrouille_agents
            WHERE patrouille_id = $1 AND agent_id = $2
        )
        "#,
    )
    .bind(patrouille_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await?;
    if assigned {
        Ok(())
    } else {
        Err(ApiError::forbidden("Acces refuse a cette patrouille"))
    }
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
        SELECT a.id
        FROM agents a
        JOIN communes c ON c.id = a.commune_id
        WHERE a.user_id = $1
          AND a.commune_id = $2
          AND a.status = 'ACTIF'
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND c.active = true
          AND c.subscription_status IN ('ACTIVE', 'TRIAL')
          AND (c.subscription_expires_at IS NULL OR c.subscription_expires_at >= now())
        LIMIT 1
        "#,
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(pool)
    .await?;
    Ok(agent_id)
}

fn apply_filters(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    status: Option<&str>,
    agent_filter: Option<Uuid>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
    if let Some(agent_id) = agent_filter {
        qb.push(
            " AND EXISTS (SELECT 1 FROM patrouille_agents pa \
             WHERE pa.patrouille_id = patrouilles.id AND pa.agent_id = ",
        )
        .push_bind(agent_id)
        .push(")");
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Vérifie qu'un agent existe, appartient à la commune et est actif.
/// Partagé par `assign_agent` et l'affectation initiale dans `create_patrouille`.
async fn assert_agent_assignable<'e, E>(
    executor: E,
    commune_id: Uuid,
    agent_id: Uuid,
) -> Result<(), ApiError>
where
    E: sqlx::PgExecutor<'e>,
{
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM agents WHERE id = $1 AND commune_id = $2 AND deleted_at IS NULL",
    )
    .bind(agent_id)
    .bind(commune_id)
    .fetch_optional(executor)
    .await?;

    match status {
        None => Err(ApiError::not_found("Agent introuvable dans cette commune")),
        Some(s) if s != "ACTIF" => Err(ApiError::bad_request(
            "Seul un agent actif peut être assigné",
        )),
        _ => Ok(()),
    }
}

/// Valide que la fin prévue n'est pas antérieure au début prévu.
async fn load_zone_name_for_commune<'e, E>(
    executor: E,
    commune_id: Uuid,
    zone_id: Uuid,
) -> Result<String, ApiError>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_scalar(
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
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| ApiError::bad_request("Zone invalide pour cette commune"))
}

fn default_patrouille_name(zone_name: &str, date_debut_prevue: Option<DateTime<Utc>>) -> String {
    match date_debut_prevue {
        Some(date) => format!("Patrouille {zone_name} {}", date.format("%Y-%m-%d %H:%M")),
        None => format!("Patrouille {zone_name}"),
    }
}

fn validate_planning(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<(), ApiError> {
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            return Err(ApiError::bad_request(
                "date_fin_prevue doit être postérieure à date_debut_prevue",
            ));
        }
    }
    Ok(())
}
