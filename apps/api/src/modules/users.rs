use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::resolve_commune_filter;
use crate::modules::agents::{read_image_field, serve_avatar};
use crate::modules::audit;
use crate::modules::auth::{
    assign_roles_in_tx, hash_password, normalize_email, revoke_all_refresh_tokens_in_tx,
    roles_for_user, AuthUser,
};
use crate::modules::rbac::{parse_roles, Role};
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;
use crate::storage::{image_extension, MAX_AVATAR_BYTES};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", axum::routing::get(list_users).post(create_user))
        .route(
            "/users/{id}",
            axum::routing::get(get_user).patch(patch_user),
        )
        .route(
            "/users/{id}/photo",
            axum::routing::get(get_user_photo_content)
                .post(upload_user_photo)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_BYTES)),
        )
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    email: String,
    password: String,
    full_name: String,
    commune_id: Option<Uuid>,
    roles: Vec<String>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UserFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    /// Ajoute les superviseurs globaux (SUPER_ADMIN / SUPERVISEUR sans commune)
    /// au filtre commune — utilisé pour les sélecteurs d'affectation.
    include_global: Option<bool>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchUserRequest {
    email: Option<String>,
    password: Option<String>,
    full_name: Option<String>,
    commune_id: Option<Option<Uuid>>,
    roles: Option<Vec<String>>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserResponse {
    id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    roles: Vec<String>,
    active: bool,
    has_photo: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

struct UserRow {
    id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    active: bool,
    photo_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn get_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let user = load_user_response(&state.db, user_id).await?;
    match user.commune_id {
        Some(commune) => auth_user.require_commune_access(commune)?,
        None if !auth_user.has_role(Role::SuperAdmin) && !auth_user.has_role(Role::Superviseur) => {
            return Err(ApiError::forbidden("Acces interdit"));
        }
        _ => {}
    }
    Ok(Json(user))
}

/// Clause commune partagée entre le COUNT et le SELECT de `list_users`.
fn apply_user_filters(
    qb: &mut QueryBuilder<'_, sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    include_global: bool,
    active: Option<bool>,
) {
    if let Some(commune_id) = commune_filter {
        if include_global {
            qb.push(" AND (commune_id = ")
                .push_bind(commune_id)
                .push(
                    " OR (commune_id IS NULL AND EXISTS (\
                     SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
                     WHERE ur.user_id = users.id AND r.code IN ('SUPER_ADMIN', 'SUPERVISEUR'))))",
                );
        } else {
            qb.push(" AND commune_id = ").push_bind(commune_id);
        }
    }
    if let Some(active) = active {
        qb.push(" AND active = ").push_bind(active);
    }
}

async fn list_users(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<UserFilterQuery>,
) -> Result<Json<Paginated<UserResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let include_global = query.include_global.unwrap_or(false);

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM users WHERE deleted_at IS NULL");
    apply_user_filters(&mut count_qb, commune_filter, include_global, query.active);
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, email, full_name, commune_id, active, photo_url, created_at, updated_at \
         FROM users WHERE deleted_at IS NULL",
    );
    apply_user_filters(&mut qb, commune_filter, include_global, query.active);
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);
    let rows = qb.build().fetch_all(&state.db).await?;

    // Charger tous les rôles en une seule requête (évite le N+1)
    let user_rows: Vec<UserRow> = rows.into_iter().map(row_to_user).collect();
    let user_ids: Vec<Uuid> = user_rows.iter().map(|u| u.id).collect();

    let role_rows = sqlx::query(
        r#"
        SELECT ur.user_id, r.code
        FROM user_roles ur
        JOIN roles r ON ur.role_id = r.id
        WHERE ur.user_id = ANY($1)
        "#,
    )
    .bind(&user_ids[..])
    .fetch_all(&state.db)
    .await?;

    let mut roles_by_user: std::collections::HashMap<Uuid, Vec<String>> =
        std::collections::HashMap::new();
    for r in role_rows {
        roles_by_user
            .entry(r.get("user_id"))
            .or_default()
            .push(r.get::<String, _>("code"));
    }

    let users: Vec<UserResponse> = user_rows
        .into_iter()
        .map(|u| {
            let roles = roles_by_user.remove(&u.id).unwrap_or_default();
            UserResponse {
                id: u.id,
                email: u.email,
                full_name: u.full_name,
                commune_id: u.commune_id,
                roles,
                active: u.active,
                has_photo: u.photo_url.is_some(),
                created_at: u.created_at,
                updated_at: u.updated_at,
            }
        })
        .collect();

    Ok(Json(Paginated::new(users, &pagination, total)))
}

