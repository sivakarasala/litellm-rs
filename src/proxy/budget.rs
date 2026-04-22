use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;

/// Check if a key has budget remaining. Returns Ok if budget allows, Err if exceeded.
pub async fn check_budget(virtual_key_id: Uuid, max_budget_usd: Decimal) -> Result<(), AppError> {
    let pool = crate::db::db().await?;

    // Sum total cost from usage_logs for this key
    let result = sqlx::query!(
        r#"SELECT COALESCE(SUM(cost_usd), 0) as "total_cost!: Decimal"
           FROM usage_logs
           WHERE virtual_key_id = $1"#,
        virtual_key_id,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    if result.total_cost >= max_budget_usd {
        Err(AppError::Validation(format!(
            "Budget exceeded: ${:.4} of ${:.4} used",
            result.total_cost, max_budget_usd
        )))
    } else {
        Ok(())
    }
}
