use std::env;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::modules::audit;
use crate::modules::rbac::{has_any_role, has_role, Role};
use crate::state::AppState;

const REFRESH_COOKIE_NAME: &str = "apmtrack_refresh";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", axum::routing::post(login))
        .route("/refresh", axum::routing::post(refresh))
        .route("/refresh-cookie", axum::routing::post(refresh_cookie))
        .route("/logout", axum::routing::post(logout))
        .route("/me", axum::routing::get(me))
        .route("/change-password", axum::routing::post(change_password))
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub commune_id: Option<Uuid>,
    pub roles: Vec<Role>,
    /// Compte provisionne automatiquement : le mot de passe temporaire doit etre remplace.
    pub must_change_password: bool,
    /// Echeance connue de l'acces communal. `None` pour les acteurs globaux.
    pub commune_access_expires_at: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuthUser {
    pub fn has_role(&self, role: Role) -> bool {
        has_role(&self.roles, role)
    }

    pub fn require_any_role(&self, allowed: &[Role]) -> Result<(), ApiError> {
        if has_any_role(&self.roles, allowed) {
            Ok(())
        } else {
            Err(ApiError::forbidden("Role non autorise pour cette action"))
        }
    }

    pub fn can_access_commune(&self, target: Uuid) -> bool {
        self.has_role(Role::SuperAdmin)
            || (self.has_role(Role::Superviseur) && self.commune_id.is_none())
            || self.commune_id == Some(target)
    }

    pub fn require_commune_access(&self, target: Uuid) -> Result<(), ApiError> {
        if self.can_access_commune(target) {
            Ok(())
        } else {
            Err(ApiError::forbidden("Acces interdit a cette commune"))
        }
    }
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    /// Adresse email **ou** matricule d'agent. Le nom de champ reste `email` pour ne pas
    /// casser les clients existants ; `identifier` et `matricule` sont acceptes en alias.
    #[serde(alias = "identifier", alias = "matricule")]
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct LogoutRequest {
    refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    expires_in_seconds: i64,
    user: MeResponse,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub commune_id: Option<Uuid>,
    pub roles: Vec<String>,
    pub active: bool,
    pub commune_access_expires_at: Option<DateTime<Utc>>,
    /// Le client doit imposer la definition d'un nouveau mot de passe avant tout usage.
    pub must_change_password: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    email: String,
    roles: Vec<String>,
    commune_id: Option<Uuid>,
    iat: usize,
    exp: usize,
}

struct LoginUser {
    id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    password_hash: String,
    active: bool,
    must_change_password: bool,
}

struct RefreshTokenRecord {
    id: Uuid,
    user_id: Uuid,
    token_hash: String,
    expires_at: chrono::DateTime<Utc>,
    revoked_at: Option<chrono::DateTime<Utc>>,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::unauthorized("Token bearer manquant"))?;

        let claims = decode_access_token(token, &state.config.jwt_secret)?;
        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_| ApiError::unauthorized("Token invalide"))?;

        let ip_address = parts
            .headers
            .get("x-forwarded-for")
            .or_else(|| parts.headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

        let user_agent = parts
            .headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let mut user = load_auth_user(&state.db, user_id).await?;
        user.ip_address = ip_address;
        user.user_agent = user_agent;
        Ok(user)
    }
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<LoginRequest>,
) -> Result<Response, ApiError> {
    state.rate_limiter.check(
        "auth:login",
        &headers,
        state.config.rate_limit_login_max,
        state.config.rate_limit_window_seconds,
    )?;

    let identifier = normalize_login_identifier(&payload.email)?;
    let user = match load_login_user(&state.db, &identifier).await? {
        Some(user) => user,
        None => {
            audit::record(
                &state.db,
                None,
                "AUTH_LOGIN_FAILED",
                "users",
                None,
                None,
                Some(json!({ "identifier": identifier })),
                None,
                None,
            )
            .await;
            return Err(ApiError::unauthorized("Identifiants invalides"));
        }
    };

    if !user.active || !verify_password(&payload.password, &user.password_hash)? {
        audit::record_for_commune(
            &state.db,
            user.commune_id,
            Some(user.id),
            "AUTH_LOGIN_FAILED",
            "users",
            Some(user.id),
            None,
            Some(json!({ "email": user.email })),
            None,
            None,
        )
        .await;
        return Err(ApiError::unauthorized("Identifiants invalides"));
    }

    let roles = roles_for_user(&state.db, user.id).await?;
    let commune_access_expires_at = match ensure_commune_subscription_access(
        &state.db,
        user.commune_id,
        &roles,
    )
    .await
    {
        Ok(expires_at) => expires_at,
        Err(error) => {
            audit::record_for_commune(
                &state.db,
                user.commune_id,
                Some(user.id),
                "AUTH_LOGIN_BLOCKED_SUBSCRIPTION",
                "users",
                Some(user.id),
                None,
                Some(json!({ "email": user.email })),
                None,
                None,
            )
            .await;
            return Err(error);
        }
    };
    let response = issue_tokens(
        &state.db,
        &state.config,
        user.id,
        user.email.clone(),
        user.full_name.clone(),
        user.commune_id,
        roles,
        user.must_change_password,
        commune_access_expires_at,
    )
    .await?;

    audit::record_for_commune(
        &state.db,
        user.commune_id,
        Some(user.id),
        "AUTH_LOGIN_SUCCEEDED",
        "users",
        Some(user.id),
        None,
        None,
        None,
        None,
    )
    .await;

    token_response_with_cookie(response, &state.config)
}

