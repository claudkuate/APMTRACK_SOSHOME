use axum::extract::{Query, State};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::errors::ApiError;
use crate::helpers::resolve_commune_filter;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/dashboard/summary", axum::routing::get(summary))
        .route("/dashboard/pvs", axum::routing::get(pv_stats))
        .route("/dashboard/payments", axum::routing::get(payment_stats))
        .route("/dashboard/agents", axum::routing::get(agent_stats))
        .route(
            "/dashboard/signalements",
            axum::routing::get(signalement_stats),
        )
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    commune_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub pvs: PvSummary,
    pub payments: PaymentSummary,
    pub agents: AgentSummary,
    pub signalements: SignalementSummary,
    pub patrouilles: PatrouillesSummary,
    pub commune_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct PvSummary {
    pub total: i64,
    pub en_attente: i64,
    pub payes: i64,
    pub en_retard: i64,
    pub annules: i64,
    pub non_payants: i64,
}

#[derive(Debug, Serialize)]
pub struct PaymentSummary {
    pub total_payments: i64,
    pub total_collected_fcfa: f64,
    pub pending_fcfa: f64,
}

#[derive(Debug, Serialize)]
pub struct AgentSummary {
    pub total: i64,
    pub actifs: i64,
    pub suspendus: i64,
    pub retraites: i64,
}

#[derive(Debug, Serialize)]
pub struct SignalementSummary {
    pub total: i64,
    pub recu: i64,
    pub en_cours: i64,
    pub traites: i64,
    pub rejetes: i64,
}

#[derive(Debug, Serialize)]
pub struct PatrouillesSummary {
    pub actives: i64,
    pub planifiees: i64,
    pub cloturees: i64,
}

#[derive(Debug, Serialize)]
pub struct PvStatsResponse {
    pub by_status: Vec<StatusCount>,
    pub total: i64,
    pub commune_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PaymentStatsResponse {
    pub total_payments: i64,
    pub total_collected_fcfa: f64,
    pub total_penalties_fcfa: f64,
    pub pending_count: i64,
    pub pending_fcfa: f64,
    pub commune_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AgentStatsResponse {
    pub total: i64,
    pub actifs: i64,
    pub suspendus: i64,
    pub top_agents: Vec<AgentActivity>,
    pub commune_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AgentActivity {
    pub agent_id: Uuid,
    pub agent_nom: String,
    pub matricule: String,
    pub pv_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SignalementStatsResponse {
    pub total: i64,
    pub by_status: Vec<StatusCount>,
    pub commune_id: Option<Uuid>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn summary(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<SummaryResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let db = &state.db;

    // PV stats
    let pv_rows = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE deleted_at IS NULL) AS total,
            COUNT(*) FILTER (WHERE status = 'EN_ATTENTE_PAIEMENT' AND deleted_at IS NULL) AS en_attente,
            COUNT(*) FILTER (WHERE status = 'PAYE' AND deleted_at IS NULL) AS payes,
            COUNT(*) FILTER (WHERE status = 'EN_RETARD' AND deleted_at IS NULL) AS en_retard,
            COUNT(*) FILTER (WHERE status = 'ANNULE' AND deleted_at IS NULL) AS annules,
            COUNT(*) FILTER (WHERE status = 'NON_PAYANT' AND deleted_at IS NULL) AS non_payants
        FROM pvs
        WHERE ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    let pvs = PvSummary {
        total: pv_rows.get("total"),
        en_attente: pv_rows.get("en_attente"),
        payes: pv_rows.get("payes"),
        en_retard: pv_rows.get("en_retard"),
        annules: pv_rows.get("annules"),
        non_payants: pv_rows.get("non_payants"),
    };

    // Payment stats
    let pay_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_payments,
            COALESCE(SUM(amount_paid), 0) AS total_collected,
            COALESCE(SUM(p.amount_total) FILTER (WHERE pv.status IN ('EN_ATTENTE_PAIEMENT','EN_RETARD')), 0) AS pending
        FROM payments p
        JOIN pvs pv ON p.pv_id = pv.id
        WHERE ($1::uuid IS NULL OR p.commune_id = $1)
          AND p.status = 'PAYE'
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    // Pending amount from pvs directly
    let pending_row = sqlx::query(
        r#"
        SELECT COALESCE(SUM(amount_initial), 0) AS pending_fcfa
        FROM pvs
        WHERE status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
          AND deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    let payments = PaymentSummary {
        total_payments: pay_row.get("total_payments"),
        total_collected_fcfa: pay_row
            .get::<Option<f64>, _>("total_collected")
            .unwrap_or(0.0),
        pending_fcfa: pending_row
            .get::<Option<f64>, _>("pending_fcfa")
            .unwrap_or(0.0),
    };

    // Agent stats — commune_filter always applies for agents
    let agent_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'ACTIF') AS actifs,
            COUNT(*) FILTER (WHERE status = 'SUSPENDU') AS suspendus,
            COUNT(*) FILTER (WHERE status = 'RETRAITE') AS retraites
        FROM agents
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    let agents = AgentSummary {
        total: agent_row.get("total"),
        actifs: agent_row.get("actifs"),
        suspendus: agent_row.get("suspendus"),
        retraites: agent_row.get("retraites"),
    };

    // Signalement stats
    let sig_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'RECU') AS recu,
            COUNT(*) FILTER (WHERE status = 'EN_COURS') AS en_cours,
            COUNT(*) FILTER (WHERE status = 'TRAITE') AS traites,
            COUNT(*) FILTER (WHERE status = 'REJETE') AS rejetes
        FROM signalements
        WHERE ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    let signalements = SignalementSummary {
        total: sig_row.get("total"),
        recu: sig_row.get("recu"),
        en_cours: sig_row.get("en_cours"),
        traites: sig_row.get("traites"),
        rejetes: sig_row.get("rejetes"),
    };

    // Patrouilles stats
    let pat_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'EN_COURS') AS actives,
            COUNT(*) FILTER (WHERE status = 'PLANIFIEE') AS planifiees,
            COUNT(*) FILTER (WHERE status = 'CLOTUREE') AS cloturees
        FROM patrouilles
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(db)
    .await?;

    let patrouilles = PatrouillesSummary {
        actives: pat_row.get("actives"),
        planifiees: pat_row.get("planifiees"),
        cloturees: pat_row.get("cloturees"),
    };

    Ok(Json(SummaryResponse {
        pvs,
        payments,
        agents,
        signalements,
        patrouilles,
        commune_id: commune_filter,
    }))
}

async fn pv_stats(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<PvStatsResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) AS count
        FROM pvs
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        GROUP BY status
        ORDER BY count DESC
        "#,
    )
    .bind(commune_filter)
    .fetch_all(&state.db)
    .await?;

