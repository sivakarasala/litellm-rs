/// Log an action to the audit_log table.
/// Errors are logged but never propagated — audit must not break the main flow.
#[cfg(feature = "ssr")]
pub async fn log_audit(
    pool: &sqlx::PgPool,
    user_id: Option<uuid::Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    details: Option<serde_json::Value>,
    ip_address: Option<&str>,
) {
    let result = sqlx::query!(
        r#"INSERT INTO audit_log (user_id, action, target_type, target_id, details, ip_address)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
        user_id,
        action,
        target_type,
        target_id,
        details,
        ip_address
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::error!("Failed to write audit log: {}", e);
    }
}