async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(payload): ApiJson<RefreshRequest>,
) -> Result<Response, ApiError> {
    state.rate_limiter.check(
        "auth:refresh",
        &headers,
        state.config.rate_limit_login_max,
        state.config.rate_limit_window_seconds,
    )?;

    let response = refresh_with_token(&state, &payload.refresh_token).await?;
    token_response_with_cookie(response, &state.config)
}

async fn refresh_cookie(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    state.rate_limiter.check(
        "auth:refresh-cookie",
        &headers,
        state.config.rate_limit_login_max,
        state.config.rate_limit_window_seconds,
    )?;

    let refresh_token = refresh_token_from_cookie(&headers)?;
    let response = refresh_with_token(&state, &refresh_token).await?;
    token_response_with_cookie(response, &state.config)
}

async fn refresh_with_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<TokenResponse, ApiError> {
    let (token_id, secret) = parse_refresh_token(refresh_token)?;
    let mut transaction = state.db.begin().await?;
    let record = load_refresh_token_for_update(&mut transaction, token_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Refresh token invalide"))?;

    if record.revoked_at.is_some() || record.expires_at <= Utc::now() {
        return Err(ApiError::unauthorized("Refresh token expire ou revoque"));
    }

    if !verify_password(&secret, &record.token_hash)? {
        return Err(ApiError::unauthorized("Refresh token invalide"));
    }

    revoke_refresh_token_in_tx(&mut transaction, token_id, record.user_id).await?;
    let auth_user = load_auth_user_in_tx(&mut transaction, record.user_id).await?;
    let response = issue_tokens_in_tx(
        &mut transaction,
        &state.config,
        auth_user.id,
        auth_user.email.clone(),
        auth_user.full_name.clone(),
        auth_user.commune_id,
        auth_user.roles.clone(),
        auth_user.must_change_password,
        auth_user.commune_access_expires_at,
    )
    .await?;
    audit::record_for_commune_tx(
        &mut transaction,
        auth_user.commune_id,
        Some(auth_user.id),
        "AUTH_TOKEN_REFRESHED",
        "refresh_tokens",
        Some(record.id),
        None,
        None,
        None,
        None,
    )
    .await;
    transaction.commit().await?;

    Ok(response)
}

async fn logout(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<LogoutRequest>,
) -> Result<Response, ApiError> {
    if let Some(refresh_token) = payload.refresh_token {
        let (token_id, _) = parse_refresh_token(&refresh_token)?;
        revoke_refresh_token(&state.db, token_id, auth_user.id).await?;
    } else {
        revoke_all_refresh_tokens(&state.db, auth_user.id).await?;
    }

    audit::record_for_commune(
        &state.db,
        auth_user.commune_id,
        Some(auth_user.id),
        "AUTH_LOGOUT",
        "users",
        Some(auth_user.id),
        None,
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear_refresh_cookie(&state.config)).map_err(|error| {
            tracing::error!(%error, "clear refresh cookie header failed");
            ApiError::internal("Impossible de finaliser la deconnexion")
        })?,
    );
    Ok(response)
}

