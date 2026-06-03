use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use apmtrack_api::config::AppConfig;
use apmtrack_api::state::AppState;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env()?;
    let port = config.app_port;
    let app_env = config.app_env.clone();
    let state = AppState::try_new(config)?;
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
