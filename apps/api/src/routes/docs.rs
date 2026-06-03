use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;

const OPENAPI_JSON: &str = include_str!("../../../../packages/api-contracts/openapi.json");

pub async fn openapi_json() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI_JSON,
    )
}
