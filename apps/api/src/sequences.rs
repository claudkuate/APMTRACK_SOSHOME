use chrono::{Datelike, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::errors::ApiError;

pub const SEQUENCE_PV: &str = "PV";
pub const SEQUENCE_RECEIPT: &str = "RECEIPT";
pub const SEQUENCE_SIGNALEMENT: &str = "SIGNALEMENT";
pub const SEQUENCE_FOURRIERE: &str = "FOURRIERE";

pub async fn next_document_sequence(
    tx: &mut Transaction<'_, Postgres>,
    commune_id: Uuid,
    kind: &str,
) -> Result<(i32, i64), ApiError> {
    let year = Utc::now().year();
    let seq: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO document_sequences (commune_id, kind, year, next_value)
        VALUES ($1, $2, $3, 2)
        ON CONFLICT (commune_id, kind, year)
        DO UPDATE SET next_value = document_sequences.next_value + 1
        RETURNING next_value - 1
        "#,
    )
    .bind(commune_id)
    .bind(kind)
    .bind(year)
    .fetch_one(&mut **tx)
    .await?;

    Ok((year, seq))
}
