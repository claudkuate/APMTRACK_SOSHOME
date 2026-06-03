use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::pvs::{load_pv, record_status_change};
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payments", axum::routing::get(list_payments))
        .route("/payments/pending", axum::routing::get(list_pending))
        .route(
            "/payments/{pv_id}/validate",
            axum::routing::post(validate_payment),
        )
        .route(
            "/payments/{id}/receipt",
            axum::routing::get(get_receipt_pdf),
        )
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub pv_id: Uuid,
    pub commune_id: Uuid,
    pub amount_due: f64,
    pub amount_penalty: f64,
    pub amount_total: f64,
    pub amount_paid: f64,
    pub receiver_user_id: Uuid,
    pub paid_at: Option<DateTime<Utc>>,
    pub status: String,
    pub receipt_number: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PendingPvResponse {
    pub pv_id: Uuid,
    pub pv_number: String,
    pub commune_id: Uuid,
    pub amount_due: f64,
    pub amount_penalty: f64,
    pub amount_total: f64,
    pub due_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidatePaymentRequest {
    pub amount_paid: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_payments(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PaymentFilterQuery>,
) -> Result<Json<Paginated<PaymentResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Superviseur, Role::Receveur])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM payments WHERE 1=1");
    if let Some(id) = commune_filter {
        count_qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        count_qb.push(" AND status = ").push_bind(s.clone());
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM payments WHERE 1=1");
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = query.status {
        qb.push(" AND status = ").push_bind(s.clone());
    }
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_payment).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn list_pending(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PaymentFilterQuery>,
) -> Result<Json<Vec<PendingPvResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Receveur])?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT
            p.id AS pv_id,
            p.pv_number,
            p.commune_id,
            p.amount_initial,
            p.created_at,
            i.delai_paiement_jours,
            i.taux_penalite
        FROM pvs p
        JOIN interventions i ON p.intervention_id = i.id
        WHERE p.status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
          AND p.deleted_at IS NULL
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND p.commune_id = ").push_bind(id);
    }
    qb.push(" ORDER BY p.created_at ASC LIMIT 100");

    let rows = qb.build().fetch_all(&state.db).await?;
    let now = Utc::now();

    let items = rows
        .into_iter()
        .map(|row| {
            let amount_due: f64 = row.get::<Option<f64>, _>("amount_initial").unwrap_or(0.0);
            let delai: i32 = row.get::<Option<i32>, _>("delai_paiement_jours").unwrap_or(30);
            let rate: f64 = row.get::<Option<f64>, _>("taux_penalite").unwrap_or(0.0);
            let created_at: DateTime<Utc> = row.get("created_at");
            let due_date = created_at + Duration::days(delai as i64);
            let penalty = calculate_penalty(amount_due, rate, due_date, now);
            PendingPvResponse {
                pv_id: row.get("pv_id"),
                pv_number: row.get("pv_number"),
                commune_id: row.get("commune_id"),
                amount_due,
                amount_penalty: penalty,
                amount_total: amount_due + penalty,
                due_date,
                created_at,
            }
        })
        .collect();

    Ok(Json(items))
}