async fn me(auth_user: AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        id: auth_user.id,
        email: auth_user.email,
        full_name: auth_user.full_name,
        commune_id: auth_user.commune_id,
        must_change_password: auth_user.must_change_password,
        roles: auth_user
            .roles
            .into_iter()
            .map(|role| role.code().to_string())
            .collect(),
        active: true,
        commune_access_expires_at: auth_user.commune_access_expires_at,
    })
}

/// Changement de mot de passe par l'utilisateur lui-meme.
///
/// C'est la sortie du provisionnement : elle leve `must_change_password` et revoque les
/// refresh tokens, pour que le mot de passe temporaire communique par l'administrateur
/// cesse d'ouvrir une session.
async fn change_password(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<ChangePasswordRequest>,
) -> Result<Json<MeResponse>, ApiError> {
    let current_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(auth_user.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::unauthorized("Utilisateur introuvable"))?;

    if !verify_password(&payload.current_password, &current_hash)? {
        return Err(ApiError::unauthorized("Mot de passe actuel invalide"));
    }

    let new_password = payload.new_password.trim();
    if new_password == payload.current_password.trim() {
        return Err(ApiError::bad_request(
            "Le nouveau mot de passe doit etre different de l'actuel",
        ));
    }
    let new_hash = hash_password(new_password)?;

    let mut transaction = state.db.begin().await?;
    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $2, must_change_password = FALSE, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(auth_user.id)
    .bind(&new_hash)
    .execute(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = now()
        WHERE user_id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(auth_user.id)
    .execute(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune_tx(
        &mut transaction,
        auth_user.commune_id,
        Some(auth_user.id),
        "AUTH_PASSWORD_CHANGED",
        "users",
        Some(auth_user.id),
        None,
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    transaction.commit().await?;

    Ok(Json(MeResponse {
        id: auth_user.id,
        email: auth_user.email,
        full_name: auth_user.full_name,
        commune_id: auth_user.commune_id,
        roles: auth_user
            .roles
            .iter()
            .map(|role| role.code().to_string())
            .collect(),
        active: true,
        commune_access_expires_at: auth_user.commune_access_expires_at,
        must_change_password: false,
    }))
}

pub async fn seed_super_admin(pool: &PgPool) -> anyhow::Result<()> {
    let email = normalize_email(&env::var("SEED_SUPER_ADMIN_EMAIL")?)
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let password = env::var("SEED_SUPER_ADMIN_PASSWORD")?;
    let full_name =
        env::var("SEED_SUPER_ADMIN_FULL_NAME").unwrap_or_else(|_| "APMTRACK Super Admin".into());

    if password.len() < 12 {
        anyhow::bail!("SEED_SUPER_ADMIN_PASSWORD must be at least 12 characters");
    }

    let password_hash = hash_password(&password).map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let user_id = match sqlx::query(
        r#"
        SELECT id FROM users
        WHERE lower(email) = lower($1) AND deleted_at IS NULL
        "#,
    )
    .bind(&email)
    .fetch_optional(pool)
    .await?
    {
        Some(row) => {
            let id: Uuid = row.get("id");
            sqlx::query(
                r#"
                UPDATE users
                SET full_name = $2,
                    password_hash = $3,
                    commune_id = NULL,
                    active = TRUE,
                    updated_at = now()
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(&full_name)
            .bind(&password_hash)
            .execute(pool)
            .await?;
            id
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
                VALUES ($1, $2, $3, $4, NULL, TRUE)
                "#,
            )
            .bind(id)
            .bind(&email)
            .bind(&password_hash)
            .bind(&full_name)
            .execute(pool)
            .await?;
            id
        }
    };

    assign_roles(pool, user_id, &[Role::SuperAdmin]).await?;
    audit::record(
        pool,
        None,
        "SEED_SUPER_ADMIN",
        "users",
        Some(user_id),
        None,
        Some(json!({ "email": email })),
        None,
        None,
    )
    .await;

    tracing::info!(%email, "super admin seed completed");
    Ok(())
}

pub async fn load_auth_user(pool: &PgPool, user_id: Uuid) -> Result<AuthUser, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, full_name, commune_id, active, must_change_password
        FROM users
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::unauthorized("Utilisateur introuvable"))?;

    let active: bool = row.get("active");
    if !active {
        return Err(ApiError::unauthorized("Utilisateur inactif"));
    }

    let roles = roles_for_user(pool, user_id).await?;
    if roles.is_empty() {
        return Err(ApiError::forbidden("Utilisateur sans role actif"));
    }

    let commune_id = row.get("commune_id");
    let commune_access_expires_at =
        ensure_commune_subscription_access(pool, commune_id, &roles).await?;

    Ok(AuthUser {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        commune_id,
        roles,
        must_change_password: row.get("must_change_password"),
        commune_access_expires_at,
        ip_address: None,
        user_agent: None,
    })
}

async fn load_auth_user_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> Result<AuthUser, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, full_name, commune_id, active, must_change_password
        FROM users
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| ApiError::unauthorized("Utilisateur introuvable"))?;

    let active: bool = row.get("active");
    if !active {
        return Err(ApiError::unauthorized("Utilisateur inactif"));
    }

    let roles = roles_for_user_in_tx(transaction, user_id).await?;
    if roles.is_empty() {
        return Err(ApiError::forbidden("Utilisateur sans role actif"));
    }

    let commune_id = row.get("commune_id");
    let commune_access_expires_at =
        ensure_commune_subscription_access_in_tx(transaction, commune_id, &roles).await?;

    Ok(AuthUser {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        commune_id,
        roles,
        must_change_password: row.get("must_change_password"),
        commune_access_expires_at,
        ip_address: None,
        user_agent: None,
    })
}

