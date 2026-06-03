use axum::extract::{Query, State};
use axum::response::Response;
use axum::{Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exports/pvs", axum::routing::get(export_pvs))
        .route("/exports/payments", axum::routing::get(export_payments))
        .route("/exports/signalements", axum::routing::get(export_signalements))
        .route("/exports/agents", axum::routing::get(export_agents))
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    commune_id: Option<Uuid>,
    status: Option<String>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

const MAX_ROWS: i64 = 10_000;

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn export_pvs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur, Role::Receveur])?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT p.pv_number, p.status, p.amount_initial, p.verbalized_name,
               p.vehicle_plate, p.location_description,
               p.created_at, c.nom AS commune_nom, a.matricule AS agent_matricule,
               a.full_name AS agent_nom, i.nom AS intervention_nom
        FROM pvs p
        JOIN communes c ON p.commune_id = c.id
        JOIN agents a ON p.agent_id = a.id
        JOIN interventions i ON p.intervention_id = i.id
        WHERE p.deleted_at IS NULL
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND p.commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        qb.push(" AND p.status = ").push_bind(s.clone());
    }
    apply_date_filter(&mut qb, "p.created_at", query.from, query.to);
    qb.push(" ORDER BY p.created_at DESC LIMIT ").push_bind(MAX_ROWS);

    let rows = qb.build().fetch_all(&state.db).await?;

    let mut csv = String::from("Numéro PV,Statut,Montant (FCFA),Verbalisé,Plaque,Lieu,Agent Matricule,Agent Nom,Commune,Intervention,Date\n");
    for row in &rows {
        let pv_number: String = row.get("pv_number");
        let status: String = row.get("status");
        let amount: Option<f64> = row.get("amount_initial");
        let verbalized: Option<String> = row.get("verbalized_name");
        let plate: Option<String> = row.get("vehicle_plate");
        let location: Option<String> = row.get("location_description");
        let agent_mat: String = row.get("agent_matricule");
        let agent_nom: String = row.get("agent_nom");
        let commune_nom: String = row.get("commune_nom");
        let interv_nom: String = row.get("intervention_nom");
        let created_at: DateTime<Utc> = row.get("created_at");

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&pv_number),
            csv_field(&status),
            amount.map(|a| format!("{:.0}", a)).unwrap_or_default(),
            csv_field(&verbalized.unwrap_or_default()),
            csv_field(&plate.unwrap_or_default()),
            csv_field(&location.unwrap_or_default()),
            csv_field(&agent_mat),
            csv_field(&agent_nom),
            csv_field(&commune_nom),
            csv_field(&interv_nom),
            created_at.format("%Y-%m-%d %H:%M"),
        ));
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "EXPORT_PVS",
        "pvs",
        None,
        None,
        Some(serde_json::json!({ "rows": rows.len(), "commune_id": commune_filter })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    csv_response(csv, "pvs")
}

