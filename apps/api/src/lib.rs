pub mod config;
pub mod database;
pub mod errors;
pub mod extractors;
pub mod helpers;
pub mod modules;
pub mod pagination;
pub mod rate_limit;
pub mod routes;
pub mod sequences;
pub mod state;

use axum::http::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, Method,
};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::state::AppState;

pub fn build_app(state: AppState) -> Router {
    let cors = cors_layer(&state.config);

    Router::new()
        .route("/health", axum::routing::get(routes::health::health))
        .route("/health/db", axum::routing::get(routes::health::health_db))
        .route(
            "/docs/openapi.json",
            axum::routing::get(routes::docs::openapi_json),
        )
        .nest("/api/v1", routes::api::router())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

fn cors_layer(config: &AppConfig) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE]);

    if config
        .cors_allowed_origins
        .iter()
        .any(|origin| origin.trim() == "*")
    {
        return base.allow_origin(Any);
    }

    let origins = config
        .cors_allowed_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin.trim()).ok())
        .collect::<Vec<_>>();

    base.allow_origin(origins)
}