#[derive(Debug)]
struct CommuneAccessSnapshot {
    active: bool,
    subscription_status: String,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    access_active: bool,
}

fn subscription_exempt(commune_id: Option<Uuid>, roles: &[Role]) -> bool {
    roles.contains(&Role::SuperAdmin)
        || (commune_id.is_none() && roles.contains(&Role::Superviseur))
}

fn evaluate_commune_access(
    snapshot: CommuneAccessSnapshot,
) -> Result<Option<DateTime<Utc>>, ApiError> {
    if snapshot.access_active {
        return Ok(snapshot.subscription_expires_at);
    }

    let now = Utc::now();
    let reason = if snapshot.subscription_status == "SUSPENDED" {
        "SUSPENDED"
    } else if !snapshot.active {
        "INACTIVE"
    } else if snapshot.subscription_status == "EXPIRED"
        || snapshot
            .subscription_expires_at
            .is_none_or(|expires_at| expires_at < now)
    {
        "EXPIRED"
    } else if snapshot
        .subscription_started_at
        .is_none_or(|started_at| started_at > now)
    {
        "NOT_STARTED"
    } else {
        "PAYMENT_REQUIRED"
    };

    Err(ApiError::commune_subscription_inactive(
        reason,
        snapshot.subscription_expires_at,
    ))
}

fn row_to_commune_access(row: sqlx::postgres::PgRow) -> CommuneAccessSnapshot {
    CommuneAccessSnapshot {
        active: row.get("active"),
        subscription_status: row.get("subscription_status"),
        subscription_started_at: row.get("subscription_started_at"),
        subscription_expires_at: row.get("subscription_expires_at"),
        access_active: row.get("access_active"),
    }
}

async fn ensure_commune_subscription_access(
    pool: &PgPool,
    commune_id: Option<Uuid>,
    roles: &[Role],
) -> Result<Option<DateTime<Utc>>, ApiError> {
    if subscription_exempt(commune_id, roles) {
        return Ok(None);
    }
    let Some(commune_id) = commune_id else {
        return Ok(None);
    };

    let row = sqlx::query(
        r#"
        SELECT active, subscription_status, subscription_started_at,
               subscription_expires_at,
               commune_subscription_is_active(id, now()) AS access_active
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::commune_subscription_inactive("INACTIVE", None))?;

    evaluate_commune_access(row_to_commune_access(row))
}

async fn ensure_commune_subscription_access_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Option<Uuid>,
    roles: &[Role],
) -> Result<Option<DateTime<Utc>>, ApiError> {
    if subscription_exempt(commune_id, roles) {
        return Ok(None);
    }
    let Some(commune_id) = commune_id else {
        return Ok(None);
    };

    let row = sqlx::query(
        r#"
        SELECT active, subscription_status, subscription_started_at,
               subscription_expires_at,
               commune_subscription_is_active(id, now()) AS access_active
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| ApiError::commune_subscription_inactive("INACTIVE", None))?;

    evaluate_commune_access(row_to_commune_access(row))
}

