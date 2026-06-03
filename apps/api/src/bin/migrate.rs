use apmtrack_api::config::AppConfig;
use apmtrack_api::database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env()?;
    let pool = database::create_pool(&config)?;
    database::run_migrations(&pool).await?;
    Ok(())
}
