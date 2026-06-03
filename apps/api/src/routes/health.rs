use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: &'static str,
    environment: String,
    version: &'static str,
}

#[derive(Serialize)]
pub struct DbHealthResponse {
    status: &'static str,
    service: &'static str,
    environment: String,
    database: &'static str,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "apmtrack-api",
        environment: state.config.app_env,
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn health_db(State(state): State<AppState>) -> (StatusCode, Json<DbHealthResponse>) {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(DbHealthResponse {
                status: "ok",
                service: "apmtrack-api",
                environment: state.config.app_env,
                database: "reachable",
            }),
        ),
        Err(error) => {
            tracing::warn!(%error, "database health check failed");

            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(DbHealthResponse {
                    status: "error",
                    service: "apmtrack-api",
                    environment: state.config.app_env,
                    database: "unreachable",
                }),
            )
        }
    }
}
