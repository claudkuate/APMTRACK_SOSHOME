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

    // Lie le compte utilisateur existant (même email) à l'agent : la connexion
    // mobile plus bas doit résoudre l'agent via agents.user_id.
    let link_account = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/account"),
        json!({
            "email": "agent.mobile@example.test",
            "password": "agent-mobile-password",
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

    // Zone requise pour créer une patrouille.
    let zone = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/zones",
        json!({
            "commune_id": commune_id,
            "nom": "Quartier Centre",
            "type_zone": "QUARTIER"
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(
        zone.status,
        StatusCode::OK,
        "create zone response: {:?}",
        zone.body
    );
    let zone_id = zone.body["id"].as_str().expect("zone id");

    let patrouille = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/patrouilles",
        json!({
            "commune_id": commune_id,
            "zone_id": zone_id,
            "nom": "Patrouille mobile test",
            "agent_ids": [agent_id]
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(
        patrouille.status,
        StatusCode::CREATED,
        "create patrouille response: {:?}",
        patrouille.body
    );
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
    assert_eq!(
        assigned.status,
        StatusCode::CREATED,
        "assign agent response: {:?}",
        assigned.body
    );

    let started = request_empty(
        app.clone(),
        Method::POST,
        &format!("/api/v1/patrouilles/{patrouille_id}/start"),
        Some(admin_token),
    )
    .await;
    assert_eq!(
        started.status,
        StatusCode::OK,
        "start patrouille response: {:?}",
        started.body
    );

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
    assert_eq!(
        agent_login.status,
        StatusCode::OK,
        "agent login response: {:?}",
        agent_login.body
    );
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
    assert_eq!(
        mobile_me.status,
        StatusCode::OK,
        "mobile me response: {:?}",
        mobile_me.body
    );
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
    assert_eq!(
        mobile_patrouille.status,
        StatusCode::OK,
        "mobile patrouille response: {:?}",
        mobile_patrouille.body
    );
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
    assert_eq!(
        position.status,
        StatusCode::CREATED,
        "record position response: {:?}",
        position.body
    );

    let pv = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "verbalized_name": "Contrevenant Mobile",
            "verbalized_phone": "+237699000010",
            "vehicle_plate": "CE123AB",
            "location_description": "Carrefour test",
            "gps_latitude": 3.8667,
            "gps_longitude": 11.5167
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(
        pv.status,
        StatusCode::CREATED,
        "create pv response: {:?}",
        pv.body
    );
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
            "verbalized_name": "Contrevenant Suspendu",
            "verbalized_phone": "+237699000011",
            "vehicle_plate": "CE999AB",
            "location_description": "Carrefour test"
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(rejected_pv.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn receveur_payment_flow_lenient_totals_and_flat_penalty() {
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

    // Agent avec compte lié : requis pour créer des PV.
    let agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-YDE1-PAY",
            "full_name": "Agent Caisse",
            "commune_id": commune_id
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(agent.status, StatusCode::OK);
    let agent_id = agent.body["id"].as_str().expect("agent id");

    let linked = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/account"),
        json!({
            "email": "agent.caisse@example.test",
            "password": "agent-caisse-pass",
            "active": true
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(linked.status, StatusCode::OK);

    let agent_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": "agent.caisse@example.test",
            "password": "agent-caisse-pass"
        }),
        None,
    )
    .await;
    assert_eq!(agent_login.status, StatusCode::OK);
    let agent_token = agent_login.body["access_token"]
        .as_str()
        .expect("agent token");

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

    // Fiscalité communale : taux (10 %) ET forfait (4 000 FCFA) — le forfait doit primer.
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
            "taux_penalite_basis_points": 1000,
            "penalite_fcfa": 4000,
            "reference_deliberation": "DEL-YDE1-PAY"
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
    assert_eq!(intervention.body["penalite_fcfa"], 4000);
    let intervention_id = intervention.body["id"].as_str().expect("intervention id");

    let receveur = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "receveur.yde1@example.test",
            "password": "receveur-password",
            "full_name": "Receveur YDE1",
            "commune_id": commune_id,
            "roles": ["RECEVEUR"]
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(receveur.status, StatusCode::OK);

    let receveur_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({
            "email": "receveur.yde1@example.test",
            "password": "receveur-password"
        }),
        None,
    )
    .await;
    assert_eq!(receveur_login.status, StatusCode::OK);
    let receveur_token = receveur_login.body["access_token"]
        .as_str()
        .expect("receveur token");

    // PV 1 : on simule un PV hérité sans ligne payante (le calcul doit retomber
    // sur amount_initial_fcfa au lieu de rejeter la validation).
    let pv_legacy = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "verbalized_name": "Contrevenant Legacy",
            "verbalized_phone": "+237699000001",
            "vehicle_plate": "CE100AA",
            "location_description": "Carrefour test",
            "gps_latitude": 3.8667,
            "gps_longitude": 11.5167
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(
        pv_legacy.status,
        StatusCode::CREATED,
        "create legacy pv response: {:?}",
        pv_legacy.body
    );
    let pv_legacy_id = pv_legacy.body["id"].as_str().expect("pv legacy id");
    let pv_legacy_uuid = Uuid::parse_str(pv_legacy_id).expect("pv legacy uuid");
    sqlx::query("DELETE FROM pv_interventions WHERE pv_id = $1")
        .bind(pv_legacy_uuid)
        .execute(&state.db)
        .await
        .expect("strip pv interventions");

    // PV 2 : antidaté au-delà du délai de paiement → pénalité forfaitaire due.
    let pv_late = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "verbalized_name": "Contrevenant Retard",
            "verbalized_phone": "+237699000002",
            "vehicle_plate": "CE200BB",
            "location_description": "Carrefour test",
            "gps_latitude": 3.8667,
            "gps_longitude": 11.5167
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(pv_late.status, StatusCode::CREATED);
    let pv_late_id = pv_late.body["id"].as_str().expect("pv late id");
    let pv_late_uuid = Uuid::parse_str(pv_late_id).expect("pv late uuid");
    sqlx::query("UPDATE pvs SET created_at = now() - interval '10 days' WHERE id = $1")
        .bind(pv_late_uuid)
        .execute(&state.db)
        .await
        .expect("backdate pv");

    // La liste des PV à encaisser doit afficher exactement ce que la validation exigera.
    let pending = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/payments/pending",
        Some(receveur_token),
    )
    .await;
    assert_eq!(pending.status, StatusCode::OK);
    let items = pending.body["items"].as_array().expect("pending items");
    let legacy_row = items
        .iter()
        .find(|pv| pv["pv_id"].as_str() == Some(pv_legacy_id))
        .expect("legacy pv pending");
    assert_eq!(legacy_row["amount_penalty_fcfa"], 0);
    assert_eq!(legacy_row["amount_total_fcfa"], 10000);
    let late_row = items
        .iter()
        .find(|pv| pv["pv_id"].as_str() == Some(pv_late_id))
        .expect("late pv pending");
    assert_eq!(
        late_row["amount_penalty_fcfa"], 4000,
        "le forfait (4000) doit primer sur le taux (10% = 1000)"
    );
    assert_eq!(late_row["amount_total_fcfa"], 14000);

    // Validation du PV hérité au montant affiché — bloquée avant ce correctif
    // (409 « Ce PV n'a aucune infraction payante »).
    let paid_legacy = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_legacy_id}/validate"),
        json!({ "amount_paid_fcfa": 10000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(
        paid_legacy.status,
        StatusCode::CREATED,
        "validate legacy pv response: {:?}",
        paid_legacy.body
    );
    let receipt_number = paid_legacy.body["receipt_number"]
        .as_str()
        .expect("receipt number");
    assert!(receipt_number.starts_with("REC-YDE1-"));

    let legacy_status: String = sqlx::query("SELECT status FROM pvs WHERE id = $1")
        .bind(pv_legacy_uuid)
        .fetch_one(&state.db)
        .await
        .expect("pv status")
        .get("status");
    assert_eq!(legacy_status, "PAYE");
    let history_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM pv_status_history WHERE pv_id = $1 AND new_status = 'PAYE'",
    )
    .bind(pv_legacy_uuid)
    .fetch_one(&state.db)
    .await
    .expect("status history")
    .get("total");
    assert_eq!(history_count, 1);

    // Montant insuffisant → 400 avec le message métier détaillé (affiché tel quel au receveur).
    let insufficient = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_late_id}/validate"),
        json!({ "amount_paid_fcfa": 10000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(insufficient.status, StatusCode::BAD_REQUEST);
    assert!(insufficient.body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("Montant insuffisant"));

    let paid_late = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_late_id}/validate"),
        json!({ "amount_paid_fcfa": 14000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(
        paid_late.status,
        StatusCode::CREATED,
        "validate late pv response: {:?}",
        paid_late.body
    );
    assert_eq!(paid_late.body["amount_penalty_fcfa"], 4000);
    assert_eq!(paid_late.body["amount_total_fcfa"], 14000);

    // Un PV payé ne peut pas être encaissé deux fois.
    let replay = request_json(
        app,
        Method::POST,
        &format!("/api/v1/payments/{pv_late_id}/validate"),
        json!({ "amount_paid_fcfa": 14000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(replay.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn signalement_assignment_restricted_to_commune_or_global_supervisor() {
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

    let root_login = request_json(
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
    assert_eq!(root_login.status, StatusCode::OK);
    let root_token = root_login.body["access_token"].as_str().expect("token");

    let commune_a = create_commune(&app, root_token, "YDE1", "Yaounde 1").await;
    let commune_b = create_commune(&app, root_token, "DLA1", "Douala 1").await;
    let commune_a_id = commune_a["id"].as_str().expect("commune a id");
    let commune_b_id = commune_b["id"].as_str().expect("commune b id");

    let mut created_users = std::collections::HashMap::new();
    for (key, email, commune, roles, active) in [
        ("admin_a", "admin.a@example.test", Some(commune_a_id), json!(["ADMIN_COMMUNE"]), true),
        ("user_b", "admin.b@example.test", Some(commune_b_id), json!(["ADMIN_COMMUNE"]), true),
        ("global_sup", "superviseur@example.test", None, json!(["SUPERVISEUR"]), true),
        ("inactive_a", "inactif.a@example.test", Some(commune_a_id), json!(["APM_AGENT"]), false),
    ] {
        let created = request_json(
            app.clone(),
            Method::POST,
            "/api/v1/users",
            json!({
                "email": email,
                "password": "user-password-123",
                "full_name": key,
                "commune_id": commune,
                "roles": roles,
                "active": active
            }),
            Some(root_token),
        )
        .await;
        assert_eq!(created.status, StatusCode::OK, "create {key}: {:?}", created.body);
        created_users.insert(key, created.body["id"].as_str().expect("user id").to_string());
    }

    // Signalement en commune A (insertion directe, hors parcours public).
    let signalement_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO signalements (
            id, commune_id, signalement_number, type_incident,
            location_description, description, contact_anonyme, status
        ) VALUES ($1, $2, 'SIG-YDE1-2026-000001', 'Amende', 'Marché central', 'Test', TRUE, 'RECU')
        "#,
    )
    .bind(signalement_id)
    .bind(Uuid::parse_str(commune_a_id).expect("uuid"))
    .execute(&state.db)
    .await
    .expect("insert signalement");

    let admin_a_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "admin.a@example.test", "password": "user-password-123" }),
        None,
    )
    .await;
    assert_eq!(admin_a_login.status, StatusCode::OK);
    let admin_a_token = admin_a_login.body["access_token"].as_str().expect("token");

    // Affectations : même commune → OK ; autre commune / inactif / inconnu → 400 ;
    // superviseur global (NASLA / MINISTÈRE) → OK.
    let cases = [
        (created_users["admin_a"].clone(), StatusCode::OK),
        (created_users["user_b"].clone(), StatusCode::BAD_REQUEST),
        (created_users["global_sup"].clone(), StatusCode::OK),
        (created_users["inactive_a"].clone(), StatusCode::BAD_REQUEST),
        (Uuid::new_v4().to_string(), StatusCode::BAD_REQUEST),
    ];
    for (assigned_to, expected) in cases {
        let response = request_json(
            app.clone(),
            Method::PATCH,
            &format!("/api/v1/signalements/{signalement_id}/status"),
            json!({ "status": "EN_COURS", "assigned_to": assigned_to }),
            Some(admin_a_token),
        )
        .await;
        assert_eq!(
            response.status, expected,
            "assigned_to={assigned_to}: {:?}",
            response.body
        );
    }

    // Le sélecteur « Affecter à » : commune du signalement + superviseurs globaux.
    let assignables = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/users?commune_id={commune_a_id}&include_global=true&active=true"),
        Some(admin_a_token),
    )
    .await;
    assert_eq!(assignables.status, StatusCode::OK);
    let emails: Vec<&str> = assignables.body["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|item| item["email"].as_str())
        .collect();
    assert!(emails.contains(&"admin.a@example.test"), "emails: {emails:?}");
    assert!(emails.contains(&"superviseur@example.test"), "emails: {emails:?}");
    assert!(emails.contains(&"root@example.test"), "emails: {emails:?}");
    assert!(!emails.contains(&"admin.b@example.test"), "emails: {emails:?}");
    assert!(!emails.contains(&"inactif.a@example.test"), "emails: {emails:?}");

    // Un ADMIN_COMMUNE ne peut pas lister les utilisateurs d'une autre commune.
    let forbidden = request_empty(
        app,
        Method::GET,
        &format!("/api/v1/users?commune_id={commune_b_id}"),
        Some(admin_a_token),
    )
    .await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn fourriere_creation_auto_generates_linked_pv() {
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

    let root_login = request_json(
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
    assert_eq!(root_login.status, StatusCode::OK);
    let root_token = root_login.body["access_token"].as_str().expect("token");

    let commune = create_commune(&app, root_token, "YDE1", "Yaounde 1").await;
    let commune_id = commune["id"].as_str().expect("commune id");
    let commune_uuid = Uuid::parse_str(commune_id).expect("commune uuid");

    // Agent avec compte lié : porteur des PV de mise en fourrière.
    let agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-YDE1-FOUR",
            "full_name": "Agent Fourrière",
            "commune_id": commune_id
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(agent.status, StatusCode::OK);
    let agent_id = agent.body["id"].as_str().expect("agent id");
    let linked = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/account"),
        json!({
            "email": "agent.four@example.test",
            "password": "agent-four-pass",
            "active": true
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(linked.status, StatusCode::OK);
    let agent_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "agent.four@example.test", "password": "agent-four-pass" }),
        None,
    )
    .await;
    assert_eq!(agent_login.status, StatusCode::OK);
    let agent_token = agent_login.body["access_token"].as_str().expect("token");

    // Admin sans pv_id ni agent_id → l'agent est requis pour générer le PV.
    let missing_agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/fourrieres",
        json!({
            "commune_id": commune_id,
            "vehicle_plate": "CE100AA",
            "motif": "Stationnement gênant"
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(
        missing_agent.status,
        StatusCode::BAD_REQUEST,
        "missing agent: {:?}",
        missing_agent.body
    );

    // Admin avec agent_id → PV auto-généré et lié (référentiel provisionné lazy :
    // la commune a été créée après la migration).
    let created = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/fourrieres",
        json!({
            "commune_id": commune_id,
            "agent_id": agent_id,
            "vehicle_plate": "CE100AA",
            "motif": "Stationnement gênant"
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(
        created.status,
        StatusCode::CREATED,
        "create fourriere: {:?}",
        created.body
    );
    let pv_id = created.body["pv_id"].as_str().expect("pv_id lié");
    let pv_uuid = Uuid::parse_str(pv_id).expect("pv uuid");
    let pv_number = created.body["pv_number"].as_str().expect("pv_number");
    assert!(pv_number.starts_with("PV-YDE1-"), "pv_number: {pv_number}");

    let interv_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM interventions \
         WHERE commune_id = $1 AND system_code = 'FOURRIERE' AND deleted_at IS NULL",
    )
    .bind(commune_uuid)
    .fetch_one(&state.db)
    .await
    .expect("interventions count")
    .get("total");
    assert_eq!(interv_count, 1, "intervention système provisionnée");

    let pv_row = sqlx::query(
        "SELECT amount_initial_fcfa, status, agent_id FROM pvs WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(pv_uuid)
    .fetch_one(&state.db)
    .await
    .expect("pv row");
    assert_eq!(pv_row.get::<Option<i64>, _>("amount_initial_fcfa"), Some(25_000));
    assert_eq!(pv_row.get::<String, _>("status"), "EN_ATTENTE_PAIEMENT");
    assert_eq!(
        pv_row.get::<Uuid, _>("agent_id"),
        Uuid::parse_str(agent_id).expect("agent uuid")
    );

    let history_count: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM pv_status_history WHERE pv_id = $1")
            .bind(pv_uuid)
            .fetch_one(&state.db)
            .await
            .expect("history")
            .get("total");
    assert_eq!(history_count, 1);
    let lines_count: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM pv_interventions WHERE pv_id = $1")
            .bind(pv_uuid)
            .fetch_one(&state.db)
            .await
            .expect("pv interventions")
            .get("total");
    assert_eq!(lines_count, 1);

    // Même plaque encore en fourrière → doublon métier refusé.
    let duplicate = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/fourrieres",
        json!({
            "commune_id": commune_id,
            "agent_id": agent_id,
            "vehicle_plate": "CE100AA",
            "motif": "Stationnement gênant"
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(duplicate.status, StatusCode::CONFLICT);

    // Agent terrain sans agent_id → le PV est généré en son nom.
    let by_agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/fourrieres",
        json!({
            "vehicle_plate": "CE200BB",
            "motif": "Entrave à la circulation"
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(
        by_agent.status,
        StatusCode::CREATED,
        "fourriere agent: {:?}",
        by_agent.body
    );
    let agent_pv_uuid =
        Uuid::parse_str(by_agent.body["pv_id"].as_str().expect("pv_id")).expect("uuid");
    let agent_pv_agent: Uuid = sqlx::query("SELECT agent_id FROM pvs WHERE id = $1")
        .bind(agent_pv_uuid)
        .fetch_one(&state.db)
        .await
        .expect("agent pv")
        .get("agent_id");
    assert_eq!(agent_pv_agent, Uuid::parse_str(agent_id).expect("agent uuid"));

    // PV existant fourni → simple lien, aucun PV supplémentaire créé.
    let pv_count_before: i64 = sqlx::query("SELECT COUNT(*) AS total FROM pvs")
        .fetch_one(&state.db)
        .await
        .expect("count")
        .get("total");
    let linked_fourriere = request_json(
        app,
        Method::POST,
        "/api/v1/fourrieres",
        json!({
            "commune_id": commune_id,
            "pv_id": pv_id,
            "item_type": "MARCHANDISE",
            "designation": "Étal de marchandises",
            "motif": "Occupation illégale de la voie"
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(
        linked_fourriere.status,
        StatusCode::CREATED,
        "fourriere avec pv existant: {:?}",
        linked_fourriere.body
    );
    assert_eq!(linked_fourriere.body["pv_number"], pv_number);
    let pv_count_after: i64 = sqlx::query("SELECT COUNT(*) AS total FROM pvs")
        .fetch_one(&state.db)
        .await
        .expect("count")
        .get("total");
    assert_eq!(pv_count_before, pv_count_after, "aucun PV dupliqué");
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
        public_web_url: "http://localhost:4200".to_string(),
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
