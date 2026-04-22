use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;

/// All data needed to record a usage log entry.
pub struct UsageRecord {
    pub virtual_key_id: Uuid,
    pub provider_key_id: Uuid,
    pub model: String,
    pub endpoint: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    pub cost_usd: Decimal,
    pub cached: bool,
    pub status_code: i32,
    pub latency_ms: i32,
}

/// Record a usage log entry for a proxy request.
pub async fn record_usage(record: UsageRecord) -> Result<(), AppError> {
    let pool = crate::db::db().await?;

    sqlx::query!(
        r#"INSERT INTO usage_logs
           (virtual_key_id, provider_key_id, model, endpoint,
            input_tokens, output_tokens, total_tokens, cost_usd,
            cached, status_code, latency_ms)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        record.virtual_key_id,
        record.provider_key_id,
        record.model,
        record.endpoint,
        record.input_tokens,
        record.output_tokens,
        record.total_tokens,
        record.cost_usd,
        record.cached,
        record.status_code,
        record.latency_ms,
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(())
}