pub async fn roles_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Role>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT r.code
        FROM roles r
        INNER JOIN user_roles ur ON ur.role_id = r.id
        WHERE ur.user_id = $1
        ORDER BY r.code
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| Role::from_code(row.get::<String, _>("code").as_str()))
        .collect())
}

async fn roles_for_user_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> Result<Vec<Role>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT r.code
        FROM roles r
        INNER JOIN user_roles ur ON ur.role_id = r.id
        WHERE ur.user_id = $1
        ORDER BY r.code
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| Role::from_code(row.get::<String, _>("code").as_str()))
        .collect())
}

pub async fn assign_roles(pool: &PgPool, user_id: Uuid, roles: &[Role]) -> Result<(), ApiError> {
    let mut transaction = pool.begin().await?;
    assign_roles_in_tx(&mut transaction, user_id, roles).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn assign_roles_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    roles: &[Role],
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;

    for role in roles {
        sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role_id)
            SELECT $1, id FROM roles WHERE code = $2
            "#,
        )
        .bind(user_id)
        .bind(role.code())
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    if password.len() < 8 {
        return Err(ApiError::bad_request(
            "Le mot de passe doit contenir au moins 8 caracteres",
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| {
            tracing::error!(%error, "password hash failed");
            ApiError::internal("Impossible de securiser le mot de passe")
        })
}

/// Mot de passe temporaire d'un compte provisionne automatiquement.
///
/// Tire sur `OsRng` et **jamais** derive du matricule : celui-ci est publiquement
/// verifiable via `/public/agents/verify/{matricule}`, un mot de passe qui en decoulerait
/// serait devinable par n'importe qui. L'alphabet exclut les caracteres ambigus (0/O, 1/l/I)
/// car l'identifiant est recopie a la main ou dicte a l'agent.
pub fn generate_temp_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    const LENGTH: usize = 12;

    let modulo = ALPHABET.len() as u8;
    // Tirage avec rejet : on ecarte la queue non divisible pour rester uniforme.
    let limit = u8::MAX - (u8::MAX % modulo);
    let mut password = String::with_capacity(LENGTH);
    let mut buffer = [0u8; 32];
    while password.len() < LENGTH {
        OsRng.fill_bytes(&mut buffer);
        for byte in buffer {
            if password.len() == LENGTH {
                break;
            }
            if byte < limit {
                password.push(ALPHABET[(byte % modulo) as usize] as char);
            }
        }
    }
    password
}

/// Identifiant de connexion accepte : adresse email complete **ou** matricule d'agent.
pub fn normalize_login_identifier(value: &str) -> Result<String, ApiError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.contains('@') {
        return normalize_email(&normalized);
    }

    let err = || ApiError::bad_request("Identifiant invalide");
    if normalized.is_empty() || normalized.len() > 128 {
        return Err(err());
    }
    if normalized.chars().any(char::is_whitespace) {
        return Err(err());
    }
    Ok(normalized)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, ApiError> {
    let parsed = PasswordHash::new(password_hash).map_err(|error| {
        tracing::warn!(%error, "stored password hash is invalid");
        ApiError::internal("Hash de mot de passe invalide")
    })?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn normalize_email(email: &str) -> Result<String, ApiError> {
    let normalized = email.trim().to_ascii_lowercase();
    let err = || ApiError::bad_request("Adresse email invalide");

    if normalized.is_empty() {
        return Err(err());
    }

    let (local, domain) = normalized.split_once('@').ok_or_else(err)?;

    if local.is_empty() || domain.is_empty() {
        return Err(err());
    }

    // Le domaine doit contenir au moins un point et aucune espace
    if !domain.contains('.') || domain.contains(' ') {
        return Err(err());
    }

    // Ni partie locale ni domaine ne peut commencer ou finir par un point
    if local.starts_with('.') || local.ends_with('.') {
        return Err(err());
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err(err());
    }

    if normalized.len() > 254 {
        return Err(err());
    }

    Ok(normalized)
}

/// Resout l'identifiant de connexion : adresse email **ou** matricule d'agent.
///
/// Un agent est provisionne avec une adresse technique qu'il ne connait pas ; son
/// identifiant naturel est son matricule. La resolution par matricule passe par
/// `agents.user_id`, donc un agent sans compte lie reste non connectable.
async fn load_login_user(pool: &PgPool, identifier: &str) -> Result<Option<LoginUser>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT u.id, u.email, u.password_hash, u.full_name, u.commune_id, u.active,
               u.must_change_password
        FROM users u
        WHERE u.deleted_at IS NULL
          AND (
                lower(u.email) = lower($1)
                OR u.id = (
                    SELECT a.user_id
                    FROM agents a
                    WHERE lower(a.matricule) = lower($1)
                      AND a.user_id IS NOT NULL
                      AND a.deleted_at IS NULL
                    LIMIT 1
                )
              )
        LIMIT 1
        "#,
    )
    .bind(identifier)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| LoginUser {
        id: row.get("id"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        active: row.get("active"),
        must_change_password: row.get("must_change_password"),
    }))
}

