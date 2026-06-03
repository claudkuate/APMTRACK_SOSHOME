use sqlx::PgPool;

use crate::config::AppConfig;
use crate::database;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
}

impl AppState {
    pub fn try_new(config: AppConfig) -> Result<Self, sqlx::Error> {
        let db = database::create_pool(&config.database_url)?;

        Ok(Self { config, db })
    }
}
