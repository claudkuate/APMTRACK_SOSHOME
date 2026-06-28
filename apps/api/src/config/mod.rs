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
    pub public_web_url: String,
    pub run_migrations_on_startup: bool,
    pub rate_limit_enabled: bool,
    pub rate_limit_window_seconds: u64,
    pub rate_limit_login_max: u32,
    pub rate_limit_public_max: u32,
    pub s3: Option<S3Config>,
    pub smtp: Option<SmtpConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
    pub daily_report_enabled: bool,
    pub daily_report_hour_utc: u32,
}

/// SMTP configuration for outbound email (daily mayor reports). Optional: when
/// the required variables are absent, email delivery is reported as disabled.
#[derive(Clone, Debug)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

/// WhatsApp Cloud API (Meta) configuration for outbound notifications (citizen
/// report tracking number). Optional: when the required variables are absent,
/// WhatsApp delivery is silently skipped.
#[derive(Clone, Debug)]
pub struct WhatsAppConfig {
    pub api_base_url: String,
    pub phone_number_id: String,
    pub access_token: String,
}

/// Object storage (MinIO/S3) configuration for PV photos. Optional: when the
/// required variables are absent, photo endpoints report storage as disabled.
#[derive(Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
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
        let public_web_url = env::var("PUBLIC_WEB_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim_end_matches('/').to_string())
            .unwrap_or_else(|| public_api_url.clone());
        let run_migrations_on_startup = optional_bool("RUN_MIGRATIONS_ON_STARTUP", false);
        let rate_limit_enabled = optional_bool("RATE_LIMIT_ENABLED", true);
        let rate_limit_window_seconds =
            optional_u64("RATE_LIMIT_WINDOW_SECONDS", 60, "RATE_LIMIT_WINDOW_SECONDS")?;
        let rate_limit_login_max =
            optional_u32("RATE_LIMIT_LOGIN_MAX", 10, "RATE_LIMIT_LOGIN_MAX")?;
        let rate_limit_public_max =
            optional_u32("RATE_LIMIT_PUBLIC_MAX", 60, "RATE_LIMIT_PUBLIC_MAX")?;

        let s3 = load_s3_config();
        let smtp = load_smtp_config();
        let whatsapp = load_whatsapp_config();
        let daily_report_enabled = optional_bool("DAILY_REPORT_ENABLED", false);
        let daily_report_hour_utc = optional_hour("DAILY_REPORT_HOUR_UTC", 5)?;

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
            public_web_url,
            run_migrations_on_startup,
            rate_limit_enabled,
            rate_limit_window_seconds,
            rate_limit_login_max,
            rate_limit_public_max,
            s3,
            smtp,
            whatsapp,
            daily_report_enabled,
            daily_report_hour_utc,
        })
    }
}

fn load_s3_config() -> Option<S3Config> {
    let endpoint = env::var("S3_ENDPOINT").ok().filter(|v| !v.trim().is_empty())?;
    let access_key = env::var("S3_ACCESS_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let secret_key = env::var("S3_SECRET_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    Some(S3Config {
        endpoint,
        region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "apmtrack-pv-photos".to_string()),
        access_key,
        secret_key,
    })
}

fn load_smtp_config() -> Option<SmtpConfig> {
    let host = env::var("SMTP_HOST").ok().filter(|v| !v.trim().is_empty())?;
    let from = env::var("SMTP_FROM").ok().filter(|v| !v.trim().is_empty())?;
    let port = env::var("SMTP_PORT")
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(587);
    let username = env::var("SMTP_USERNAME")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let password = env::var("SMTP_PASSWORD")
        .ok()
        .filter(|v| !v.trim().is_empty());
    Some(SmtpConfig {
        host,
        port,
        username,
        password,
        from,
    })
}

fn load_whatsapp_config() -> Option<WhatsAppConfig> {
    let phone_number_id = env::var("WHATSAPP_PHONE_NUMBER_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let access_token = env::var("WHATSAPP_ACCESS_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    Some(WhatsAppConfig {
        api_base_url: env::var("WHATSAPP_API_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "https://graph.facebook.com/v21.0".to_string()),
        phone_number_id,
        access_token,
    })
}

/// Heure UTC (0–23) d'envoi du rapport quotidien. Tolère 0 (minuit) et borne à 23.
fn optional_hour(name: &'static str, default: u32) -> Result<u32, ConfigError> {
    match env::var(name) {
        Ok(value) => {
            let parsed = value
                .trim()
                .parse::<u32>()
                .map_err(|_| ConfigError::InvalidInteger { name, value })?;
            Ok(parsed.min(23))
        }
        Err(_) => Ok(default),
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