async fn issue_tokens(
    pool: &PgPool,
    config: &AppConfig,
    user_id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    roles: Vec<Role>,
    must_change_password: bool,
    commune_access_expires_at: Option<DateTime<Utc>>,
) -> Result<TokenResponse, ApiError> {
    let access_token = create_access_token(config, user_id, &email, commune_id, &roles)?;
    let (refresh_token_id, refresh_token, refresh_secret) = generate_refresh_token();
    let refresh_hash = hash_password(&refresh_secret)?;
    let refresh_expires_at = Utc::now() + Duration::days(config.jwt_refresh_token_ttl_days);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(refresh_token_id)
    .bind(user_id)
    .bind(refresh_hash)
    .bind(refresh_expires_at)
    .execute(pool)
    .await
    .map_err(map_database_error)?;

    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in_seconds: config.jwt_access_token_ttl_minutes * 60,
        user: MeResponse {
            id: user_id,
            email,
            full_name,
            commune_id,
            roles: roles
                .into_iter()
                .map(|role| role.code().to_string())
                .collect(),
            active: true,
            commune_access_expires_at,
            must_change_password,
        },
    })
}

async fn issue_tokens_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    config: &AppConfig,
    user_id: Uuid,
    email: String,
    full_name: String,
    commune_id: Option<Uuid>,
    roles: Vec<Role>,
    must_change_password: bool,
    commune_access_expires_at: Option<DateTime<Utc>>,
) -> Result<TokenResponse, ApiError> {
    let access_token = create_access_token(config, user_id, &email, commune_id, &roles)?;
    let (refresh_token_id, refresh_token, refresh_secret) = generate_refresh_token();
    let refresh_hash = hash_password(&refresh_secret)?;
    let refresh_expires_at = Utc::now() + Duration::days(config.jwt_refresh_token_ttl_days);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(refresh_token_id)
    .bind(user_id)
    .bind(refresh_hash)
    .bind(refresh_expires_at)
    .execute(&mut **transaction)
    .await
    .map_err(map_database_error)?;

    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in_seconds: config.jwt_access_token_ttl_minutes * 60,
        user: MeResponse {
            id: user_id,
            email,
            full_name,
            commune_id,
            roles: roles
                .into_iter()
                .map(|role| role.code().to_string())
                .collect(),
            active: true,
            commune_access_expires_at,
            must_change_password,
        },
    })
}

fn create_access_token(
    config: &AppConfig,
    user_id: Uuid,
    email: &str,
    commune_id: Option<Uuid>,
    roles: &[Role],
) -> Result<String, ApiError> {
    let now = Utc::now();
    let expires_at = now + Duration::minutes(config.jwt_access_token_ttl_minutes);
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        commune_id,
        roles: roles.iter().map(|role| role.code().to_string()).collect(),
        iat: now.timestamp() as usize,
        exp: expires_at.timestamp() as usize,
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|error| {
        tracing::error!(%error, "jwt encode failed");
        ApiError::internal("Impossible de generer le token")
    })
}

fn decode_access_token(token: &str, secret: &str) -> Result<Claims, ApiError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|_| ApiError::unauthorized("Token invalide ou expire"))
}

fn generate_refresh_token() -> (Uuid, String, String) {
    let id = Uuid::new_v4();
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token = format!("{id}.{secret}");
    (id, token, secret)
}

fn parse_refresh_token(token: &str) -> Result<(Uuid, String), ApiError> {
    let (id, secret) = token
        .split_once('.')
        .ok_or_else(|| ApiError::unauthorized("Refresh token invalide"))?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::unauthorized("Refresh token invalide"))?;

    if secret.len() < 32 {
        return Err(ApiError::unauthorized("Refresh token invalide"));
    }

    Ok((id, secret.to_string()))
}

