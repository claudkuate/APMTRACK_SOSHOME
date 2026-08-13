use apmtrack_api::config::AppConfig;
use apmtrack_api::database;
use apmtrack_api::modules::auth::{assign_roles, hash_password};
use apmtrack_api::modules::demo_seed;
use apmtrack_api::modules::rbac::Role;
use apmtrack_api::state::AppState;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use chrono::Datelike;
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

    // Désynchronisation héritée : un reçu déjà en base (données semées/reprises)
    // occupe le numéro que le compteur RECEIPT — vierge, donc 000001 — va émettre.
    // La validation doit sauter les numéros pris ; avant ce correctif elle bouclait
    // sur un 409 (violation UNIQUE dont le rollback annulait aussi l'incrément).
    let pv_seeded = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/pvs",
        json!({
            "intervention_id": intervention_id,
            "verbalized_name": "Contrevenant Seme",
            "verbalized_phone": "+237699000003",
            "vehicle_plate": "CE300CC",
            "location_description": "Carrefour test",
            "gps_latitude": 3.8667,
            "gps_longitude": 11.5167
        }),
        Some(agent_token),
    )
    .await;
    assert_eq!(pv_seeded.status, StatusCode::CREATED);
    let pv_seeded_uuid =
        Uuid::parse_str(pv_seeded.body["id"].as_str().expect("pv seeded id")).expect("pv seeded");
    let receveur_uuid =
        Uuid::parse_str(receveur.body["id"].as_str().expect("receveur id")).expect("receveur");
    let year = chrono::Utc::now().year();
    let seeded_receipt = format!("REC-YDE1-{year}-000001");
    sqlx::query(
        r#"
        INSERT INTO payments (
            id, pv_id, commune_id, amount_due, amount_penalty, amount_total,
            amount_paid, amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
            amount_paid_fcfa, receiver_user_id, paid_at, status, receipt_number
        )
        VALUES ($1, $2, $3, 10000, 0, 10000, 10000, 10000, 0, 10000, 10000, $4, now(), 'PAYE', $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(pv_seeded_uuid)
    .bind(Uuid::parse_str(commune_id).expect("commune uuid"))
    .bind(receveur_uuid)
    .bind(&seeded_receipt)
    .execute(&state.db)
    .await
    .expect("seed legacy payment");
    sqlx::query("UPDATE pvs SET status = 'PAYE' WHERE id = $1")
        .bind(pv_seeded_uuid)
        .execute(&state.db)
        .await
        .expect("mark seeded pv paid");

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
    assert_eq!(
        receipt_number,
        format!("REC-YDE1-{year}-000002"),
        "le numero deja pris (000001) doit etre saute, pas reemis"
    );

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
    assert_eq!(
        paid_late.body["receipt_number"]
            .as_str()
            .expect("late receipt number"),
        format!("REC-YDE1-{year}-000003")
    );

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

/// Comptabilité : la pénalité entre dans les agrégats, un PV sans ligne payante en
/// accumule quand même une, et tableau de bord / caisse ne peuvent plus diverger.
///
/// Couvre le retour terrain « on a 160 000 en attente au lieu de 162 000 [...] sinon le
/// Receveur empochera les pénalités et le logiciel le lui permettra ».
#[tokio::test]
async fn accounting_totals_include_penalties_and_stay_consistent() {
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
        json!({ "email": super_admin.email, "password": super_admin.password }),
        None,
    )
    .await;
    let admin_token = admin_login.body["access_token"]
        .as_str()
        .expect("admin token");

    let commune = create_commune(&app, admin_token, "ACC1", "Commune Compta").await;
    let commune_id = commune["id"].as_str().expect("commune id");

    let agent = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/agents",
        json!({
            "matricule": "APM-ACC1-001",
            "full_name": "Agent Compta",
            "commune_id": commune_id
        }),
        Some(admin_token),
    )
    .await;
    let agent_id = agent.body["id"].as_str().expect("agent id");
    request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/agents/{agent_id}/account"),
        json!({
            "email": "agent.acc1@example.test",
            "password": "agent-acc1-pass",
            "active": true
        }),
        Some(admin_token),
    )
    .await;
    let agent_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "agent.acc1@example.test", "password": "agent-acc1-pass" }),
        None,
    )
    .await;
    let agent_token = agent_login.body["access_token"]
        .as_str()
        .expect("agent token");

    let category = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/categories",
        json!({ "commune_id": commune_id, "nom": "Voirie" }),
        Some(admin_token),
    )
    .await;
    let category_id = category.body["id"].as_str().expect("category id");
    let intervention_type = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/types",
        json!({ "commune_id": commune_id, "category_id": category_id, "nom": "Trottoir" }),
        Some(admin_token),
    )
    .await;
    let type_id = intervention_type.body["id"].as_str().expect("type id");

    // Taux 10 %, sans forfait : 20 000 FCFA -> 2 000 FCFA de pénalité une fois échu.
    let intervention = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/referentiel/interventions",
        json!({
            "commune_id": commune_id,
            "type_id": type_id,
            "nom": "Occupation trottoir",
            "sujet_paiement": true,
            "montant_fcfa": 20000,
            "delai_paiement_jours": 7,
            "taux_penalite_basis_points": 1000,
            "reference_deliberation": "DEL-ACC1"
        }),
        Some(admin_token),
    )
    .await;
    let intervention_id = intervention.body["id"].as_str().expect("intervention id");

    let make_pv = |name: &'static str, plate: &'static str| {
        let app = app.clone();
        let intervention_id = intervention_id.to_string();
        let agent_token = agent_token.to_string();
        async move {
            let pv = request_json(
                app,
                Method::POST,
                "/api/v1/pvs",
                json!({
                    "intervention_id": intervention_id,
                    "verbalized_name": name,
                    "verbalized_phone": "+237699000009",
                    "vehicle_plate": plate,
                    "location_description": "Rue test",
                    "gps_latitude": 3.8667,
                    "gps_longitude": 11.5167
                }),
                Some(&agent_token),
            )
            .await;
            assert_eq!(pv.status, StatusCode::CREATED, "create pv: {:?}", pv.body);
            Uuid::parse_str(pv.body["id"].as_str().expect("pv id")).expect("pv uuid")
        }
    };

    // PV A : à l'heure → aucune pénalité.
    let pv_on_time = make_pv("Contrevenant A", "ACC-001-AA").await;
    // PV B : échu ET privé de ses lignes payantes, comme les PV semés par `seed-demo`.
    // Avant correctif, le repli forçait la pénalité à 0 « à vie » : le receveur pouvait
    // encaisser 20 000 sur un PV échu depuis des semaines.
    let pv_late_legacy = make_pv("Contrevenant B", "ACC-002-BB").await;
    sqlx::query("DELETE FROM pv_interventions WHERE pv_id = $1")
        .bind(pv_late_legacy)
        .execute(&state.db)
        .await
        .expect("strip lines");
    sqlx::query("UPDATE pvs SET created_at = now() - interval '30 days' WHERE id = $1")
        .bind(pv_late_legacy)
        .execute(&state.db)
        .await
        .expect("backdate");

    let amounts = sqlx::query(
        "SELECT amount_base_fcfa, amount_penalty_fcfa, amount_total_fcfa, is_late \
         FROM pv_amounts_due WHERE pv_id = $1",
    )
    .bind(pv_late_legacy)
    .fetch_one(&state.db)
    .await
    .expect("view row");
    assert_eq!(amounts.get::<i64, _>("amount_base_fcfa"), 20000);
    assert_eq!(
        amounts.get::<i64, _>("amount_penalty_fcfa"),
        2000,
        "un PV sans ligne payante doit tout de meme accumuler la penalite du referentiel"
    );
    assert_eq!(amounts.get::<i64, _>("amount_total_fcfa"), 22000);
    assert!(amounts.get::<bool, _>("is_late"));

    // Le tableau de bord doit compter base + pénalité, pas la seule base.
    let summary = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/dashboard/summary?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(summary.status, StatusCode::OK);
    let payments_summary = &summary.body["payments"];
    assert_eq!(payments_summary["pending_base_fcfa"], 40000);
    assert_eq!(payments_summary["pending_penalty_fcfa"], 2000);
    assert_eq!(
        payments_summary["pending_total_fcfa"], 42000,
        "l'encours doit valoir base + penalite"
    );
    assert_eq!(
        payments_summary["pending_fcfa"], 42000,
        "le champ historique doit desormais porter le total"
    );
    assert_eq!(payments_summary["pending_count"], 2);
    assert_eq!(payments_summary["pending_late_count"], 1);

    // Tableau de bord et caisse doivent compter exactement la même chose.
    let pending = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/payments/pending?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    let late_in_list = pending.body["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter(|pv| pv["is_late"].as_bool().unwrap_or(false))
        .count();
    assert_eq!(
        late_in_list as i64,
        payments_summary["pending_late_count"].as_i64().expect("late"),
        "le nombre de PV en retard doit etre identique sur les deux ecrans"
    );

    let caisse = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/payments/summary?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(caisse.status, StatusCode::OK);
    assert_eq!(caisse.body["pending_total_fcfa"], 42000);
    assert_eq!(caisse.body["pending_penalty_fcfa"], 2000);
    assert_eq!(caisse.body["pending_late_count"], 1);

    // Le receveur ne peut plus encaisser en oubliant la pénalité, ni encaisser en trop.
    let receveur = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "receveur.acc1@example.test",
            "password": "receveur-acc1-pass",
            "full_name": "Receveur ACC1",
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
        json!({ "email": "receveur.acc1@example.test", "password": "receveur-acc1-pass" }),
        None,
    )
    .await;
    let receveur_token = receveur_login.body["access_token"]
        .as_str()
        .expect("receveur token");

    let without_penalty = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_late_legacy}/validate"),
        json!({ "amount_paid_fcfa": 20000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(
        without_penalty.status,
        StatusCode::BAD_REQUEST,
        "encaisser la base seule doit etre refuse sur un PV echu"
    );

    let overpaid = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_late_legacy}/validate"),
        json!({ "amount_paid_fcfa": 25000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(overpaid.status, StatusCode::BAD_REQUEST);
    assert!(overpaid.body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("excedentaire"));
    let payment_rows: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM payments WHERE pv_id = $1")
            .bind(pv_late_legacy)
            .fetch_one(&state.db)
            .await
            .expect("count payments")
            .get("total");
    assert_eq!(payment_rows, 0, "aucun paiement ne doit avoir ete enregistre");

    let exact = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/v1/payments/{pv_late_legacy}/validate"),
        json!({ "amount_paid_fcfa": 22000 }),
        Some(receveur_token),
    )
    .await;
    assert_eq!(
        exact.status,
        StatusCode::CREATED,
        "validate exact: {:?}",
        exact.body
    );
    assert_eq!(exact.body["amount_penalty_fcfa"], 2000);

    // Encaissé du tableau de bord et de la caisse : même montant, pénalité comprise.
    let summary_after = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/dashboard/summary?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    let caisse_after = request_empty(
        app.clone(),
        Method::GET,
        &format!("/api/v1/payments/summary?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(summary_after.body["payments"]["total_collected_fcfa"], 22000);
    assert_eq!(
        summary_after.body["payments"]["total_collected_penalty_fcfa"],
        2000
    );
    assert_eq!(
        caisse_after.body["collected_today_fcfa"],
        summary_after.body["payments"]["total_collected_fcfa"],
        "la recette de la caisse doit egaler l'encaisse du tableau de bord"
    );
    assert_eq!(caisse_after.body["receipts_today"], 1);

    // Un paiement annulé ne doit jamais gonfler la recette du jour.
    sqlx::query("UPDATE payments SET status = 'ANNULE' WHERE pv_id = $1")
        .bind(pv_late_legacy)
        .execute(&state.db)
        .await
        .expect("cancel payment");
    let caisse_cancelled = request_empty(
        app,
        Method::GET,
        &format!("/api/v1/payments/summary?commune_id={commune_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(caisse_cancelled.body["receipts_today"], 0);
    assert_eq!(caisse_cancelled.body["collected_today_fcfa"], 0);

    let _ = pv_on_time;
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
async fn commune_admin_cannot_modify_own_subscription() {
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

    // Le SUPER_ADMIN suspend l'abonnement de la commune.
    let suspended = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/communes/{commune_id}"),
        json!({ "subscription_status": "EXPIRED" }),
        Some(root_token),
    )
    .await;
    assert_eq!(suspended.status, StatusCode::OK, "{:?}", suspended.body);
    assert_eq!(suspended.body["subscription_status"], "EXPIRED");

    let admin = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "admin.yde1@example.test",
            "password": "admin-commune-password",
            "full_name": "Admin YDE1",
            "commune_id": commune_id,
            "roles": ["ADMIN_COMMUNE"]
        }),
        Some(root_token),
    )
    .await;
    assert_eq!(admin.status, StatusCode::OK);

    let admin_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "admin.yde1@example.test", "password": "admin-commune-password" }),
        None,
    )
    .await;
    assert_eq!(admin_login.status, StatusCode::OK);
    let admin_token = admin_login.body["access_token"].as_str().expect("token");

    // L'ADMIN_COMMUNE tente de se réabonner lui-même : la requête passe (il
    // peut éditer sa commune) mais les champs d'abonnement sont ignorés.
    let attempt = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/communes/{commune_id}"),
        json!({
            "nom": "Yaounde 1er",
            "subscription_status": "ACTIVE",
            "subscription_expires_at": "2099-01-01T00:00:00Z",
            "active": true
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(attempt.status, StatusCode::OK, "{:?}", attempt.body);
    assert_eq!(attempt.body["nom"], "Yaounde 1er");
    assert_eq!(attempt.body["subscription_status"], "EXPIRED");
    assert!(attempt.body["subscription_expires_at"].is_null());

    // Le SUPER_ADMIN, lui, peut réactiver l'abonnement.
    let reactivated = request_json(
        app,
        Method::PATCH,
        &format!("/api/v1/communes/{commune_id}"),
        json!({ "subscription_status": "ACTIVE" }),
        Some(root_token),
    )
    .await;
    assert_eq!(reactivated.status, StatusCode::OK);
    assert_eq!(reactivated.body["subscription_status"], "ACTIVE");
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

/// Import CSV des agents : dispatch national par code commune, cloisonnement par ligne,
/// tolérance au format Excel FR et préservation des colonnes absentes.
///
/// Couvre la demande « l'import au niveau de l'admin peut importer tous les agents une
/// seule fois en dispatchant chacun dans sa commune » et « l'import à partir d'un admin
/// de commune va juste avoir 2 colonnes ».
#[tokio::test]
async fn agents_csv_import_dispatches_by_commune_code_and_isolates_tenants() {
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
        json!({ "email": super_admin.email, "password": super_admin.password }),
        None,
    )
    .await;
    let admin_token = admin_login.body["access_token"]
        .as_str()
        .expect("admin token");

    let yde1 = create_commune(&app, admin_token, "YDE1", "Yaounde 1").await;
    let dla1 = create_commune(&app, admin_token, "DLA1", "Douala 1").await;
    let yde1_id = yde1["id"].as_str().expect("yde1 id").to_string();
    let dla1_id = dla1["id"].as_str().expect("dla1 id").to_string();

    // Fichier national tel que le produit Excel en configuration française :
    // BOM UTF-8, séparateur « ; », en-têtes du client. Sans les correctifs de parsing,
    // chaque ligne échouait.
    let national = "\u{feff}Matricule;Nom_Complet;Code_Commune_attache\n\
                    APM-NAT-001;NGONO Marie;YDE1\n\
                    APM-NAT-002;FOTSO Pierre;DLA1\n\
                    APM-NAT-003;MBALLA Jean;YDE1\n\
                    ;;\n";

    // Simulation d'abord : les compteurs sont exacts et rien n'est écrit.
    let dry = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv?dry_run=true",
        national.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(dry.status, StatusCode::OK, "dry run: {:?}", dry.body);
    assert_eq!(dry.body["created"], 3);
    assert_eq!(dry.body["skipped"], 0, "la ligne vide finale est ignoree");
    assert_eq!(dry.body["dry_run"], true);
    let dry_count: i64 = sqlx::query("SELECT COUNT(*) AS total FROM agents")
        .fetch_one(&state.db)
        .await
        .expect("count")
        .get("total");
    assert_eq!(dry_count, 0, "une simulation ne doit rien ecrire");

    let imported = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        national.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(imported.status, StatusCode::OK, "import: {:?}", imported.body);
    assert_eq!(imported.body["created"], 3);
    assert_eq!(
        imported.body["communes"].as_array().expect("communes").len(),
        2,
        "les agents doivent etre dispatches dans leurs deux communes"
    );
    let dispatched: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM agents WHERE commune_id = $1 AND deleted_at IS NULL",
    )
    .bind(Uuid::parse_str(&yde1_id).expect("uuid"))
    .fetch_one(&state.db)
    .await
    .expect("count")
    .get("total");
    assert_eq!(dispatched, 2);

    // Colonnes absentes du fichier : elles ne doivent JAMAIS écraser la base.
    sqlx::query(
        "UPDATE agents SET telephone = '+237690112233', email = 'marie@yde1.cm', \
         date_prise_fonction = DATE '2020-01-15' WHERE matricule = 'APM-NAT-001'",
    )
    .execute(&state.db)
    .await
    .expect("enrich agent");
    let reimport = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        national.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(reimport.body["updated"], 3);
    let preserved = sqlx::query(
        "SELECT telephone, email, date_prise_fonction FROM agents WHERE matricule = 'APM-NAT-001'",
    )
    .fetch_one(&state.db)
    .await
    .expect("agent");
    assert_eq!(
        preserved.get::<Option<String>, _>("telephone").as_deref(),
        Some("+237690112233"),
        "un fichier a 3 colonnes ne doit pas effacer le telephone"
    );
    assert_eq!(
        preserved.get::<Option<String>, _>("email").as_deref(),
        Some("marie@yde1.cm")
    );
    assert!(preserved
        .get::<Option<chrono::NaiveDate>, _>("date_prise_fonction")
        .is_some());

    // Un ADMIN_COMMUNE ne doit jamais écrire hors de sa commune, même si le fichier
    // contient d'autres codes : les lignes étrangères sont rejetées, pas écrites.
    let admin_commune = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "admin.yde1@example.test",
            "password": "admin-yde1-pass",
            "full_name": "Admin YDE1",
            "commune_id": yde1_id,
            "roles": ["ADMIN_COMMUNE"]
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(admin_commune.status, StatusCode::OK);
    let admin_commune_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "admin.yde1@example.test", "password": "admin-yde1-pass" }),
        None,
    )
    .await;
    let admin_commune_token = admin_commune_login.body["access_token"]
        .as_str()
        .expect("token");

    let dla1_before: i64 = sqlx::query("SELECT COUNT(*) AS total FROM agents WHERE commune_id = $1")
        .bind(Uuid::parse_str(&dla1_id).expect("uuid"))
        .fetch_one(&state.db)
        .await
        .expect("count")
        .get("total");
    let scoped = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        national.as_bytes(),
        Some(admin_commune_token),
    )
    .await;
    assert_eq!(scoped.status, StatusCode::OK);
    assert_eq!(scoped.body["skipped"], 1, "la ligne DLA1 doit etre rejetee");
    assert!(scoped.body["errors"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("perimetre"));
    let dla1_after: i64 = sqlx::query("SELECT COUNT(*) AS total FROM agents WHERE commune_id = $1")
        .bind(Uuid::parse_str(&dla1_id).expect("uuid"))
        .fetch_one(&state.db)
        .await
        .expect("count")
        .get("total");
    assert_eq!(
        dla1_before, dla1_after,
        "aucune ligne ne doit avoir ete ecrite dans l'autre commune"
    );

    // Import à 2 colonnes : la commune de l'appelant sert implicitement.
    let two_columns = "matricule,nom_complet\nAPM-LOC-001,ESSOMBA Paul\n";
    let local = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        two_columns.as_bytes(),
        Some(admin_commune_token),
    )
    .await;
    assert_eq!(local.status, StatusCode::OK, "2 colonnes: {:?}", local.body);
    assert_eq!(local.body["created"], 1);
    let local_commune: Uuid = sqlx::query("SELECT commune_id FROM agents WHERE matricule = $1")
        .bind("APM-LOC-001")
        .fetch_one(&state.db)
        .await
        .expect("agent")
        .get("commune_id");
    assert_eq!(local_commune.to_string(), yde1_id);

    // Code inconnu et doublon interne : erreurs de ligne, pas un 500 sur tout le fichier.
    let messy = "Matricule;Nom_Complet;Code_Commune_attache\n\
                 APM-ERR-1;Test Un;ZZZZ\n\
                 APM-ERR-2;Test Deux;YDE1\n\
                 APM-ERR-2;Doublon;DLA1\n";
    let messy_result = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        messy.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(messy_result.status, StatusCode::OK);
    assert_eq!(messy_result.body["created"], 1);
    assert_eq!(messy_result.body["skipped"], 2);

    // Transfert inter-communes : refusé par défaut, tracé quand il est demandé.
    let transfer = "Matricule;Nom_Complet;Code_Commune_attache\nAPM-NAT-001;NGONO Marie;DLA1\n";
    let refused = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        transfer.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(refused.body["skipped"], 1);
    assert!(refused.body["errors"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("allow_transfer"));

    let allowed = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv?allow_transfer=true",
        transfer.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(allowed.body["transferred"], 1);
    let audited: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM audit_logs WHERE action = 'AGENTS_IMPORT_TRANSFER'",
    )
    .fetch_one(&state.db)
    .await
    .expect("audit")
    .get("total");
    assert_eq!(audited, 1, "un transfert doit etre audite agent par agent");

    // `allow_transfer` est réservé au SUPER_ADMIN.
    let forbidden = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv?allow_transfer=true",
        two_columns.as_bytes(),
        Some(admin_commune_token),
    )
    .await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);

    // Matricule supprimé logiquement : restauré, pas dupliqué (l'index unique est partiel).
    sqlx::query("UPDATE agents SET deleted_at = now() WHERE matricule = 'APM-LOC-001'")
        .execute(&state.db)
        .await
        .expect("soft delete");
    let restored = request_csv(
        app.clone(),
        "/api/v1/agents/import-csv",
        two_columns.as_bytes(),
        Some(admin_commune_token),
    )
    .await;
    assert_eq!(restored.body["restored"], 1);
    let duplicates: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM agents WHERE matricule = 'APM-LOC-001'")
            .fetch_one(&state.db)
            .await
            .expect("count")
            .get("total");
    assert_eq!(duplicates, 1, "le matricule ne doit pas etre duplique");

    // En-tête obligatoire manquant : 400 explicite, citant les colonnes lues.
    let bad_header = request_csv(
        app,
        "/api/v1/agents/import-csv",
        "nom_complet;code_commune\nX;YDE1\n".as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(bad_header.status, StatusCode::BAD_REQUEST);
    assert!(bad_header.body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("matricule"));
}

