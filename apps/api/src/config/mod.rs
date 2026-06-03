use std::env;

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub app_env: String,
    pub app_port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_access_token_ttl_minutes: i64,
    pub jwt_refresh_token_ttl_days: i64,
    pub cors_allowed_origins: Vec<String>,
    pub public_api_url: String,
    pub run_migrations_on_startup: bool,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),
    #[error("invalid APP_PORT value: {0}")]
    InvalidPort(String),
    #[error("invalid integer value for {name}: {value}")]
    InvalidInteger { name: &'static str, value: String },
    #[error("JWT_SECRET must be at least 16 characters outside local development")]
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
        let jwt_secret = required_env("JWT_SECRET")?;
        let jwt_access_token_ttl_minutes =
            optional_i64("JWT_ACCESS_TOKEN_TTL_MINUTES", 15)?;
        let jwt_refresh_token_ttl_days = optional_i64("JWT_REFRESH_TOKEN_TTL_DAYS", 7)?;

        if app_env != "development" && app_env != "test" && jwt_secret.len() < 16 {
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

        Ok(Self {
            app_env,
            app_port,
            database_url,
            jwt_secret,
            jwt_access_token_ttl_minutes,
            jwt_refresh_token_ttl_days,
            cors_allowed_origins,
            public_api_url,
            run_migrations_on_startup,
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

fn optional_bool(name: &'static str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}
