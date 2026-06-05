use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::resolve_commune_filter;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::pvs::{load_pv, record_status_change_tx};
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::sequences::{next_document_sequence, SEQUENCE_RECEIPT};
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
    pub amount_due_fcfa: Option<i64>,
    pub amount_penalty_fcfa: i64,
    pub amount_total_fcfa: Option<i64>,
    pub amount_paid_fcfa: Option<i64>,
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
    pub amount_due_fcfa: Option<i64>,
    pub amount_penalty_fcfa: i64,
    pub amount_total_fcfa: Option<i64>,
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
    pub amount_paid_fcfa: Option<i64>,
    pub amount_paid: Option<f64>,
}

struct PaymentComputation {
    amount_due_fcfa: i64,
    amount_penalty_fcfa: i64,
    amount_total_fcfa: i64,
    due_date: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn list_payments(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<PaymentFilterQuery>,
) -> Result<Json<Paginated<PaymentResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::Superviseur,
        Role::Receveur,
    ])?;

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

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT
            id, pv_id, commune_id,
            amount_due::DOUBLE PRECISION AS amount_due,
            amount_penalty::DOUBLE PRECISION AS amount_penalty,
            amount_total::DOUBLE PRECISION AS amount_total,
            amount_paid::DOUBLE PRECISION AS amount_paid,
            amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
            amount_paid_fcfa, receiver_user_id, paid_at, status,
            receipt_number, created_at
        FROM payments
        WHERE 1=1
        "#,
    );
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
) -> Result<Json<Paginated<PendingPvResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune, Role::Receveur])?;

    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) AS total FROM pvs p WHERE p.status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD') AND p.deleted_at IS NULL",
    );
    if let Some(id) = commune_filter {
        count_qb.push(" AND p.commune_id = ").push_bind(id);
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        r#"
        SELECT
            p.id AS pv_id,
            p.pv_number,
            p.commune_id,
            p.created_at
        FROM pvs p
        WHERE p.status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
          AND p.deleted_at IS NULL
        "#,
    );

    if let Some(id) = commune_filter {
        qb.push(" AND p.commune_id = ").push_bind(id);
    }
    qb.push(" ORDER BY p.created_at ASC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let pv_id: Uuid = row.get("pv_id");
        let created_at: DateTime<Utc> = row.get("created_at");
        let computed = payment_computation_pool(&state.db, pv_id, created_at).await?;
        items.push(PendingPvResponse {
            pv_id,
            pv_number: row.get("pv_number"),
            commune_id: row.get("commune_id"),
            amount_due: computed.amount_due_fcfa as f64,
            amount_penalty: computed.amount_penalty_fcfa as f64,
            amount_total: computed.amount_total_fcfa as f64,
            amount_due_fcfa: Some(computed.amount_due_fcfa),
            amount_penalty_fcfa: computed.amount_penalty_fcfa,
            amount_total_fcfa: Some(computed.amount_total_fcfa),
            due_date: computed.due_date,
            created_at,
        });
    }

    Ok(Json(Paginated::new(items, &pagination, total)))
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

    let amount_paid_fcfa = match payload.amount_paid_fcfa {
        Some(amount) => amount,
        None => payload
            .amount_paid
            .map(|amount| amount.round() as i64)
            .ok_or_else(|| ApiError::bad_request("amount_paid_fcfa est requis"))?,
    };
    if amount_paid_fcfa <= 0 {
        return Err(ApiError::bad_request(
            "Le montant encaisse doit etre positif",
        ));
    }
    let amount_paid = payload.amount_paid.unwrap_or(amount_paid_fcfa as f64);

    let mut tx = state.db.begin().await?;

    let pv_row = sqlx::query(
        r#"
        SELECT
            id, commune_id, status, pv_number, created_at
        FROM pvs
        WHERE id = $1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(pv_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::not_found("PV introuvable"))?;

    let pv_commune_id: Uuid = pv_row.get("commune_id");
    if pv_commune_id != commune_id {
        return Err(ApiError::forbidden(
            "Vous ne pouvez valider que les PV de votre commune",
        ));
    }

    let pv_status: String = pv_row.get("status");
    if pv_status != "EN_ATTENTE_PAIEMENT" && pv_status != "EN_RETARD" {
        return Err(ApiError::conflict(format!(
            "Ce PV n'est pas en attente de paiement (statut: {})",
            pv_status
        )));
    }

    let existing_payment: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM payments WHERE pv_id = $1 AND status = 'PAYE' FOR UPDATE",
    )
    .bind(pv_id)
    .fetch_optional(&mut *tx)
    .await?;

    if existing_payment.is_some() {
        return Err(ApiError::conflict("Ce PV a deja un paiement valide"));
    }

    let created_at: DateTime<Utc> = pv_row.get("created_at");
    let computed = payment_computation_tx(&mut tx, pv_id, created_at).await?;
    let amount_due_fcfa = computed.amount_due_fcfa;
    let amount_penalty_fcfa = computed.amount_penalty_fcfa;
    let amount_total_fcfa = computed.amount_total_fcfa;
    let amount_due = amount_due_fcfa as f64;
    let amount_penalty = amount_penalty_fcfa as f64;
    let amount_total = amount_total_fcfa as f64;

    if amount_paid_fcfa < amount_total_fcfa {
        return Err(ApiError::bad_request(format!(
            "Montant insuffisant: {} FCFA requis (dont {} FCFA de penalites), {} FCFA recu",
            amount_total_fcfa, amount_penalty_fcfa, amount_paid_fcfa
        )));
    }

    let receipt_number = generate_receipt_number(&mut tx, commune_id).await?;
    let payment_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO payments (
            id, pv_id, commune_id, amount_due, amount_penalty, amount_total,
            amount_paid, amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
            amount_paid_fcfa, receiver_user_id, paid_at, status, receipt_number
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10,
            $11, $12, now(), 'PAYE', $13
        )
        "#,
    )
    .bind(payment_id)
    .bind(pv_id)
    .bind(commune_id)
    .bind(amount_due)
    .bind(amount_penalty)
    .bind(amount_total)
    .bind(amount_paid)
    .bind(amount_due_fcfa)
    .bind(amount_penalty_fcfa)
    .bind(amount_total_fcfa)
    .bind(amount_paid_fcfa)
    .bind(auth_user.id)
    .bind(&receipt_number)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    sqlx::query("UPDATE pvs SET status = 'PAYE', updated_at = now() WHERE id = $1")
        .bind(pv_id)
        .execute(&mut *tx)
        .await?;

    record_status_change_tx(
        &mut tx,
        pv_id,
        Some(&pv_status),
        "PAYE",
        auth_user.id,
        Some("Paiement valide"),
    )
    .await;

    audit::record_for_commune_tx(
        &mut tx,
        Some(commune_id),
        Some(auth_user.id),
        "PAYMENT_VALIDATED",
        "payments",
        Some(payment_id),
        None,
        Some(json!({
            "pv_id": pv_id,
            "pv_number": pv_row.get::<String, _>("pv_number"),
            "amount_due_fcfa": amount_due_fcfa,
            "amount_penalty_fcfa": amount_penalty_fcfa,
            "amount_total_fcfa": amount_total_fcfa,
            "amount_paid_fcfa": amount_paid_fcfa,
            "receipt_number": receipt_number
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    tx.commit().await?;

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
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        pdf_bytes,
    )
        .into_response())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn load_payment(pool: &PgPool, id: Uuid) -> Result<PaymentResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT
            id, pv_id, commune_id,
            amount_due::DOUBLE PRECISION AS amount_due,
            amount_penalty::DOUBLE PRECISION AS amount_penalty,
            amount_total::DOUBLE PRECISION AS amount_total,
            amount_paid::DOUBLE PRECISION AS amount_paid,
            amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
            amount_paid_fcfa, receiver_user_id, paid_at, status,
            receipt_number, created_at
        FROM payments
        WHERE id = $1
        "#,
    )
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
        amount_due_fcfa: row.get("amount_due_fcfa"),
        amount_penalty_fcfa: row.get("amount_penalty_fcfa"),
        amount_total_fcfa: row.get("amount_total_fcfa"),
        amount_paid_fcfa: row.get("amount_paid_fcfa"),
        receiver_user_id: row.get("receiver_user_id"),
        paid_at: row.get("paid_at"),
        status: row.get("status"),
        receipt_number: row.get("receipt_number"),
        created_at: row.get("created_at"),
    }
}

