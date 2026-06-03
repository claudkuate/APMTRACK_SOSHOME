use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::modules::audit;
use crate::modules::auth::{assign_roles, hash_password, normalize_email, roles_for_user, AuthUser};
use crate::modules::rbac::{parse_roles, Role};
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", axum::routing::get(list_users).post(create_user))
        .route("/users/{id}", axum::routing::patch(patch_user))
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
struct PatchUserRequest {
    email: Option<String>,
    password: Option<String>,
    full_name: Option<String>,
    commune_id: Option<Uuid>,
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
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

struct UserRow {
    id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn list_users(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Paginated<UserResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
    ])?;
    let pagination = Pagination::from_query(query)?;

    let (rows, total) = if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        let total = sqlx::query("SELECT COUNT(*) AS total FROM users WHERE deleted_at IS NULL")
            .fetch_one(&state.db)
            .await?
            .get("total");
        let rows = sqlx::query(
            r#"
            SELECT id, email, full_name, commune_id, active, created_at, updated_at
            FROM users
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&state.db)
        .await?;
        (rows, total)
    } else {
        let commune_id = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        let total = sqlx::query(
            "SELECT COUNT(*) AS total FROM users WHERE commune_id = $1 AND deleted_at IS NULL",
        )
        .bind(commune_id)
        .fetch_one(&state.db)
        .await?
        .get("total");
        let rows = sqlx::query(
            r#"
            SELECT id, email, full_name, commune_id, active, created_at, updated_at
            FROM users
            WHERE commune_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(commune_id)
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&state.db)
        .await?;
        (rows, total)
    };

    let mut users = Vec::with_capacity(rows.len());
    for row in rows {
        users.push(response_from_row(&state.db, row_to_user(row)).await?);
    }

    Ok(Json(Paginated::new(users, &pagination, total)))
}

async fn create_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let roles = parse_roles(&payload.roles)?;
    validate_user_assignment(&auth_user, payload.commune_id, &roles)?;

    let email = normalize_email(&payload.email)?;
    let full_name = required_text(payload.full_name, "full_name")?;
    let password_hash = hash_password(&payload.password)?;
    let active = payload.active.unwrap_or(true);
    let user_id = Uuid::new_v4();

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
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    assign_roles(&state.db, user_id, &roles).await?;
    audit::record(
        &state.db,
        Some(auth_user.id),
        "USER_CREATED",
        "users",
        Some(user_id),
        None,
        Some(json!({ "email": email, "roles": payload.roles })),
    )
    .await;

    let user = load_user_response(&state.db, user_id).await?;
    Ok(Json(user))
}

async fn patch_user(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<PatchUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;

    let existing = load_user_row(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur introuvable"))?;
    auth_user.require_commune_access(existing.commune_id.unwrap_or_else(Uuid::nil))?;

    let new_roles = match &payload.roles {
        Some(values) => parse_roles(values)?,
        None => roles_for_user(&state.db, user_id).await?,
    };
    let new_commune_id = payload.commune_id.or(existing.commune_id);
    validate_user_assignment(&auth_user, new_commune_id, &new_roles)?;

    let email = match payload.email {
        Some(email) => normalize_email(&email)?,
        None => existing.email.clone(),
    };
    let full_name = match payload.full_name {
        Some(full_name) => required_text(full_name, "full_name")?,
        None => existing.full_name.clone(),
    };
    let active = payload.active.unwrap_or(existing.active);

    if let Some(password) = payload.password {
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
        .execute(&state.db)
        .await
        .map_err(map_database_error)?;
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
        .execute(&state.db)
        .await
        .map_err(map_database_error)?;
    }

    if payload.roles.is_some() {
        assign_roles(&state.db, user_id, &new_roles).await?;
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "USER_UPDATED",
        "users",
        Some(user_id),
        Some(json!({ "email": existing.email, "commune_id": existing.commune_id })),
        Some(json!({ "email": email, "commune_id": new_commune_id })),
    )
    .await;

    let user = load_user_response(&state.db, user_id).await?;
    Ok(Json(user))
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
        roles: roles.into_iter().map(|role| role.code().to_string()).collect(),
        active: row.active,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_user_row(pool: &PgPool, user_id: Uuid) -> Result<Option<UserRow>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, full_name, commune_id, active, created_at, updated_at
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
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn validate_user_assignment(
    actor: &AuthUser,
    commune_id: Option<Uuid>,
    roles: &[Role],
) -> Result<(), ApiError> {
    let is_super_admin_target = roles.iter().any(|role| *role == Role::SuperAdmin);
    if is_super_admin_target && !actor.has_role(Role::SuperAdmin) {
        return Err(ApiError::forbidden("Seul SUPER_ADMIN peut attribuer SUPER_ADMIN"));
    }

    if is_super_admin_target && commune_id.is_some() {
        return Err(ApiError::bad_request(
            "Un SUPER_ADMIN ne doit pas etre rattache a une commune",
        ));
    }

    let needs_commune = roles.iter().any(|role| {
        matches!(
            role,
            Role::AdminCommune | Role::ApmAgent | Role::Receveur
        )
    });
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
            return Err(ApiError::forbidden("Gestion limitee a la commune de l'utilisateur"));
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
