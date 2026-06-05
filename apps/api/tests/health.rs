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
        "/docs/openapi.json",
        "/api/v1/",
        "/api/v1/search",
        "/api/v1/auth/login",
        "/api/v1/auth/refresh",
        "/api/v1/auth/refresh-cookie",
        "/api/v1/auth/logout",
        "/api/v1/auth/me",
        "/api/v1/users",
        "/api/v1/users/{id}",
        "/api/v1/communes",
        "/api/v1/communes/{id}",
        "/api/v1/agents",
        "/api/v1/agents/import-csv",
        "/api/v1/agents/{id}",
        "/api/v1/agents/{id}/suspend",
        "/api/v1/agents/{id}/reactivate",
        "/api/v1/agents/{id}/retire",
        "/api/v1/public/agents/verify/{matricule}",
        "/api/v1/zones",
        "/api/v1/zones/{id}",
        "/api/v1/referentiel/categories",
        "/api/v1/referentiel/categories/{id}",
        "/api/v1/referentiel/types",
        "/api/v1/referentiel/types/{id}",
        "/api/v1/referentiel/interventions",
        "/api/v1/referentiel/interventions/{id}",
        "/api/v1/pvs",
        "/api/v1/pvs/{id}",
        "/api/v1/pvs/{id}/status",
        "/api/v1/pvs/{id}/qr",
        "/api/v1/pvs/{id}/pdf",
        "/api/v1/pvs/{id}/photos",
        "/api/v1/pvs/{id}/photos/{photo_id}",
        "/api/v1/public/pvs/{pv_number}",
        "/api/v1/payments",
        "/api/v1/payments/pending",
        "/api/v1/payments/{pv_id}/validate",
        "/api/v1/payments/{id}/receipt",
        "/api/v1/signalements",
        "/api/v1/signalements/{id}",
        "/api/v1/signalements/{id}/status",
        "/api/v1/public/signalements",
        "/api/v1/public/signalements/{numero_suivi}",
        "/api/v1/mobile/me",
        "/api/v1/mobile/interventions",
        "/api/v1/mobile/patrouille-active",
        "/api/v1/patrouilles",
        "/api/v1/patrouilles/{id}",
        "/api/v1/patrouilles/{id}/start",
        "/api/v1/patrouilles/{id}/end",
        "/api/v1/patrouilles/{id}/agents",
        "/api/v1/patrouilles/{id}/agents/{agent_id}",
        "/api/v1/patrouilles/{id}/positions",
        "/api/v1/patrouilles/{id}/track",
        "/api/v1/geo/overview",
        "/api/v1/geo/pvs",
        "/api/v1/geo/signalements",
        "/api/v1/geo/zones",
        "/api/v1/geo/communes",
        "/api/v1/geo/nearby",
        "/api/v1/dashboard/summary",
        "/api/v1/dashboard/pvs",
        "/api/v1/dashboard/payments",
        "/api/v1/dashboard/agents",
        "/api/v1/dashboard/signalements",
        "/api/v1/audit-logs",
        "/api/v1/audit-logs/{id}",
        "/api/v1/exports/pvs",
        "/api/v1/exports/payments",
        "/api/v1/exports/signalements",
        "/api/v1/exports/agents",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path {path}");
    }

    assert!(openapi["components"]["schemas"]["Pv"]["properties"]
        .as_object()
        .expect("pv properties")
        .contains_key("amount_initial_fcfa"));
    assert!(openapi["components"]["schemas"]["Pv"]["properties"]
        .as_object()
        .expect("pv properties")
        .contains_key("interventions"));
    assert!(openapi["components"]["schemas"]["Pv"]["properties"]
        .as_object()
        .expect("pv properties")
        .contains_key("subject_type"));
    assert!(openapi["components"]["schemas"]["Payment"]["properties"]
        .as_object()
        .expect("payment properties")
        .contains_key("amount_paid_fcfa"));
    assert!(
        openapi["components"]["schemas"]["Intervention"]["properties"]
            .as_object()
            .expect("intervention properties")
            .contains_key("montant_fcfa")
    );
    assert!(openapi["components"]["schemas"]["AuditLog"]["properties"]
        .as_object()
        .expect("audit properties")
        .contains_key("commune_id"));
    assert!(openapi["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .contains_key("MobileMe"));
    assert!(openapi["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .contains_key("MobilePatrouilleActive"));
    assert!(openapi["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .contains_key("RecordPositionRequest"));
}

fn test_state() -> AppState {
    let config = AppConfig {
        app_env: "test".to_string(),
        app_port: 8080,
        database_url: "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
            .to_string(),
        database_max_connections: 5,
        database_acquire_timeout_seconds: 3,
        database_idle_timeout_seconds: None,
        jwt_secret: "test_secret_for_phase0".to_string(),
        jwt_access_token_ttl_minutes: 15,
        jwt_refresh_token_ttl_days: 7,
        cors_allowed_origins: vec!["http://localhost:4200".to_string()],
        public_api_url: "http://localhost:8080".to_string(),
        run_migrations_on_startup: false,
        rate_limit_enabled: false,
        rate_limit_window_seconds: 60,
        rate_limit_login_max: 10,
        rate_limit_public_max: 60,
        s3: None,
    };

    AppState::try_new(config).expect("valid lazy database pool")
}