async fn create_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let roles = parse_roles(&payload.roles)?;
    validate_user_assignment(&auth_user, payload.commune_id, &roles)?;

    let email = normalize_email(&payload.email)?;
    let full_name = required_text(payload.full_name, "full_name")?;
    let password_hash = hash_password(&payload.password)?;
    let active = payload.active.unwrap_or(true);
    let user_id = Uuid::new_v4();

    let mut transaction = state.db.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(&password_hash)
    .bind(&full_name)
    .bind(payload.commune_id)
    .bind(active)
    .execute(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    assign_roles_in_tx(&mut transaction, user_id, &roles).await?;
    audit::record_for_commune_tx(
        &mut transaction,
        payload.commune_id,
        Some(auth_user.id),
        "USER_CREATED",
        "users",
        Some(user_id),
        None,
        Some(json!({ "email": email, "roles": payload.roles })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    transaction.commit().await?;

    let user = load_user_response(&state.db, user_id).await?;
    Ok(Json(user))
}

async fn patch_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;

    let existing = load_user_row(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur introuvable"))?;
    match existing.commune_id {
        Some(commune) => auth_user.require_commune_access(commune)?,
        None if !auth_user.has_role(Role::SuperAdmin) => {
            return Err(ApiError::forbidden("Acces interdit"));
        }
        _ => {}
    }

    let PatchUserRequest {
        email,
        password,
        full_name,
        commune_id,
        roles,
        active,
    } = payload;
    let roles_changed = roles.is_some();
    let audit_role_codes = roles.clone();

    let new_roles = match &roles {
        Some(values) => parse_roles(values)?,
        None => roles_for_user(&state.db, user_id).await?,
    };
    let new_commune_id = commune_id.unwrap_or(existing.commune_id);
    validate_user_assignment(&auth_user, new_commune_id, &new_roles)?;

    let email = match email {
        Some(email) => normalize_email(&email)?,
        None => existing.email.clone(),
    };
    let full_name = match full_name {
        Some(full_name) => required_text(full_name, "full_name")?,
        None => existing.full_name.clone(),
    };
    let active = active.unwrap_or(existing.active);

    let mut transaction = state.db.begin().await?;
    if let Some(password) = password {
        let password_hash = hash_password(&password)?;
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
        .bind(&full_name)
        .bind(new_commune_id)
        .bind(active)
        .execute(&mut *transaction)
        .await
        .map_err(map_database_error)?;

        revoke_all_refresh_tokens_in_tx(&mut transaction, user_id).await?;
    } else {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $2,
                full_name = $3,
                commune_id = $4,
                active = $5,
                updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&full_name)
        .bind(new_commune_id)
        .bind(active)
        .execute(&mut *transaction)
        .await
        .map_err(map_database_error)?;
    }

    if roles_changed {
        assign_roles_in_tx(&mut transaction, user_id, &new_roles).await?;
    }

    audit::record_for_commune_tx(
        &mut transaction,
        new_commune_id,
        Some(auth_user.id),
        "USER_UPDATED",
        "users",
        Some(user_id),
        Some(json!({ "email": existing.email, "commune_id": existing.commune_id })),
        Some(json!({
            "email": email,
            "commune_id": new_commune_id,
            "roles": audit_role_codes,
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    transaction.commit().await?;

    let user = load_user_response(&state.db, user_id).await?;
    Ok(Json(user))
}

/// Verifie que l'acteur peut gerer le compte cible (memes regles que patch_user).
fn require_user_management_access(actor: &AuthUser, target: &UserRow) -> Result<(), ApiError> {
    match target.commune_id {
        Some(commune) => actor.require_commune_access(commune),
        None if !actor.has_role(Role::SuperAdmin) => Err(ApiError::forbidden("Acces interdit")),
        _ => Ok(()),
    }
}

async fn upload_user_photo(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_user_row(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur introuvable"))?;
    require_user_management_access(&auth_user, &existing)?;

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;

    let (data, content_type) = read_image_field(&mut multipart).await?;
    let object_key = format!("avatars/users/{}.{}", user_id, image_extension(&content_type));
    storage
        .put(&object_key, data.as_ref(), &content_type)
        .await
        .map_err(|error| {
            tracing::error!(%error, "user photo upload failed");
            ApiError::internal("Echec de l'enregistrement de la photo")
        })?;

    sqlx::query("UPDATE users SET photo_url = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(user_id)
        .bind(&object_key)
        .execute(&state.db)
        .await
        .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        existing.commune_id,
        Some(auth_user.id),
        "USER_PHOTO_UPLOADED",
        "users",
        Some(user_id),
        None,
        Some(json!({ "content_type": content_type })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(load_user_response(&state.db, user_id).await?),
    ))
}

async fn get_user_photo_content(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let existing = load_user_row(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur introuvable"))?;
    require_user_management_access(&auth_user, &existing)?;
    serve_avatar(&state, existing.photo_url).await
}

pub async fn load_user_response(pool: &PgPool, user_id: Uuid) -> Result<UserResponse, ApiError> {
    let row = load_user_row(pool, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur introuvable"))?;

    response_from_row(pool, row).await
}

async fn response_from_row(pool: &PgPool, row: UserRow) -> Result<UserResponse, ApiError> {
    let roles = roles_for_user(pool, row.id).await?;

    Ok(UserResponse {
        id: row.id,
        email: row.email,
        full_name: row.full_name,
        commune_id: row.commune_id,
        roles: roles
            .into_iter()
            .map(|role| role.code().to_string())
            .collect(),
        active: row.active,
        has_photo: row.photo_url.is_some(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_user_row(pool: &PgPool, user_id: Uuid) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, full_name, commune_id, active, photo_url, created_at, updated_at
        FROM users
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(row_to_user))
}

fn row_to_user(row: sqlx::postgres::PgRow) -> UserRow {
    UserRow {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        active: row.get("active"),
        photo_url: row.get("photo_url"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn validate_user_assignment(
    actor: &AuthUser,
    commune_id: Option<Uuid>,
    roles: &[Role],
) -> Result<(), ApiError> {
    let is_super_admin_target = roles.contains(&Role::SuperAdmin);
    if is_super_admin_target && !actor.has_role(Role::SuperAdmin) {
        return Err(ApiError::forbidden(
            "Seul SUPER_ADMIN peut attribuer SUPER_ADMIN",
        ));
    }

    if is_super_admin_target && commune_id.is_some() {
        return Err(ApiError::bad_request(
            "Un SUPER_ADMIN ne doit pas etre rattache a une commune",
        ));
    }

    let needs_commune = roles
        .iter()
        .any(|role| matches!(role, Role::AdminCommune | Role::ApmAgent | Role::Receveur));
    if needs_commune && commune_id.is_none() {
        return Err(ApiError::bad_request(
            "Ce role doit etre rattache a une commune",
        ));
    }

    if !actor.has_role(Role::SuperAdmin) {
        let actor_commune_id = actor
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Administrateur sans commune"))?;
        if commune_id != Some(actor_commune_id) {
            return Err(ApiError::forbidden(
                "Gestion limitee a la commune de l'utilisateur",
            ));
        }
    }

    Ok(())
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}
