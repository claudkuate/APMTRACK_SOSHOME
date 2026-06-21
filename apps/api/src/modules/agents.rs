use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::modules::audit;
use crate::modules::auth::{assign_roles_in_tx, hash_password, roles_for_user, AuthUser};
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;
use crate::storage::{content_type_for_key, image_extension, MAX_AVATAR_BYTES};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/agents",
            axum::routing::get(list_agents).post(create_agent),
        )
        .route("/agents/import-csv", axum::routing::post(import_agents_csv))
        .route(
            "/agents/{id}/account",
            axum::routing::post(link_agent_mobile_account),
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
        .route(
            "/agents/{id}/photo",
            axum::routing::get(get_agent_photo_content)
                .post(upload_agent_photo)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_BYTES)),
        )
}

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route(
            "/agents/verify/{matricule}",
            axum::routing::get(verify_agent_public),
        )
        .route(
            "/agents/verify/{matricule}/photo",
            axum::routing::get(verify_agent_photo_public),
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
struct ImportAgentsQuery {
    commune_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Serialize)]
struct ImportAgentsResponse {
    created: usize,
    updated: usize,
    skipped: usize,
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CsvAgentRow {
    matricule: String,
    full_name: String,
    date_prise_fonction: Option<NaiveDate>,
    telephone: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PatchAgentRequest {
    matricule: Option<String>,
    full_name: Option<String>,
    commune_id: Option<Uuid>,
    status: Option<String>,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinkAgentAccountRequest {
    email: String,
    password: String,
    active: Option<bool>,
}

#[derive(Debug, Serialize)]
struct LinkAgentAccountResponse {
    agent_id: Uuid,
    user_id: Uuid,
    email: String,
    full_name: String,
    commune_id: Uuid,
    linked: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentResponse {
    id: Uuid,
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    status: String,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    has_photo: bool,
    telephone: Option<String>,
    email: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PublicAgentVerification {
    matricule: String,
    full_name: String,
    commune_code: String,
    commune_nom: String,
    status: String,
    active: bool,
    has_photo: bool,
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
            id, matricule, full_name, commune_id, status,
            date_prise_fonction, photo_url, telephone, email
        )
        VALUES ($1, $2, $3, $4, 'ACTIF', $5, $6, $7, $8)
        "#,
    )
    .bind(agent_id)
    .bind(required_text(payload.matricule, "matricule")?)
    .bind(required_text(payload.full_name, "full_name")?)
    .bind(payload.commune_id)
    .bind(payload.date_prise_fonction)
    .bind(clean_optional(payload.photo_url))
    .bind(clean_optional(payload.telephone))
    .bind(clean_optional(payload.email))
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

async fn import_agents_csv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ImportAgentsQuery>,
    body: Bytes,
) -> Result<Json<ImportAgentsResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(query.commune_id)?;

    let content = std::str::from_utf8(&body)
        .map_err(|_| ApiError::bad_request("Le fichier CSV doit etre encode en UTF-8"))?;
    if content.trim().is_empty() {
        return Err(ApiError::bad_request("Le fichier CSV est vide"));
    }

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();
    let mut tx = state.db.begin().await?;

    for (idx, row) in reader.deserialize::<CsvAgentRow>().enumerate() {
        let line = idx + 2;
        let row = match row {
            Ok(row) => row,
            Err(error) => {
                skipped += 1;
                errors.push(format!("Ligne {line}: {error}"));
                continue;
            }
        };

        let matricule = match required_text(row.matricule, "matricule") {
            Ok(value) => value,
            Err(error) => {
                skipped += 1;
                errors.push(format!("Ligne {line}: {error}"));
                continue;
            }
        };
        let full_name = match required_text(row.full_name, "full_name") {
            Ok(value) => value,
            Err(error) => {
                skipped += 1;
                errors.push(format!("Ligne {line}: {error}"));
                continue;
            }
        };
        let existing_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM agents WHERE lower(matricule) = lower($1) AND deleted_at IS NULL",
        )
        .bind(&matricule)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(agent_id) = existing_id {
            sqlx::query(
                r#"
                UPDATE agents
                SET full_name = $2,
                    commune_id = $3,
                    date_prise_fonction = $4,
                    telephone = $5,
                    email = $6,
                    updated_at = now()
                WHERE id = $1 AND deleted_at IS NULL
                "#,
            )
            .bind(agent_id)
            .bind(&full_name)
            .bind(query.commune_id)
            .bind(row.date_prise_fonction)
            .bind(clean_optional(row.telephone))
            .bind(clean_optional(row.email))
            .execute(&mut *tx)
            .await
            .map_err(map_database_error)?;
            updated += 1;
        } else {
            sqlx::query(
                r#"
                INSERT INTO agents (
                    id, matricule, full_name, commune_id, status,
                    date_prise_fonction, telephone, email
                )
                VALUES ($1, $2, $3, $4, 'ACTIF', $5, $6, $7)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(&matricule)
            .bind(&full_name)
            .bind(query.commune_id)
            .bind(row.date_prise_fonction)
            .bind(clean_optional(row.telephone))
            .bind(clean_optional(row.email))
            .execute(&mut *tx)
            .await
            .map_err(map_database_error)?;
            created += 1;
        }
    }

    audit::record_for_commune_tx(
        &mut tx,
        Some(query.commune_id),
        Some(auth_user.id),
        "AGENTS_IMPORTED_CSV",
        "agents",
        None,
        None,
        Some(json!({
            "created": created,
            "updated": updated,
            "skipped": skipped,
            "errors": errors.len()
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    tx.commit().await?;

    Ok(Json(ImportAgentsResponse {
        created,
        updated,
        skipped,
        errors,
    }))
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
            status = $5,
            date_prise_fonction = $6,
            photo_url = $7,
            telephone = $8,
            email = $9,
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
    .bind(&status)
    .bind(payload.date_prise_fonction.or(existing.date_prise_fonction))
    .bind(payload.photo_url.or(existing.photo_url.clone()))
    .bind(payload.telephone.or(existing.telephone.clone()))
    .bind(payload.email.or(existing.email.clone()))
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

async fn link_agent_mobile_account(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    ApiJson(payload): ApiJson<LinkAgentAccountRequest>,
) -> Result<Json<LinkAgentAccountResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;

    let email = normalize_email(&payload.email)?;
    let password_hash = hash_password(payload.password.trim())?;
    let active = payload.active.unwrap_or(true);

    let existing_user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE lower(email) = lower($1) AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let user_id = existing_user_id.unwrap_or_else(Uuid::new_v4);

    let conflicting_agent: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM agents
        WHERE user_id = $1
          AND id <> $2
          AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(agent_id)
    .fetch_optional(&state.db)
    .await?;
    if conflicting_agent.is_some() {
        return Err(ApiError::bad_request(
            "Ce compte utilisateur est deja lie a un autre agent",
        ));
    }

    let mut roles = match existing_user_id {
        Some(id) => roles_for_user(&state.db, id).await?,
        None => Vec::new(),
    };
    if !roles.contains(&Role::ApmAgent) {
        roles.push(Role::ApmAgent);
    }

    let mut tx = state.db.begin().await?;
    if existing_user_id.is_some() {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $2,
                password_hash = $3,
                full_name = $4,
                commune_id = $5,
                active = $6,
                updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(&agent.full_name)
        .bind(agent.commune_id)
        .bind(active)
        .execute(&mut *tx)
        .await
        .map_err(map_database_error)?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(&agent.full_name)
        .bind(agent.commune_id)
        .bind(active)
        .execute(&mut *tx)
        .await
        .map_err(map_database_error)?;
    }

    assign_roles_in_tx(&mut tx, user_id, &roles).await?;
    sqlx::query(
        r#"
        UPDATE agents
        SET user_id = $2, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune_tx(
        &mut tx,
        Some(agent.commune_id),
        Some(auth_user.id),
        "AGENT_ACCOUNT_LINKED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({ "email": email.clone(), "user_id": user_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    tx.commit().await?;

    Ok(Json(LinkAgentAccountResponse {
        agent_id,
        user_id,
        email,
        full_name: agent.full_name,
        commune_id: agent.commune_id,
        linked: true,
    }))
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
            a.status,
            a.photo_url,
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
        active: status == "ACTIF",
        status,
        has_photo: row.get::<Option<String>, _>("photo_url").is_some(),
    }))
}

/// Streame l'avatar public d'un agent (sans authentification).
async fn verify_agent_photo_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(matricule): Path<String>,
) -> Result<Response, ApiError> {
    state.rate_limiter.check(
        "public:agents:photo",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let matricule = required_text(matricule, "matricule")?;
    let object_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT a.photo_url
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
    .flatten();

    serve_avatar(&state, object_key).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Photo de profil (avatar, object storage MinIO/S3)
// ─────────────────────────────────────────────────────────────────────────────

/// Lit un champ image unique d'un corps multipart et valide type/taille.
pub(crate) async fn read_image_field(
    multipart: &mut Multipart,
) -> Result<(Bytes, String), ApiError> {
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
    if data.len() > MAX_AVATAR_BYTES {
        return Err(ApiError::bad_request("Image trop volumineuse (max 5 Mo)"));
    }
    Ok((data, content_type))
}

/// Charge un objet avatar depuis le stockage et le renvoie en reponse HTTP.
pub(crate) async fn serve_avatar(
    state: &AppState,
    object_key: Option<String>,
) -> Result<Response, ApiError> {
    let object_key = object_key.ok_or_else(|| ApiError::not_found("Photo introuvable"))?;
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;
    let bytes = storage.get(&object_key).await.map_err(|error| {
        tracing::error!(%error, "agent photo download failed");
        ApiError::internal("Echec du telechargement de la photo")
    })?;
    let content_type = content_type_for_key(&object_key);
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

async fn upload_agent_photo(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AgentResponse>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;

    let (data, content_type) = read_image_field(&mut multipart).await?;
    let object_key = format!("avatars/agents/{}.{}", agent_id, image_extension(&content_type));
    storage
        .put(&object_key, data.as_ref(), &content_type)
        .await
        .map_err(|error| {
            tracing::error!(%error, "agent photo upload failed");
            ApiError::internal("Echec de l'enregistrement de la photo")
        })?;

    sqlx::query("UPDATE agents SET photo_url = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(agent_id)
        .bind(&object_key)
        .execute(&state.db)
        .await
        .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(agent.commune_id),
        Some(auth_user.id),
        "AGENT_PHOTO_UPLOADED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({ "content_type": content_type })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(load_agent(&state.db, agent_id).await?),
    ))
}

async fn get_agent_photo_content(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;
    serve_avatar(&state, agent.photo_url).await
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
        status: row.get("status"),
        date_prise_fonction: row.get("date_prise_fonction"),
        has_photo: row.get::<Option<String>, _>("photo_url").is_some(),
        photo_url: row.get("photo_url"),
        telephone: row.get("telephone"),
        email: row.get("email"),
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

fn normalize_email(value: &str) -> Result<String, ApiError> {
    let email = value.trim().to_ascii_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::bad_request("Email invalide"));
    }
    Ok(email)
}
