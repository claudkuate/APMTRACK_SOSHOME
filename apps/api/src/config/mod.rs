use std::env;

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub app_env: String,
    pub app_port: u16,
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_acquire_timeout_seconds: u64,
    pub database_idle_timeout_seconds: Option<u64>,
    pub jwt_secret: String,
    pub jwt_access_token_ttl_minutes: i64,
    pub jwt_refresh_token_ttl_days: i64,
    pub cors_allowed_origins: Vec<String>,
    pub public_api_url: String,
    pub run_migrations_on_startup: bool,
    pub rate_limit_enabled: bool,
    pub rate_limit_window_seconds: u64,
    pub rate_limit_login_max: u32,
    pub rate_limit_public_max: u32,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),
    #[error("invalid APP_PORT value: {0}")]
    InvalidPort(String),
    #[error("invalid integer value for {name}: {value}")]
    InvalidInteger { name: &'static str, value: String },
    #[error("{name} must be greater than zero")]
    NonPositiveInteger { name: &'static str },
    #[error("JWT_SECRET must be at least 32 characters")]
    WeakJwtSecret,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let app_port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|err| ConfigError::InvalidPort(err.to_string()))?;
        let database_url = required_env("DATABASE_URL")?;
        let database_max_connections =
            optional_u32("DATABASE_MAX_CONNECTIONS", 5, "DATABASE_MAX_CONNECTIONS")?;
        let database_acquire_timeout_seconds = optional_u64(
            "DATABASE_ACQUIRE_TIMEOUT_SECONDS",
            3,
            "DATABASE_ACQUIRE_TIMEOUT_SECONDS",
        )?;
        let database_idle_timeout_seconds = optional_u64_opt(
            "DATABASE_IDLE_TIMEOUT_SECONDS",
            "DATABASE_IDLE_TIMEOUT_SECONDS",
        )?;
        let jwt_secret = required_env("JWT_SECRET")?;
        let jwt_access_token_ttl_minutes = optional_i64("JWT_ACCESS_TOKEN_TTL_MINUTES", 15)?;
        let jwt_refresh_token_ttl_days = optional_i64("JWT_REFRESH_TOKEN_TTL_DAYS", 7)?;

        if jwt_secret.len() < 32 {
            return Err(ConfigError::WeakJwtSecret);
        }

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:4200".to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        let public_api_url =
            env::var("PUBLIC_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let run_migrations_on_startup = optional_bool("RUN_MIGRATIONS_ON_STARTUP", false);
        let rate_limit_enabled = optional_bool("RATE_LIMIT_ENABLED", true);
        let rate_limit_window_seconds =
            optional_u64("RATE_LIMIT_WINDOW_SECONDS", 60, "RATE_LIMIT_WINDOW_SECONDS")?;
        let rate_limit_login_max =
            optional_u32("RATE_LIMIT_LOGIN_MAX", 10, "RATE_LIMIT_LOGIN_MAX")?;
        let rate_limit_public_max =
            optional_u32("RATE_LIMIT_PUBLIC_MAX", 60, "RATE_LIMIT_PUBLIC_MAX")?;

        Ok(Self {
            app_env,
            app_port,
            database_url,
            database_max_connections,
            database_acquire_timeout_seconds,
            database_idle_timeout_seconds,
            jwt_secret,
            jwt_access_token_ttl_minutes,
            jwt_refresh_token_ttl_days,
            cors_allowed_origins,
            public_api_url,
            run_migrations_on_startup,
            rate_limit_enabled,
            rate_limit_window_seconds,
            rate_limit_login_max,
            rate_limit_public_max,
        })
    }
}

fn required_env(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::MissingEnv(name))
}

fn optional_i64(name: &'static str, default: i64) -> Result<i64, ConfigError> {
    match env::var(name) {
        Ok(value) => value
            .parse::<i64>()
            .map_err(|_| ConfigError::InvalidInteger { name, value }),
        Err(_) => Ok(default),
    }
}

fn optional_u32(
    env_name: &'static str,
    default: u32,
    error_name: &'static str,
) -> Result<u32, ConfigError> {
    let value = match env::var(env_name) {
        Ok(value) => value
            .parse::<u32>()
            .map_err(|_| ConfigError::InvalidInteger {
                name: error_name,
                value,
            })?,
        Err(_) => default,
    };
    if value == 0 {
        return Err(ConfigError::NonPositiveInteger { name: error_name });
    }
    Ok(value)
}

fn optional_u64(
    env_name: &'static str,
    default: u64,
    error_name: &'static str,
) -> Result<u64, ConfigError> {
    let value = match env::var(env_name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| ConfigError::InvalidInteger {
                name: error_name,
                value,
            })?,
        Err(_) => default,
    };
    if value == 0 {
        return Err(ConfigError::NonPositiveInteger { name: error_name });
    }
    Ok(value)
}

fn optional_u64_opt(
    env_name: &'static str,
    error_name: &'static str,
) -> Result<Option<u64>, ConfigError> {
    match env::var(env_name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| ConfigError::InvalidInteger {
                    name: error_name,
                    value,
                })?;
            if parsed == 0 {
                return Err(ConfigError::NonPositiveInteger { name: error_name });
            }
            Ok(Some(parsed))
        }
        Err(_) => Ok(None),
    }
}

fn optional_bool(name: &'static str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}
