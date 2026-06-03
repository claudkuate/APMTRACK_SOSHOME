use std::env;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{map_database_error, ApiError};
use crate::modules::audit;
use crate::modules::rbac::{has_any_role, has_role, Role};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", axum::routing::post(login))
        .route("/refresh", axum::routing::post(refresh))
        .route("/logout", axum::routing::post(logout))
        .route("/me", axum::routing::get(me))
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub commune_id: Option<Uuid>,
    pub roles: Vec<Role>,
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
    email: String,
    password: String,
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
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| ApiError::unauthorized("Token invalide"))?;

        load_auth_user(&state.db, user_id).await
    }
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let email = normalize_email(&payload.email)?;
    let user = match load_login_user(&state.db, &email).await? {
        Some(user) => user,
        None => {
            audit::record(
                &state.db,
                None,
                "AUTH_LOGIN_FAILED",
                "users",
                None,
                None,
                Some(json!({ "email": email })),
            )
            .await;
            return Err(ApiError::unauthorized("Identifiants invalides"));
        }
    };

    if !user.active || !verify_password(&payload.password, &user.password_hash)? {
        audit::record(
            &state.db,
            Some(user.id),
            "AUTH_LOGIN_FAILED",
            "users",
            Some(user.id),
            None,
            Some(json!({ "email": user.email })),
        )
        .await;
        return Err(ApiError::unauthorized("Identifiants invalides"));
    }

    let roles = roles_for_user(&state.db, user.id).await?;
    let response = issue_tokens(
        &state.db,
        &state.config,
        user.id,
        user.email.clone(),
        user.full_name.clone(),
        user.commune_id,
        roles,
    )
    .await?;

    audit::record(
        &state.db,
        Some(user.id),
        "AUTH_LOGIN_SUCCEEDED",
        "users",
        Some(user.id),
        None,
        None,
    )
    .await;

    Ok(Json(response))
}

async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let (token_id, secret) = parse_refresh_token(&payload.refresh_token)?;
    let record = load_refresh_token(&state.db, token_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Refresh token invalide"))?;

    if record.revoked_at.is_some() || record.expires_at <= Utc::now() {
        return Err(ApiError::unauthorized("Refresh token expire ou revoque"));
    }

    if !verify_password(&secret, &record.token_hash)? {
        return Err(ApiError::unauthorized("Refresh token invalide"));
    }

    revoke_refresh_token(&state.db, token_id, record.user_id).await?;
    let auth_user = load_auth_user(&state.db, record.user_id).await?;
    let response = issue_tokens(
        &state.db,
        &state.config,
        auth_user.id,
        auth_user.email.clone(),
        auth_user.full_name.clone(),
        auth_user.commune_id,
        auth_user.roles.clone(),
    )
    .await?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "AUTH_TOKEN_REFRESHED",
        "refresh_tokens",
        Some(record.id),
        None,
        None,
    )
    .await;

    Ok(Json(response))
}

async fn logout(
    State(state): State<AppState>,
    auth_user: AuthUser,
    body: Option<Json<LogoutRequest>>,
) -> Result<StatusCode, ApiError> {
    if let Some(Json(payload)) = body {
        if let Some(refresh_token) = payload.refresh_token {
            let (token_id, _) = parse_refresh_token(&refresh_token)?;
            revoke_refresh_token(&state.db, token_id, auth_user.id).await?;
        } else {
            revoke_all_refresh_tokens(&state.db, auth_user.id).await?;
        }
    } else {
        revoke_all_refresh_tokens(&state.db, auth_user.id).await?;
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "AUTH_LOGOUT",
        "users",
        Some(auth_user.id),
        None,
        None,
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

async fn me(auth_user: AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        id: auth_user.id,
        email: auth_user.email,
        full_name: auth_user.full_name,
        commune_id: auth_user.commune_id,
        roles: auth_user
            .roles
            .into_iter()
            .map(|role| role.code().to_string())
            .collect(),
        active: true,
    })
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
    )
    .await;

    tracing::info!(%email, "super admin seed completed");
    Ok(())
}

pub async fn load_auth_user(pool: &PgPool, user_id: Uuid) -> Result<AuthUser, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, full_name, commune_id, active
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

    Ok(AuthUser {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        roles,
    })
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

pub async fn assign_roles(pool: &PgPool, user_id: Uuid, roles: &[Role]) -> Result<(), ApiError> {
    let mut transaction = pool.begin().await?;

    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
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
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
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
    if normalized.is_empty() || !normalized.contains('@') {
        return Err(ApiError::bad_request("Email invalide"));
    }
    Ok(normalized)
}

async fn load_login_user(pool: &PgPool, email: &str) -> Result<Option<LoginUser>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, password_hash, full_name, commune_id, active
        FROM users
        WHERE lower(email) = lower($1) AND deleted_at IS NULL
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| LoginUser {
        id: row.get("id"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        active: row.get("active"),
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
            roles: roles.into_iter().map(|role| role.code().to_string()).collect(),
            active: true,
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
    let secret = format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
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

async fn load_refresh_token(
    pool: &PgPool,
    token_id: Uuid,
) -> Result<Option<RefreshTokenRecord>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, token_hash, expires_at, revoked_at
        FROM refresh_tokens
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .fetch_optional(pool)
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

async fn revoke_all_refresh_tokens(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
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
            app_port: 8080,
            database_url: "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
                .to_string(),
            jwt_secret: "test_secret_for_phase1".to_string(),
            jwt_access_token_ttl_minutes: 15,
            jwt_refresh_token_ttl_days: 7,
            cors_allowed_origins: vec!["http://localhost:4200".to_string()],
            public_api_url: "http://localhost:8080".to_string(),
            run_migrations_on_startup: false,
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