async fn validate_payment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(pv_id): Path<Uuid>,
    ApiJson(payload): ApiJson<ValidatePaymentRequest>,
) -> Result<(StatusCode, Json<PaymentResponse>), ApiError> {
    auth_user.require_any_role(&[Role::Receveur])?;

    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Receveur non rattache a une commune"))?;

    let pv = load_pv(&state.db, pv_id).await?;

    if pv.commune_id != commune_id {
        return Err(ApiError::forbidden(
            "Vous ne pouvez valider que les PV de votre commune",
        ));
    }

    if pv.status != "EN_ATTENTE_PAIEMENT" && pv.status != "EN_RETARD" {
        return Err(ApiError::conflict(format!(
            "Ce PV n'est pas en attente de paiement (statut: {})",
            pv.status
        )));
    }

    // Vérifier qu'aucun paiement validé n'existe
    let existing_payment: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM payments WHERE pv_id = $1 AND status = 'PAYE'",
    )
    .bind(pv_id)
    .fetch_optional(&state.db)
    .await?;

    if existing_payment.is_some() {
        return Err(ApiError::conflict("Ce PV a déjà un paiement validé"));
    }

    // Récupérer les termes de paiement depuis l'intervention
    let interv_row = sqlx::query(
        "SELECT delai_paiement_jours, taux_penalite FROM interventions WHERE id = $1",
    )
    .bind(pv.intervention_id)
    .fetch_one(&state.db)
    .await?;

    let delai: i32 = interv_row
        .get::<Option<i32>, _>("delai_paiement_jours")
        .unwrap_or(30);
    let rate: f64 = interv_row
        .get::<Option<f64>, _>("taux_penalite")
        .unwrap_or(0.0);

    let amount_due = pv.amount_initial.unwrap_or(0.0);
    let due_date = pv.created_at + Duration::days(delai as i64);
    let now = Utc::now();
    let amount_penalty = calculate_penalty(amount_due, rate, due_date, now);
    let amount_total = amount_due + amount_penalty;

    if payload.amount_paid < amount_total {
        return Err(ApiError::bad_request(format!(
            "Montant insuffisant: {:.2} requis (dont {:.2} de pénalités), {:.2} reçu",
            amount_total, amount_penalty, payload.amount_paid
        )));
    }

    let receipt_number = generate_receipt_number(&state.db, commune_id).await?;
    let payment_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO payments (
            id, pv_id, commune_id, amount_due, amount_penalty, amount_total,
            amount_paid, receiver_user_id, paid_at, status, receipt_number
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, now(), 'PAYE', $9
        )
        "#,
    )
    .bind(payment_id)
    .bind(pv_id)
    .bind(commune_id)
    .bind(amount_due)
    .bind(amount_penalty)
    .bind(amount_total)
    .bind(payload.amount_paid)
    .bind(auth_user.id)
    .bind(&receipt_number)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    // Mettre à jour le statut PV
    sqlx::query(
        "UPDATE pvs SET status = 'PAYE', updated_at = now() WHERE id = $1",
    )
    .bind(pv_id)
    .execute(&state.db)
    .await?;

    record_status_change(
        &state.db,
        pv_id,
        Some(&pv.status),
        "PAYE",
        auth_user.id,
        Some("Paiement validé"),
    )
    .await;

    audit::record(
        &state.db,
        Some(auth_user.id),
        "PAYMENT_VALIDATED",
        "payments",
        Some(payment_id),
        None,
        Some(json!({
            "pv_id": pv_id,
            "pv_number": pv.pv_number,
            "amount_due": amount_due,
            "amount_penalty": amount_penalty,
            "amount_total": amount_total,
            "amount_paid": payload.amount_paid,
            "receipt_number": receipt_number
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(load_payment(&state.db, payment_id).await?),
    ))
}

async fn get_receipt_pdf(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Receveur])?;

    let payment = load_payment(&state.db, id).await?;
    auth_user.require_commune_access(payment.commune_id)?;

    let pv = load_pv(&state.db, payment.pv_id).await?;
    let pdf_bytes = crate::modules::pdf::generate_receipt_pdf(&state.db, &payment, &pv).await?;

    use axum::http::header;
    use axum::response::IntoResponse;
    let filename = format!(
        "Recu-{}.pdf",
        payment.receipt_number.as_deref().unwrap_or(&id.to_string())
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        pdf_bytes,
    )
        .into_response())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_payment(pool: &PgPool, id: Uuid) -> Result<PaymentResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM payments WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Paiement introuvable"))?;
    Ok(row_to_payment(row))
}

fn row_to_payment(row: sqlx::postgres::PgRow) -> PaymentResponse {
    PaymentResponse {
        id: row.get("id"),
        pv_id: row.get("pv_id"),
        commune_id: row.get("commune_id"),
        amount_due: row.get("amount_due"),
        amount_penalty: row.get("amount_penalty"),
        amount_total: row.get("amount_total"),
        amount_paid: row.get("amount_paid"),
        receiver_user_id: row.get("receiver_user_id"),
        paid_at: row.get("paid_at"),
        status: row.get("status"),
        receipt_number: row.get("receipt_number"),
        created_at: row.get("created_at"),
    }
}

/// Pénalité = montant × taux% si la date d'échéance est dépassée.
pub fn calculate_penalty(
    amount_due: f64,
    penalty_rate: f64,
    due_date: DateTime<Utc>,
    now: DateTime<Utc>,
) -> f64 {
    if now <= due_date || penalty_rate <= 0.0 {
        0.0
    } else {
        (amount_due * penalty_rate / 100.0 * 100.0).round() / 100.0
    }
}

async fn generate_receipt_number(
    pool: &PgPool,
    commune_id: Uuid,
) -> Result<String, ApiError> {
    use chrono::Datelike;
    let year = Utc::now().year();
    let commune_code: String =
        sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
            .bind(commune_id)
            .fetch_one(pool)
            .await?;
    let seq: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM payments WHERE commune_id = $1 AND EXTRACT(YEAR FROM created_at) = $2",
    )
    .bind(commune_id)
    .bind(year as i64)
    .fetch_one(pool)
    .await?;
    Ok(format!(
        "REC-{}-{}-{:06}",
        commune_code.to_uppercase(),
        year,
        seq
    ))
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
