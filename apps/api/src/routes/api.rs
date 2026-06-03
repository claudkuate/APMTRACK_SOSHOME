use axum::{extract::State, Json, Router};
use serde::Serialize;

use crate::modules;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(api_root))
        .nest("/auth", modules::auth::router())
        .merge(modules::users::router())
        .merge(modules::communes::router())
        .merge(modules::agents::router())
        .nest("/public", modules::agents::public_router())
}

async fn api_root(State(state): State<AppState>) -> Json<ApiRootResponse> {
    Json(ApiRootResponse {
        service: "apmtrack-api",
        version: env!("CARGO_PKG_VERSION"),
        environment: state.config.app_env,
        message: "APMTRACK API v1 backend foundation is available.",
    })
}

#[derive(Serialize)]
struct ApiRootResponse {
    service: &'static str,
    version: &'static str,
    environment: String,
    message: &'static str,
}
