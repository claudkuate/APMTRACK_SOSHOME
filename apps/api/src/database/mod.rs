use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use crate::config::AppConfig;

pub fn create_pool(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let database_url = &config.database_url;
    let options = database_url.parse::<PgConnectOptions>()?;

    let mut pool_options = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(config.database_acquire_timeout_seconds));

    if let Some(seconds) = config.database_idle_timeout_seconds {
        pool_options = pool_options.idle_timeout(Duration::from_secs(seconds));
    }

    Ok(pool_options.connect_lazy_with(options))
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