async fn export_payments(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur, Role::Receveur])?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT pay.receipt_number, pay.amount_due, pay.amount_penalty, pay.amount_total,
               pay.amount_paid, pay.status, pay.paid_at,
               pv.pv_number, c.nom AS commune_nom,
               u.email AS receveur_email
        FROM payments pay
        JOIN pvs pv ON pay.pv_id = pv.id
        JOIN communes c ON pay.commune_id = c.id
        JOIN users u ON pay.receiver_user_id = u.id
        WHERE 1=1
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND pay.commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        qb.push(" AND pay.status = ").push_bind(s.clone());
    }
    apply_date_filter(&mut qb, "pay.created_at", query.from, query.to);
    qb.push(" ORDER BY pay.created_at DESC LIMIT ").push_bind(MAX_ROWS);

    let rows = qb.build().fetch_all(&state.db).await?;

    let mut csv = String::from("Numéro Reçu,PV,Montant Dû,Pénalités,Total Dû,Montant Encaissé,Statut,Date Paiement,Commune,Receveur\n");
    for row in &rows {
        let receipt: Option<String> = row.get("receipt_number");
        let pv_num: String = row.get("pv_number");
        let amount_due: f64 = row.get("amount_due");
        let penalty: f64 = row.get("amount_penalty");
        let total: f64 = row.get("amount_total");
        let paid: f64 = row.get("amount_paid");
        let status: String = row.get("status");
        let paid_at: Option<DateTime<Utc>> = row.get("paid_at");
        let commune_nom: String = row.get("commune_nom");
        let receveur: String = row.get("receveur_email");

        csv.push_str(&format!(
            "{},{},{:.0},{:.0},{:.0},{:.0},{},{},{},{}\n",
            csv_field(&receipt.unwrap_or_default()),
            csv_field(&pv_num),
            amount_due,
            penalty,
            total,
            paid,
            csv_field(&status),
            paid_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
            csv_field(&commune_nom),
            csv_field(&receveur),
        ));
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "EXPORT_PAYMENTS",
        "payments",
        None,
        None,
        Some(serde_json::json!({ "rows": rows.len(), "commune_id": commune_filter })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    csv_response(csv, "paiements")
}

async fn export_signalements(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT s.signalement_number, s.type_incident, s.location_description,
               s.status, s.contact_anonyme, s.created_at, c.nom AS commune_nom
        FROM signalements s
        JOIN communes c ON s.commune_id = c.id
        WHERE 1=1
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND s.commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        qb.push(" AND s.status = ").push_bind(s.clone());
    }
    apply_date_filter(&mut qb, "s.created_at", query.from, query.to);
    qb.push(" ORDER BY s.created_at DESC LIMIT ").push_bind(MAX_ROWS);

    let rows = qb.build().fetch_all(&state.db).await?;

    let mut csv = String::from("Numéro,Type Incident,Lieu,Statut,Anonyme,Commune,Date\n");
    for row in &rows {
        let num: String = row.get("signalement_number");
        let type_inc: String = row.get("type_incident");
        let location: String = row.get("location_description");
        let status: String = row.get("status");
        let anonyme: bool = row.get("contact_anonyme");
        let commune_nom: String = row.get("commune_nom");
        let created_at: DateTime<Utc> = row.get("created_at");

        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            csv_field(&num),
            csv_field(&type_inc),
            csv_field(&location),
            csv_field(&status),
            if anonyme { "Oui" } else { "Non" },
            csv_field(&commune_nom),
            created_at.format("%Y-%m-%d %H:%M"),
        ));
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "EXPORT_SIGNALEMENTS",
        "signalements",
        None,
        None,
        Some(serde_json::json!({ "rows": rows.len(), "commune_id": commune_filter })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    csv_response(csv, "signalements")
}

async fn export_agents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur])?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT a.matricule, a.full_name, a.grade, a.status, a.formation_nasla,
               a.date_prise_fonction, a.created_at, c.nom AS commune_nom
        FROM agents a
        JOIN communes c ON a.commune_id = c.id
        WHERE a.deleted_at IS NULL
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND a.commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        qb.push(" AND a.status = ").push_bind(s.clone());
    }
    apply_date_filter(&mut qb, "a.created_at", query.from, query.to);
    qb.push(" ORDER BY a.full_name ASC LIMIT ").push_bind(MAX_ROWS);

    let rows = qb.build().fetch_all(&state.db).await?;

    let mut csv = String::from("Matricule,Nom Complet,Grade,Statut,Formation NASLA,Date Prise Fonction,Commune,Créé le\n");
    for row in &rows {
        let matricule: String = row.get("matricule");
        let nom: String = row.get("full_name");
        let grade: String = row.get("grade");
        let status: String = row.get("status");
        let nasla: bool = row.get("formation_nasla");
        let date_pf: Option<chrono::NaiveDate> = row.get("date_prise_fonction");
        let commune_nom: String = row.get("commune_nom");
        let created_at: DateTime<Utc> = row.get("created_at");

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            csv_field(&matricule),
            csv_field(&nom),
            csv_field(&grade),
            csv_field(&status),
            if nasla { "Oui" } else { "Non" },
            date_pf.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            csv_field(&commune_nom),
            created_at.format("%Y-%m-%d"),
        ));
    }

    audit::record(
        &state.db,
        Some(auth_user.id),
        "EXPORT_AGENTS",
        "agents",
        None,
        None,
        Some(serde_json::json!({ "rows": rows.len(), "commune_id": commune_filter })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    csv_response(csv, "agents")
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn csv_response(content: String, name: &str) -> Result<Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let today = Utc::now().format("%Y-%m-%d");
    let filename = format!("{name}-{today}.csv");

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        content,
    )
        .into_response())
}

/// Échappe les champs CSV (guillemets doubles pour les virgules/guillemets).
fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn apply_date_filter(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    col: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) {
    if let Some(from_date) = from {
        let dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(
            from_date.and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        );
        qb.push(format!(" AND {col} >= ")).push_bind(dt);
    }
    if let Some(to_date) = to {
        let dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(
            to_date.and_hms_opt(23, 59, 59).unwrap(),
            Utc,
        );
        qb.push(format!(" AND {col} <= ")).push_bind(dt);
    }
}

fn resolve_commune_filter(
    auth_user: &AuthUser,
    requested: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        return Ok(requested);
    }
    let user_commune = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
    if let Some(req) = requested {
        if req != user_commune {
            return Err(ApiError::forbidden("Acces refuse a cette commune"));
        }
    }
    Ok(Some(user_commune))
}
