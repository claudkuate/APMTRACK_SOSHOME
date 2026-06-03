use apmtrack_api::config::AppConfig;
use apmtrack_api::state::AppState;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let app = apmtrack_api::build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn openapi_contract_is_served() {
    let app = apmtrack_api::build_app(test_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/docs/openapi.json")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("openapi response");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("openapi body");
    let openapi: Value = serde_json::from_slice(&body).expect("openapi json");
    let paths = openapi["paths"].as_object().expect("paths object");

    for path in [
        "/api/v1/auth/login",
        "/api/v1/auth/refresh",
        "/api/v1/auth/logout",
        "/api/v1/auth/me",
        "/api/v1/users",
        "/api/v1/communes",
        "/api/v1/agents",
        "/api/v1/public/agents/verify/{matricule}",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path {path}");
    }
}

fn test_state() -> AppState {
    let config = AppConfig {
        app_env: "test".to_string(),
        app_port: 8080,
        database_url: "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
            .to_string(),
        jwt_secret: "test_secret_for_phase0".to_string(),
        jwt_access_token_ttl_minutes: 15,
        jwt_refresh_token_ttl_days: 7,
        cors_allowed_origins: vec!["http://localhost:4200".to_string()],
        public_api_url: "http://localhost:8080".to_string(),
        run_migrations_on_startup: false,
    };

    AppState::try_new(config).expect("valid lazy database pool")
}
