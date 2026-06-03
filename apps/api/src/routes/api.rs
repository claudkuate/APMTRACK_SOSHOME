use axum::{extract::State, Json, Router};
use serde::Serialize;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", axum::routing::get(api_root))
}

async fn api_root(State(state): State<AppState>) -> Json<ApiRootResponse> {
    Json(ApiRootResponse {
        service: "apmtrack-api",
        version: env!("CARGO_PKG_VERSION"),
        environment: state.config.app_env,
        message: "APMTRACK API v1 reserved for Phase 1 business modules.",
    })
}

#[derive(Serialize)]
struct ApiRootResponse {
    service: &'static str,
    version: &'static str,
    environment: String,
    message: &'static str,
}
