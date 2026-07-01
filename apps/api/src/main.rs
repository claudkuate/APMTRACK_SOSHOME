use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use apmtrack_api::config::AppConfig;
use apmtrack_api::database;
use apmtrack_api::modules;
use apmtrack_api::state::AppState;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env()?;
    let command = std::env::args().nth(1);
    let port = config.app_port;
    let app_env = config.app_env.clone();
    let state = AppState::try_new(config)?;

    if state.config.run_migrations_on_startup
        || matches!(command.as_deref(), Some("seed-super-admin" | "seed-demo"))
    {
        database::run_migrations(&state.db).await?;
    }

    if command.as_deref() == Some("seed-super-admin") {
        modules::auth::seed_super_admin(&state.db).await?;
        return Ok(());
    }

    if command.as_deref() == Some("seed-demo") {
        modules::demo_seed::seed_demo(
            &state.db,
            &state.config.app_env,
            state.storage.as_deref(),
        )
        .await?;
        return Ok(());
    }

    // Démarre le scheduler des rapports quotidiens au Maire (no-op si désactivé).
    modules::reports::spawn_if_enabled(state.clone());

    let app = apmtrack_api::build_app(state);
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

    tracing::info!(
        service = "apmtrack-api",
        environment = %app_env,
        address = %addr,
        "starting api"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("apmtrack_api=info,tower_http=info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