fn token_response_with_cookie(
    response: TokenResponse,
    config: &AppConfig,
) -> Result<Response, ApiError> {
    let cookie = refresh_cookie_header(&response.refresh_token, config);
    let mut http_response = Json(response).into_response();
    http_response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|error| {
            tracing::error!(%error, "refresh cookie header failed");
            ApiError::internal("Impossible de securiser la session")
        })?,
    );
    Ok(http_response)
}

fn refresh_token_from_cookie(headers: &HeaderMap) -> Result<String, ApiError> {
    let raw = headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Cookie de session manquant"))?;

    raw.split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(name, value)| {
            if name == REFRESH_COOKIE_NAME {
                Some(value.to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::unauthorized("Cookie de session invalide"))
}

fn refresh_cookie_header(refresh_token: &str, config: &AppConfig) -> String {
    let max_age = config.jwt_refresh_token_ttl_days * 24 * 60 * 60;
    let secure = secure_cookie_suffix(config);
    format!(
        "{REFRESH_COOKIE_NAME}={refresh_token}; Path=/api/v1/auth; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    )
}

fn clear_refresh_cookie(config: &AppConfig) -> String {
    let secure = secure_cookie_suffix(config);
    format!("{REFRESH_COOKIE_NAME}=; Path=/api/v1/auth; Max-Age=0; HttpOnly; SameSite=Lax{secure}")
}

fn secure_cookie_suffix(config: &AppConfig) -> &'static str {
    match config.app_env.as_str() {
        "development" | "test" => "",
        _ => "; Secure",
    }
}

async fn load_refresh_token_for_update(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_id: Uuid,
) -> Result<Option<RefreshTokenRecord>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, token_hash, expires_at, revoked_at
        FROM refresh_tokens
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(token_id)
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(row.map(|row| RefreshTokenRecord {
        id: row.get("id"),
        user_id: row.get("user_id"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
    }))
}

async fn revoke_refresh_token(
    pool: &PgPool,
    token_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = COALESCE(revoked_at, now())
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(token_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn revoke_refresh_token_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = COALESCE(revoked_at, now())
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(token_id)
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

pub async fn revoke_all_refresh_tokens(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = COALESCE(revoked_at, now())
        WHERE user_id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn revoke_all_refresh_tokens_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = COALESCE(revoked_at, now())
        WHERE user_id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_verifies_original_password() {
        let hash = hash_password("strong-password").expect("hash");

        assert!(verify_password("strong-password", &hash).expect("verify"));
        assert!(!verify_password("wrong-password", &hash).expect("verify"));
    }

    #[test]
    fn refresh_token_roundtrip_extracts_id_and_secret() {
        let (id, token, secret) = generate_refresh_token();
        let (parsed_id, parsed_secret) = parse_refresh_token(&token).expect("parse");

        assert_eq!(id, parsed_id);
        assert_eq!(secret, parsed_secret);
    }

    #[test]
    fn jwt_roundtrip_returns_claims() {
        let config = AppConfig {
            app_env: "test".to_string(),
            app_timezone: "Africa/Douala".to_string(),
            app_port: 8080,
            database_url: "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
                .to_string(),
            database_max_connections: 5,
            database_acquire_timeout_seconds: 3,
            database_idle_timeout_seconds: None,
            jwt_secret: "test_secret_for_phase1".to_string(),
            jwt_access_token_ttl_minutes: 15,
            jwt_refresh_token_ttl_days: 7,
            cors_allowed_origins: vec!["http://localhost:4200".to_string()],
            public_api_url: "http://localhost:8080".to_string(),
            public_web_url: "http://localhost:4200".to_string(),
            run_migrations_on_startup: false,
            rate_limit_enabled: false,
            rate_limit_window_seconds: 60,
            rate_limit_login_max: 10,
            rate_limit_public_max: 60,
            s3: None,
            smtp: None,
            whatsapp: None,
            daily_report_enabled: false,
            daily_report_hour_utc: 5,
        };
        let user_id = Uuid::new_v4();
        let token = create_access_token(
            &config,
            user_id,
            "admin@example.test",
            None,
            &[Role::SuperAdmin],
        )
        .expect("token");
        let claims = decode_access_token(&token, &config.jwt_secret).expect("claims");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.roles, vec!["SUPER_ADMIN"]);
    }
}