/// Pénalité = montant × taux% si la date d'échéance est dépassée.
async fn payment_computation_pool(
    pool: &PgPool,
    pv_id: Uuid,
    created_at: DateTime<Utc>,
) -> Result<PaymentComputation, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT montant_fcfa, delai_paiement_jours,
               taux_penalite::DOUBLE PRECISION AS taux_penalite
        FROM pv_interventions
        WHERE pv_id = $1 AND sujet_paiement = TRUE
        ORDER BY order_index ASC
        "#,
    )
    .bind(pv_id)
    .fetch_all(pool)
    .await?;
    payment_computation_from_rows(rows, created_at)
}

async fn payment_computation_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pv_id: Uuid,
    created_at: DateTime<Utc>,
) -> Result<PaymentComputation, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT montant_fcfa, delai_paiement_jours,
               taux_penalite::DOUBLE PRECISION AS taux_penalite
        FROM pv_interventions
        WHERE pv_id = $1 AND sujet_paiement = TRUE
        ORDER BY order_index ASC
        "#,
    )
    .bind(pv_id)
    .fetch_all(&mut **tx)
    .await?;
    payment_computation_from_rows(rows, created_at)
}

fn payment_computation_from_rows(
    rows: Vec<sqlx::postgres::PgRow>,
    created_at: DateTime<Utc>,
) -> Result<PaymentComputation, ApiError> {
    if rows.is_empty() {
        return Err(ApiError::conflict("Ce PV n'a aucune infraction payante"));
    }

    let now = Utc::now();
    let mut amount_due_fcfa = 0_i64;
    let mut amount_penalty_fcfa = 0_i64;
    let mut earliest_due_date: Option<DateTime<Utc>> = None;
    for row in rows {
        let amount = row.get::<Option<i64>, _>("montant_fcfa").unwrap_or(0);
        let delay = row
            .get::<Option<i32>, _>("delai_paiement_jours")
            .unwrap_or(30);
        let rate = row
            .get::<Option<f64>, _>("taux_penalite")
            .unwrap_or(0.0);
        let due_date = created_at + Duration::days(delay as i64);
        amount_due_fcfa += amount;
        amount_penalty_fcfa += calculate_penalty_fcfa(amount, rate, due_date, now);
        earliest_due_date = Some(match earliest_due_date {
            Some(existing) if existing <= due_date => existing,
            _ => due_date,
        });
    }

    Ok(PaymentComputation {
        amount_due_fcfa,
        amount_penalty_fcfa,
        amount_total_fcfa: amount_due_fcfa + amount_penalty_fcfa,
        due_date: earliest_due_date.unwrap_or(created_at),
    })
}

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

fn calculate_penalty_fcfa(
    amount_due_fcfa: i64,
    penalty_rate: f64,
    due_date: DateTime<Utc>,
    now: DateTime<Utc>,
) -> i64 {
    if now <= due_date || penalty_rate <= 0.0 {
        0
    } else {
        ((amount_due_fcfa as f64) * penalty_rate / 100.0).round() as i64
    }
}

async fn generate_receipt_number(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
) -> Result<String, ApiError> {
    let commune_code: String = sqlx::query_scalar("SELECT code FROM communes WHERE id = $1")
        .bind(commune_id)
        .fetch_one(&mut **tx)
        .await?;
    let (year, seq) = next_document_sequence(tx, commune_id, SEQUENCE_RECEIPT).await?;
    Ok(format!(
        "REC-{}-{}-{:06}",
        commune_code.to_uppercase(),
        year,
        seq
    ))
}
