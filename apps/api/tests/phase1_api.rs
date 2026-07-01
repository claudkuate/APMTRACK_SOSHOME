use apmtrack_api::config::AppConfig;
use apmtrack_api::database;
use apmtrack_api::modules::auth::{assign_roles, hash_password};
use apmtrack_api::modules::rbac::Role;
use apmtrack_api::state::AppState;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use sqlx::Row;
use tower::ServiceExt;
use uuid::Uuid;

static DB_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn phase1_auth_crud_audit_and_commune_isolation_flow() {
    if std::env::var("APMTRACK_RUN_DB_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping db integration test; set APMTRACK_RUN_DB_TESTS=1");
        return;
    }

    let _db_guard = DB_TEST_LOCK.lock().await;
    let state = test_state();
    database::run_migrations(&state.db)
        .await
        .expect("migrations");
    reset_database(&state).await;
    let super_admin = seed_test_super_admin(&state).await;
    let app = apmtrack_api::build_app(state.clone());

    let login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": super_admin.email,
            "password": super_admin.password
        }),
        None,
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    let access_token = login.body["access_token"].as_str().expect("access token");
    let refresh_token = login.body["refresh_token"].as_str().expect("refresh token");

    let me = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/auth/me",
        Some(access_token),
    )
    .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["email"], super_admin.email);

    let commune_a = create_commune(&app, access_token, "YDE1", "Yaounde 1").await;
    let commune_b = create_commune(&app, access_token, "DLA1", "Douala 1").await;
    let commune_a_id = commune_a["id"].as_str().expect("commune a id");
    let commune_b_id = commune_b["id"].as_str().expect("commune b id");

    let admin_commune = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "admin.yde1@example.test",
            "password": "admin-commune-password",
            "full_name": "Admin YDE1",
            "commune_id": commune_a_id,
            "roles": ["ADMIN_COMMUNE"]
        }),
        Some(access_token),
    )
    .await;
    assert_eq!(admin_commune.status, StatusCode::OK);

    let admin_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": "admin.yde1@example.test",
            "password": "admin-commune-password"
        }),
        None,
    )
    .await;
    assert_eq!(admin_login.status, StatusCode::OK);
    let commune_admin_token = admin_login.body["access_token"]
        .as_str()
        .expect("commune admin token");

    let visible_communes = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/communes",
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(visible_communes.status, StatusCode::OK);
    assert_eq!(visible_communes.body["total"], 1);
    assert_eq!(visible_communes.body["items"][0]["id"], commune_a_id);

    let forbidden_agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-DLA-001",
            "full_name": "Agent Douala",
            "commune_id": commune_b_id
        }),
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(forbidden_agent.status, StatusCode::FORBIDDEN);

    let agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-YDE-001",
            "full_name": "Agent Yaounde",
            "commune_id": commune_a_id
        }),
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(agent.status, StatusCode::OK);
    let agent_id = agent.body["id"].as_str().expect("agent id");

    let public_verify = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/public/agents/verify/APM-YDE-001",
        None,
    )
    .await;
    assert_eq!(public_verify.status, StatusCode::OK);
    assert_eq!(public_verify.body["active"], true);

    let suspended = request_empty(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/suspend"),
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(suspended.status, StatusCode::OK);
    assert_eq!(suspended.body["status"], "SUSPENDU");

    let audit_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM audit_logs WHERE action IN ('AGENT_CREATED', 'AGENT_SUSPENDED')",
    )
    .fetch_one(&state.db)
    .await
    .expect("audit count")
    .get("total");
    assert!(audit_count >= 2);

    let refreshed = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/refresh",
        json!({ "refresh_token": refresh_token }),
        None,
    )
    .await;
    assert_eq!(refreshed.status, StatusCode::OK);

    let old_refresh_rejected = request_json(
        app,
        Method::POST,
        "/api/v1/auth/refresh",
        json!({ "refresh_token": refresh_token }),
        None,
    )
    .await;
    assert_eq!(old_refresh_rejected.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mobile_agent_mvp_flow_is_scoped_to_authenticated_agent() {
    if std::env::var("APMTRACK_RUN_DB_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping db integration test; set APMTRACK_RUN_DB_TESTS=1");
        return;
    }

    let _db_guard = DB_TEST_LOCK.lock().await;
    let state = test_state();
    database::run_migrations(&state.db)
        .await
        .expect("migrations");
    reset_database(&state).await;
    let super_admin = seed_test_super_admin(&state).await;
    let app = apmtrack_api::build_app(state.clone());

    let admin_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": super_admin.email,
            "password": super_admin.password
        }),
        None,
    )
    .await;
    assert_eq!(admin_login.status, StatusCode::OK);
    let admin_token = admin_login.body["access_token"]
        .as_str()
        .expect("admin token");

    let commune = create_commune(&app, admin_token, "YDE1", "Yaounde 1").await;
    let commune_id = commune["id"].as_str().expect("commune id");

    let agent_user = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "agent.mobile@example.test",
            "password": "agent-mobile-password",
            "full_name": "Agent Mobile",
            "commune_id": commune_id,
            "roles": ["APM_AGENT"]
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(agent_user.status, StatusCode::OK);

    let agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-YDE1-MOB",
            "full_name": "Agent Mobile",
            "commune_id": commune_id
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(agent.status, StatusCode::OK);
    let agent_id = agent.body["id"].as_str().expect("agent id");

    let link_account = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/account"),
        json!({
            "email": "agent.mobile@example.com",
            "password": "agentpass123",
            "active": true
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(link_account.status, StatusCode::OK);

    let category = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/categories",
        json!({
            "commune_id": commune_id,
            "nom": "Circulation",
            "description": "Infractions circulation"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(category.status, StatusCode::OK);
    let category_id = category.body["id"].as_str().expect("category id");

    let intervention_type = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/types",
        json!({
            "commune_id": commune_id,
            "category_id": category_id,
            "nom": "Stationnement"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(intervention_type.status, StatusCode::OK);
    let type_id = intervention_type.body["id"].as_str().expect("type id");

    let intervention = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/interventions",
        json!({
            "commune_id": commune_id,
            "type_id": type_id,
            "nom": "Stationnement interdit",
            "sujet_paiement": true,
            "montant_fcfa": 10000,
            "delai_paiement_jours": 7,
            "reference_deliberation": "DEL-YDE1-TEST"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(
        intervention.status,
        StatusCode::OK,
        "create intervention response: {:?}",
        intervention.body
    );
    let intervention_id = intervention.body["id"].as_str().expect("intervention id");

    let patrouille = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/patrouilles",
        json!({
            "commune_id": commune_id,
            "nom": "Patrouille mobile test"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(patrouille.status, StatusCode::CREATED);
    let patrouille_id = patrouille.body["id"].as_str().expect("patrouille id");

    let assigned = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/patrouilles/{patrouille_id}/agents"),
        json!({
            "agent_id": agent_id,
            "role_patrouille": "CHEF"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(assigned.status, StatusCode::CREATED);

    let started = request_empty(
        app.clone(),
        Method::POST,
        &format!("/api/v1/patrouilles/{patrouille_id}/start"),
        Some(admin_token),
    )
    .await;
    assert_eq!(started.status, StatusCode::OK);

    let agent_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": "agent.mobile@example.test",
            "password": "agent-mobile-password"
        }),
        None,
    )
    .await;
    assert_eq!(agent_login.status, StatusCode::OK);
    let agent_token = agent_login.body["access_token"]
        .as_str()
        .expect("agent token");

    let mobile_me = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/mobile/me",
        Some(agent_token),
    )
    .await;
    assert_eq!(mobile_me.status, StatusCode::OK);
    assert_eq!(mobile_me.body["agent"]["id"], agent_id);

    let mobile_interventions = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/mobile/interventions",
        Some(agent_token),
    )
    .await;
    assert_eq!(mobile_interventions.status, StatusCode::OK);
    assert_eq!(
        mobile_interventions
            .body
            .as_array()
            .expect("interventions")
            .len(),
        1
    );

    let mobile_patrouille = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/mobile/patrouille-active",
        Some(agent_token),
    )
    .await;
    assert_eq!(mobile_patrouille.status, StatusCode::OK);
    assert_eq!(mobile_patrouille.body["patrouille"]["id"], patrouille_id);

    let position = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/patrouilles/{patrouille_id}/positions"),
        json!({
            "latitude": 3.8667,
            "longitude": 11.5167,
            "accuracy_m": 8.5
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(position.status, StatusCode::CREATED);

    let pv = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "vehicle_plate": "CE123AB",
            "location_description": "Carrefour test",
            "gps_latitude": 3.8667,
            "gps_longitude": 11.5167
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(pv.status, StatusCode::CREATED);
    assert_eq!(pv.body["amount_initial_fcfa"], 10000);

    let agent_pvs = request_empty(app.clone(), Method::GET, "/api/v1/pvs", Some(agent_token)).await;
    assert_eq!(agent_pvs.status, StatusCode::OK);
    assert_eq!(agent_pvs.body["total"], 1);

    let suspended = request_empty(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/suspend"),
        Some(admin_token),
    )
    .await;
    assert_eq!(suspended.status, StatusCode::OK);

    let rejected_pv = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "vehicle_plate": "CE999AB",
            "location_description": "Carrefour test"
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(rejected_pv.status, StatusCode::FORBIDDEN);
}

struct TestUser {
    email: String,
    password: String,
}

struct TestResponse {
    status: StatusCode,
    body: Value,
}

fn test_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack".into()
    });
    let config = AppConfig {
        app_env: "test".to_string(),
        app_port: 8080,
        database_url,
        database_max_connections: 5,
        database_acquire_timeout_seconds: 3,
        database_idle_timeout_seconds: None,
        jwt_secret: "test_secret_for_phase1_integration".to_string(),
        jwt_access_token_ttl_minutes: 15,
        jwt_refresh_token_ttl_days: 7,
        cors_allowed_origins: vec!["http://localhost:4200".to_string()],
        public_api_url: "http://localhost:8080".to_string(),
        public_web_url: "http://localhost:8080".to_string(),
        run_migrations_on_startup: false,
        rate_limit_enabled: false,
        rate_limit_window_seconds: 60,
        rate_limit_login_max: 10,
        rate_limit_public_max: 60,
        s3: None,
        smtp: None,
        whatsapp: None,
        daily_report_enabled: false,
        daily_report_hour_utc: 5,
    };

    AppState::try_new(config).expect("state")
}

async fn reset_database(state: &AppState) {
    sqlx::query(
        r#"
        TRUNCATE TABLE
            audit_logs,
            refresh_tokens,
            user_roles,
            agents,
            users,
            communes
        RESTART IDENTITY CASCADE
        "#,
    )
    .execute(&state.db)
    .await
    .expect("reset db");
}

async fn seed_test_super_admin(state: &AppState) -> TestUser {
    let user_id = Uuid::new_v4();
    let email = "root@example.test".to_string();
    let password = "super-admin-password".to_string();
    let password_hash = hash_password(&password).expect("password hash");

    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
        VALUES ($1, $2, $3, 'Root Admin', NULL, TRUE)
        "#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(password_hash)
    .execute(&state.db)
    .await
    .expect("insert super admin");

    assign_roles(&state.db, user_id, &[Role::SuperAdmin])
        .await
        .expect("assign role");

    TestUser { email, password }
}

async fn create_commune(app: &axum::Router, access_token: &str, code: &str, nom: &str) -> Value {
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/communes",
        json!({
            "code": code,
            "nom": nom,
            "region": "Centre",
            "departement": "Mfoundi"
        }),
        Some(access_token),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    response.body
}

async fn request_empty(
    app: axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
) -> TestResponse {
    request(app, method, uri, Body::empty(), token).await
}

async fn request_json(
    app: axum::Router,
    method: Method,
    uri: &str,
    body: Value,
    token: Option<&str>,
) -> TestResponse {
    request(app, method, uri, Body::from(body.to_string()), token).await
}

async fn request(
    app: axum::Router,
    method: Method,
    uri: &str,
    body: Body,
    token: Option<&str>,
) -> TestResponse {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");

    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    let response = app
        .oneshot(builder.body(body).expect("request body"))
        .await
        .expect("response");
    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let body = if body_bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body_bytes).expect("json response")
    };

    TestResponse { status, body }
}