/// Référentiel du découpage administratif : CRUD réservé au SUPER_ADMIN, suppressions
/// gardées, import idempotent, et surtout « enlever puis rajouter » réellement possible.
#[tokio::test]
async fn geography_referentiel_is_super_admin_only_and_reversible() {
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
        json!({ "email": super_admin.email, "password": super_admin.password }),
        None,
    )
    .await;
    let admin_token = admin_login.body["access_token"]
        .as_str()
        .expect("admin token");

    let commune = create_commune(&app, admin_token, "YDE1", "Yaounde 1").await;
    let commune_id = commune["id"].as_str().expect("commune id").to_string();

    // Un ADMIN_COMMUNE ne doit pas pouvoir toucher à une donnée nationale.
    request_json(
        app.clone(),
        Method::POST,
        "/api/v1/users",
        json!({
            "email": "admin.geo@example.test",
            "password": "admin-geo-pass",
            "full_name": "Admin Geo",
            "commune_id": commune_id,
            "roles": ["ADMIN_COMMUNE"]
        }),
        Some(admin_token),
    )
    .await;
    let commune_admin_login = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/auth/login",
        json!({ "email": "admin.geo@example.test", "password": "admin-geo-pass" }),
        None,
    )
    .await;
    let commune_admin_token = commune_admin_login.body["access_token"]
        .as_str()
        .expect("token");

    let forbidden = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/geography/regions",
        json!({ "nom": "Region Pirate", "code": "XX" }),
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(
        forbidden.status,
        StatusCode::FORBIDDEN,
        "les donnees nationales ne sont modifiables que par le SUPER_ADMIN"
    );

    // Lecture ouverte à tous les rôles (formulaire commune, cascade citoyenne).
    let readable = request_empty(
        app.clone(),
        Method::GET,
        "/api/v1/geography/regions",
        Some(commune_admin_token),
    )
    .await;
    assert_eq!(readable.status, StatusCode::OK);
    assert_eq!(readable.body["total"], 10, "les 10 regions sont semees");

    // Import du répertoire : la hiérarchie se construit et la commune est rattachée.
    let csv = "region;departement;departement_code;arrondissement;arrondissement_code;quartier;commune_code\n\
               Centre;Mfoundi;CE-MF;Yaounde Ier;CE-MF-01;Bastos;YDE1\n\
               Centre;Mfoundi;CE-MF;Yaounde Ier;CE-MF-01;Nlongkak;YDE1\n\
               Centre;Mfoundi;CE-MF;Yaounde IIe;CE-MF-02;;\n";
    let imported = request_csv(
        app.clone(),
        "/api/v1/geography/import-csv",
        csv.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(imported.status, StatusCode::OK, "import: {:?}", imported.body);
    assert_eq!(imported.body["arrondissements"]["created"], 2);
    assert_eq!(imported.body["quartiers"]["created"], 2);
    assert_eq!(imported.body["communes_linked"], 1);

    // Rejeu : strictement rien de nouveau.
    let replay = request_csv(
        app.clone(),
        "/api/v1/geography/import-csv",
        csv.as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(replay.body["arrondissements"]["created"], 0);
    assert_eq!(replay.body["quartiers"]["created"], 0);

    // Le trigger doit remonter departement/region depuis le seul arrondissement,
    // y compris sur un PATCH (piege classique de la liste « UPDATE OF »).
    let arrondissement_id: Uuid =
        sqlx::query("SELECT id FROM arrondissements WHERE code = 'CE-MF-02'")
            .fetch_one(&state.db)
            .await
            .expect("arrondissement")
            .get("id");
    sqlx::query("UPDATE communes SET arrondissement_id = NULL, region_id = NULL, departement_id = NULL WHERE id = $1")
        .bind(Uuid::parse_str(&commune_id).expect("uuid"))
        .execute(&state.db)
        .await
        .expect("reset links");
    let patched = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/v1/communes/{commune_id}"),
        json!({ "arrondissement_id": arrondissement_id }),
        Some(admin_token),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "patch: {:?}", patched.body);
    assert!(
        patched.body["departement_id"].is_string(),
        "le trigger doit remonter le departement depuis l'arrondissement"
    );
    assert!(patched.body["region_id"].is_string());

    // Une région inconnue est une erreur de ligne, jamais une 11e région.
    let typo = request_csv(
        app.clone(),
        "/api/v1/geography/import-csv",
        "region;departement;arrondissement\nCente;Mfoundi;Test\n".as_bytes(),
        Some(admin_token),
    )
    .await;
    assert_eq!(typo.body["skipped"], 1);
    let regions: i64 =
        sqlx::query("SELECT COUNT(*) AS total FROM regions WHERE deleted_at IS NULL")
            .fetch_one(&state.db)
            .await
            .expect("count")
            .get("total");
    assert_eq!(regions, 10, "une faute de frappe ne doit pas creer de region");

    // Suppressions gardées : enfants, puis communes référençantes.
    let region_id: Uuid = sqlx::query("SELECT id FROM regions WHERE nom = 'Centre'")
        .fetch_one(&state.db)
        .await
        .expect("region")
        .get("id");
    let blocked = request_empty(
        app.clone(),
        Method::DELETE,
        &format!("/api/v1/geography/regions/{region_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(blocked.status, StatusCode::CONFLICT);

    // « Enlever puis rajouter » : l'ancienne contrainte UNIQUE non partielle rendait la
    // recreation d'un code supprime definitivement impossible.
    let created = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/geography/arrondissements",
        json!({
            "nom": "Test Carte",
            "code": "TST-01",
            "departement_id": sqlx::query_scalar::<_, Uuid>("SELECT id FROM departements WHERE code = 'CE-MF'")
                .fetch_one(&state.db).await.expect("dept")
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "create: {:?}", created.body);
    let created_id = created.body["id"].as_str().expect("id").to_string();
    let deleted = request_empty(
        app.clone(),
        Method::DELETE,
        &format!("/api/v1/geography/arrondissements/{created_id}"),
        Some(admin_token),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    let recreated = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/geography/arrondissements",
        json!({
            "nom": "Test Carte",
            "code": "TST-01",
            "departement_id": sqlx::query_scalar::<_, Uuid>("SELECT id FROM departements WHERE code = 'CE-MF'")
                .fetch_one(&state.db).await.expect("dept")
        }),
        Some(admin_token),
    )
    .await;
    assert_eq!(
        recreated.status,
        StatusCode::OK,
        "un code supprime doit pouvoir etre recree: {:?}",
        recreated.body
    );

    // La cascade publique doit continuer de fonctionner (formulaire citoyen).
    let departement_id: Uuid = sqlx::query_scalar("SELECT id FROM departements WHERE code = 'CE-MF'")
        .fetch_one(&state.db)
        .await
        .expect("dept");
    let public = request_empty(
        app,
        Method::GET,
        &format!("/api/v1/public/geography/departements/{departement_id}/communes"),
        None,
    )
    .await;
    assert_eq!(
        public.status,
        StatusCode::OK,
        "la cascade citoyenne ne doit pas regresser"
    );
}

#[tokio::test]
async fn demo_seed_replay_survives_sequences_consumed_by_real_documents() {
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

    // Premier seed sur base vierge : la fourrière de démo et son PV lié existent.
    demo_seed::seed_demo(&state.db, "test", None)
        .await
        .expect("premier seed");

    let commune_id: Uuid = sqlx::query("SELECT id FROM communes WHERE code = 'YDE1'")
        .fetch_one(&state.db)
        .await
        .expect("commune YDE1")
        .get("id");
    let seeded = sqlx::query(
        "SELECT f.id, f.pv_id, p.pv_number FROM fourrieres f \
         JOIN pvs p ON p.id = f.pv_id WHERE f.commune_id = $1",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await
    .expect("fourriere semee");
    let seeded_fourriere_id: Uuid = seeded.get("id");
    let seeded_pv_id: Uuid = seeded.get("pv_id");
    let taken_pv_number: String = seeded.get("pv_number");

    // Simule une base héritée : la fourrière semée et son PV n'existent pas
    // encore (ancien seed sans fourrières), mais de vrais documents ont
    // consommé la séquence — dont le numéro que l'ancien seed codait en dur.
    sqlx::query("DELETE FROM fourrieres WHERE id = $1")
        .bind(seeded_fourriere_id)
        .execute(&state.db)
        .await
        .expect("suppression fourriere semee");
    sqlx::query("DELETE FROM pvs WHERE id = $1")
        .bind(seeded_pv_id)
        .execute(&state.db)
        .await
        .expect("suppression pv seme");

    let agent_id: Uuid =
        sqlx::query("SELECT id FROM agents WHERE commune_id = $1 ORDER BY matricule LIMIT 1")
            .bind(commune_id)
            .fetch_one(&state.db)
            .await
            .expect("agent seme")
            .get("id");
    let intervention_id: Uuid =
        sqlx::query("SELECT id FROM interventions WHERE commune_id = $1 ORDER BY nom LIMIT 1")
            .bind(commune_id)
            .fetch_one(&state.db)
            .await
            .expect("intervention semee")
            .get("id");
    let user_id: Uuid =
        sqlx::query("SELECT id FROM users WHERE commune_id = $1 ORDER BY email LIMIT 1")
            .bind(commune_id)
            .fetch_one(&state.db)
            .await
            .expect("user seme")
            .get("id");
    let year = chrono::Utc::now().year();
    let next_pv_seq: i64 = sqlx::query(
        "SELECT next_value FROM document_sequences \
         WHERE commune_id = $1 AND kind = 'PV' AND year = $2",
    )
    .bind(commune_id)
    .bind(year)
    .fetch_one(&state.db)
    .await
    .expect("compteur PV")
    .get("next_value");
    let next_four_seq: i64 = sqlx::query(
        "SELECT next_value FROM document_sequences \
         WHERE commune_id = $1 AND kind = 'FOURRIERE' AND year = $2",
    )
    .bind(commune_id)
    .bind(year)
    .fetch_one(&state.db)
    .await
    .expect("compteur FOURRIERE")
    .get("next_value");

    // Deux vrais PV : l'un reprend le numéro que le seed attribuait à la
    // fourrière (l'insert échouait en 23505 avant ce correctif), l'autre
    // occupe le prochain numéro du compteur (il doit être sauté).
    let real_pv_id = Uuid::new_v4();
    let counter_pv_number = format!("PV-YDE1-{year}-{next_pv_seq:06}");
    for (id, number) in [
        (real_pv_id, taken_pv_number.clone()),
        (Uuid::new_v4(), counter_pv_number.clone()),
    ] {
        sqlx::query(
            r#"
            INSERT INTO pvs (id, commune_id, agent_id, pv_number, intervention_id,
                             status, created_by)
            VALUES ($1, $2, $3, $4, $5, 'EN_ATTENTE_PAIEMENT', $6)
            "#,
        )
        .bind(id)
        .bind(commune_id)
        .bind(agent_id)
        .bind(number)
        .bind(intervention_id)
        .bind(user_id)
        .execute(&state.db)
        .await
        .expect("vrai pv occupant un numero");
    }
    // Une vraie fourrière occupe le prochain numéro FOURRIERE du compteur.
    let counter_four_number = format!("FOUR-YDE1-{year}-{next_four_seq:06}");
    sqlx::query(
        r#"
        INSERT INTO fourrieres (id, commune_id, pv_id, fourriere_number,
                                vehicle_plate, motif, status, daily_fee_fcfa)
        VALUES ($1, $2, $3, $4, 'CE999ZZ', 'Stationnement gênant', 'EN_FOURRIERE', 2000)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(commune_id)
    .bind(real_pv_id)
    .bind(&counter_four_number)
    .execute(&state.db)
    .await
    .expect("vraie fourriere occupant un numero");

    // Rejeu : le seed doit sauter les numéros pris au lieu d'échouer en 23505.
    demo_seed::seed_demo(&state.db, "test", None)
        .await
        .expect("rejeu du seed sur base avec documents reels");

    let replayed = sqlx::query(
        "SELECT f.fourriere_number, p.pv_number FROM fourrieres f \
         JOIN pvs p ON p.id = f.pv_id \
         WHERE f.commune_id = $1 AND f.vehicle_plate = 'YDE1-1234'",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await
    .expect("fourriere resemee");
    let new_pv_number: String = replayed.get("pv_number");
    let new_four_number: String = replayed.get("fourriere_number");
    assert_eq!(
        new_pv_number,
        format!("PV-YDE1-{year}-{:06}", next_pv_seq + 1),
        "les numeros de PV pris ({taken_pv_number}, {counter_pv_number}) doivent etre sautes"
    );
    assert_eq!(
        new_four_number,
        format!("FOUR-YDE1-{year}-{:06}", next_four_seq + 1),
        "le numero de fourriere pris ({counter_four_number}) doit etre saute"
    );

    // Nouveau rejeu : les lignes existent, leurs numéros sont conservés.
    demo_seed::seed_demo(&state.db, "test", None)
        .await
        .expect("second rejeu du seed");
    let stable = sqlx::query(
        "SELECT f.fourriere_number, p.pv_number FROM fourrieres f \
         JOIN pvs p ON p.id = f.pv_id \
         WHERE f.commune_id = $1 AND f.vehicle_plate = 'YDE1-1234'",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await
    .expect("fourriere stable au rejeu");
    assert_eq!(stable.get::<String, _>("pv_number"), new_pv_number);
    assert_eq!(stable.get::<String, _>("fourriere_number"), new_four_number);
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
        app_timezone: "Africa/Douala".to_string(),
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

    // Le découpage national n'est pas une donnée de tenant : il survit au TRUNCATE
    // ci-dessus. Sans ce nettoyage, les arrondissements/quartiers créés par un test
    // faussent les compteurs `created` du test suivant (import idempotent).
    // Les régions et départements sont semés par les migrations : on les conserve, en
    // remettant seulement à zéro les codes que l'import a pu renseigner.
    sqlx::query("DELETE FROM quartiers")
        .execute(&state.db)
        .await
        .expect("reset quartiers");
    sqlx::query("DELETE FROM arrondissements")
        .execute(&state.db)
        .await
        .expect("reset arrondissements");
    sqlx::query("UPDATE departements SET code = NULL WHERE code IS NOT NULL")
        .execute(&state.db)
        .await
        .expect("reset departement codes");
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

/// POST d'un corps CSV brut (l'import n'est pas du multipart mais du `text/csv`).
/// Prend `&[u8]` pour pouvoir injecter un BOM ou un encodage non-UTF-8.
async fn request_csv(
    app: axum::Router,
    uri: &str,
    body: &[u8],
    token: Option<&str>,
) -> TestResponse {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "text/csv; charset=utf-8");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(builder.body(Body::from(body.to_vec())).expect("csv body"))
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