    let total: i64 = rows.iter().map(|r| r.get::<i64, _>("count")).sum();
    let by_status = rows
        .into_iter()
        .map(|r| StatusCount {
            status: r.get("status"),
            count: r.get("count"),
        })
        .collect();

    Ok(Json(PvStatsResponse {
        by_status,
        total,
        commune_id: commune_filter,
    }))
}

async fn payment_stats(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<PaymentStatsResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
        Role::Receveur,
    ])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_payments,
            COALESCE(SUM(amount_paid), 0) AS total_collected,
            COALESCE(SUM(amount_penalty), 0) AS total_penalties
        FROM payments
        WHERE status = 'PAYE'
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(&state.db)
    .await?;

    let pending_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS pending_count,
            COALESCE(SUM(amount_initial), 0) AS pending_fcfa
        FROM pvs
        WHERE status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
          AND deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(PaymentStatsResponse {
        total_payments: row.get("total_payments"),
        total_collected_fcfa: row.get::<Option<f64>, _>("total_collected").unwrap_or(0.0),
        total_penalties_fcfa: row.get::<Option<f64>, _>("total_penalties").unwrap_or(0.0),
        pending_count: pending_row.get("pending_count"),
        pending_fcfa: pending_row
            .get::<Option<f64>, _>("pending_fcfa")
            .unwrap_or(0.0),
        commune_id: commune_filter,
    }))
}

async fn agent_stats(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<AgentStatsResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let count_row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'ACTIF') AS actifs,
            COUNT(*) FILTER (WHERE status = 'SUSPENDU') AS suspendus
        FROM agents
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR commune_id = $1)
        "#,
    )
    .bind(commune_filter)
    .fetch_one(&state.db)
    .await?;

    let top_rows = sqlx::query(
        r#"
        SELECT a.id AS agent_id, a.full_name AS agent_nom, a.matricule, COUNT(p.id) AS pv_count
        FROM agents a
        LEFT JOIN pvs p ON p.agent_id = a.id AND p.deleted_at IS NULL
        WHERE a.deleted_at IS NULL
          AND ($1::uuid IS NULL OR a.commune_id = $1)
        GROUP BY a.id, a.full_name, a.matricule
        ORDER BY pv_count DESC
        LIMIT 10
        "#,
    )
    .bind(commune_filter)
    .fetch_all(&state.db)
    .await?;

    let top_agents = top_rows
        .into_iter()
        .map(|r| AgentActivity {
            agent_id: r.get("agent_id"),
            agent_nom: r.get("agent_nom"),
            matricule: r.get("matricule"),
            pv_count: r.get("pv_count"),
        })
        .collect();

    Ok(Json(AgentStatsResponse {
        total: count_row.get("total"),
        actifs: count_row.get("actifs"),
        suspendus: count_row.get("suspendus"),
        top_agents,
        commune_id: commune_filter,
    }))
}

async fn signalement_stats(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<SignalementStatsResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let rows = sqlx::query(
        r#"
        SELECT status, COUNT(*) AS count
        FROM signalements
        WHERE ($1::uuid IS NULL OR commune_id = $1)
        GROUP BY status
        ORDER BY count DESC
        "#,
    )
    .bind(commune_filter)
    .fetch_all(&state.db)
    .await?;

    let total: i64 = rows.iter().map(|r| r.get::<i64, _>("count")).sum();
    let by_status = rows
        .into_iter()
        .map(|r| StatusCount {
            status: r.get("status"),
            count: r.get("count"),
        })
        .collect();

    Ok(Json(SignalementStatsResponse {
        total,
        by_status,
        commune_id: commune_filter,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────
