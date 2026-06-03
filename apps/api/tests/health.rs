use apmtrack_api::config::AppConfig;
use apmtrack_api::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
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
}

fn test_state() -> AppState {
    let config = AppConfig {
        app_env: "test".to_string(),
        app_port: 8080,
        database_url: "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
            .to_string(),
        jwt_secret: "test_secret_for_phase0".to_string(),
        cors_allowed_origins: vec!["http://localhost:4200".to_string()],
        public_api_url: "http://localhost:8080".to_string(),
    };

    AppState::try_new(config).expect("valid lazy database pool")
}
