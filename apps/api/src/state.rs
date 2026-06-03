use sqlx::PgPool;

use crate::config::AppConfig;
use crate::database;
use crate::rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    pub fn try_new(config: AppConfig) -> Result<Self, sqlx::Error> {
        let db = database::create_pool(&config)?;
        let rate_limiter = RateLimiter::new(&config);

        Ok(Self {
            config,
            db,
            rate_limiter,
        })
    }
}
