use axum::{Json, Router, extract::State};
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
        .merge(modules::zones::router())
        .merge(modules::referentiel::router())
        .merge(modules::pvs::router())
        .merge(modules::payments::router())
        .merge(modules::signalements::router())
        .merge(modules::fourrieres::router())
        .merge(modules::patrouilles::router())
        .merge(modules::mobile::router())
        .merge(modules::dashboard::router())
        .merge(modules::geo::router())
        .merge(modules::geography::router())
        .merge(modules::audit_logs::router())
        .merge(modules::exports::router())
        .merge(modules::search::router())
        .nest(
            "/public",
            modules::agents::public_router()
                .merge(modules::pvs::public_router())
                .merge(modules::signalements::public_router())
                .merge(modules::communes::public_router())
                .merge(modules::geography::public_router()),
        )
}

async fn api_root(State(state): State<AppState>) -> Json<ApiRootResponse> {
    Json(ApiRootResponse {
        service: "apmtrack-api",
        version: env!("CARGO_PKG_VERSION"),
        environment: state.config.app_env,
        message: "APMTRACK API v1 — Backend complet : PV, paiements, signalements, patrouilles, dashboard, exports.",
    })
}

#[derive(Serialize)]
struct ApiRootResponse {
    service: &'static str,
    version: &'static str,
    environment: String,
    message: &'static str,
}
