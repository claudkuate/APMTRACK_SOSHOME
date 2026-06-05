use std::sync::Arc;

use sqlx::PgPool;

use crate::config::AppConfig;
use crate::database;
use crate::rate_limit::RateLimiter;
use crate::storage::ObjectStorage;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
    pub rate_limiter: RateLimiter,
    pub storage: Option<Arc<ObjectStorage>>,
}

impl AppState {
    pub fn try_new(config: AppConfig) -> Result<Self, sqlx::Error> {
        let db = database::create_pool(&config)?;
        let rate_limiter = RateLimiter::new(&config);
        let storage = config.s3.clone().and_then(|s3| {
            match ObjectStorage::from_config(&s3) {
                Ok(storage) => Some(Arc::new(storage)),
                Err(error) => {
                    // Storage is optional: log and continue with photo endpoints disabled.
                    tracing::error!(%error, "object storage initialisation failed");
                    None
                }
            }
        });

        Ok(Self {
            config,
            db,
            rate_limiter,
            storage,
        })
    }
}
