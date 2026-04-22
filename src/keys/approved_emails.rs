#[cfg(feature = "ssr")]
use crate::error::AppError;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::{ApprovedEmailId, ProviderKeyId};

/// Approved email info returned to the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovedEmailInfo {
    pub id: ApprovedEmailId,
    pub email: String,
    pub display_name: Option<String>,
    pub provider_key_id: Option<ProviderKeyId>,
    pub max_budget_usd: Option<String>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub default_expiry_hours: Option<i32>,
    pub is_active: bool,
    pub created_at: String,
}

/// Result of a self-service token request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenRequestResult {
    pub raw_key: String,
    pub name: String,
    pub expires_at: Option<String>,
    pub max_budget_usd: Option<String>,
}

#[server]
pub async fn list_approved_emails() -> Result<Vec<ApprovedEmailInfo>, ServerFnError> {
    use crate::auth::session::require_admin;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT id, email, display_name, provider_key_id, max_budget_usd,
                  rpm_limit, tpm_limit, default_expiry_hours, is_active, created_at
           FROM approved_emails
           ORDER BY created_at DESC"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ApprovedEmailInfo {
            id: ApprovedEmailId::from_uuid(r.id),
            email: r.email,
            display_name: r.display_name,
            provider_key_id: r.provider_key_id.map(ProviderKeyId::from_uuid),
            max_budget_usd: r.max_budget_usd.map(|d| d.to_string()),
            rpm_limit: r.rpm_limit,
            tpm_limit: r.tpm_limit,
            default_expiry_hours: r.default_expiry_hours,
            is_active: r.is_active,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[server]
pub async fn add_approved_email(
    email: String,
    display_name: Option<String>,
    provider_key_id: Option<String>,
    max_budget_usd: Option<String>,
    rpm_limit: Option<i32>,
    tpm_limit: Option<i32>,
    default_expiry_hours: Option<i32>,
) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;

    let email =
        crate::auth::validate_email(&email).map_err(|e| ServerFnError::new(e.to_string()))?;
    let email = email.as_str().to_string();

    let provider_uuid: Option<uuid::Uuid> = provider_key_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::Validation("Invalid provider key ID".into()))
        })
        .transpose()?;

    let budget: Option<rust_decimal::Decimal> = max_budget_usd
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::Validation("Invalid budget amount".into()))
        })
        .transpose()?;

    let display_name = display_name.filter(|s| !s.trim().is_empty());
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!(
        r#"INSERT INTO approved_emails
           (email, display_name, provider_key_id, max_budget_usd,
            rpm_limit, tpm_limit, default_expiry_hours, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        email,
        display_name as Option<String>,
        provider_uuid as Option<uuid::Uuid>,
        budget,
        rpm_limit,
        tpm_limit,
        default_expiry_hours,
        user_uuid
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| {
        if e.to_string().contains("duplicate key") {
            AppError::Validation("This email is already in the approved list".into())
        } else {
            AppError::Internal(e.to_string())
        }
    })?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "approved_email.created",
        Some("approved_email"),
        None,
        Some(serde_json::json!({"email": email})),
        None,
    )
    .await;

    Ok(())
}

#[server]
pub async fn delete_approved_email(email_id: String) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;
    let email_uuid: uuid::Uuid = email_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid email ID".into()))?;
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!("DELETE FROM approved_emails WHERE id = $1", email_uuid)
        .execute(&pool)
        .await
        .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "approved_email.deleted",
        Some("approved_email"),
        Some(&email_id),
        None,
        None,
    )
    .await;

    Ok(())
}

#[server]
pub async fn toggle_approved_email(email_id: String, active: bool) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;
    let email_uuid: uuid::Uuid = email_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid email ID".into()))?;
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!(
        "UPDATE approved_emails SET is_active = $1, updated_at = now() WHERE id = $2",
        active,
        email_uuid
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    let action = if active {
        "approved_email.enabled"
    } else {
        "approved_email.disabled"
    };

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        action,
        Some("approved_email"),
        Some(&email_id),
        None,
        None,
    )
    .await;

    Ok(())
}

/// Public server function: request a token via approved email.
#[server]
pub async fn request_token(
    name: String,
    email: String,
) -> Result<TokenRequestResult, ServerFnError> {
    use crate::keys::virtual_keys::keygen;
    use chrono::{Duration, Utc};

    let pool = crate::db::db().await?;

    let name = crate::auth::validate_name(&name).map_err(|e| ServerFnError::new(e.to_string()))?;

    let email =
        crate::auth::validate_email(&email).map_err(|e| ServerFnError::new(e.to_string()))?;
    let email = email.as_str().to_string();

    // Look up approved email (case-insensitive)
    let approved = sqlx::query!(
        r#"SELECT id, email, display_name, provider_key_id, max_budget_usd,
                  rpm_limit, tpm_limit, default_expiry_hours, is_active
           FROM approved_emails
           WHERE LOWER(email) = $1"#,
        email
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    let approved = match approved {
        Some(a) if a.is_active => a,
        _ => {
            // Generic message to prevent email enumeration
            return Err(
                AppError::Validation("Email not authorized. Contact your admin.".into()).into(),
            );
        }
    };

    // Find a provider key to use (either the one specified or the first active one)
    let provider_key_id = if let Some(pk_id) = approved.provider_key_id {
        pk_id
    } else {
        let default = sqlx::query!(
            "SELECT id FROM provider_keys WHERE is_active = true ORDER BY created_at ASC LIMIT 1"
        )
        .fetch_optional(&pool)
        .await
        .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("No active provider keys configured".into()))?;
        default.id
    };

    // Deactivate previous keys for this email
    sqlx::query!(
        "UPDATE virtual_keys SET is_active = false, updated_at = now() WHERE user_email = $1 AND is_active = true",
        email
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    // Compute expiry
    let expiry_hours = approved.default_expiry_hours.unwrap_or(720); // 30 days default
    let expires_at = if expiry_hours > 0 {
        Some(Utc::now() + Duration::hours(expiry_hours as i64))
    } else {
        None
    };
    let expires_at_str = expires_at.map(|dt| dt.to_rfc3339());

    // Generate and hash the key
    let raw_key = keygen::generate_key();
    let key_hash = keygen::hash_key(&raw_key)?;
    let key_prefix = keygen::extract_prefix(&raw_key);

    let budget_str = approved.max_budget_usd.as_ref().map(|d| d.to_string());

    sqlx::query!(
        r#"INSERT INTO virtual_keys
           (key_hash, key_prefix, name, provider_key_id, user_name, user_email,
            max_budget_usd, rpm_limit, tpm_limit, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::timestamptz)"#,
        key_hash,
        key_prefix,
        format!("{}'s key", name),
        provider_key_id,
        name,
        email,
        approved.max_budget_usd,
        approved.rpm_limit,
        approved.tpm_limit,
        expires_at_str as Option<String>,
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        None,
        "key.self_service_created",
        Some("virtual_key"),
        None,
        Some(serde_json::json!({"email": email, "name": name})),
        None,
    )
    .await;

    Ok(TokenRequestResult {
        raw_key,
        name,
        expires_at: expires_at.map(|dt| dt.to_rfc3339()),
        max_budget_usd: budget_str,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_request_result_serializes() {
        let result = TokenRequestResult {
            raw_key: "sk-litellm-test123".to_string(),
            name: "Test User".to_string(),
            expires_at: Some("2025-01-01T00:00:00Z".to_string()),
            max_budget_usd: Some("10.00".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("sk-litellm-test123"));
        assert!(json.contains("Test User"));
    }

    #[test]
    fn token_request_result_roundtrips() {
        let result = TokenRequestResult {
            raw_key: "sk-litellm-abc".to_string(),
            name: "User".to_string(),
            expires_at: None,
            max_budget_usd: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TokenRequestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.raw_key, "sk-litellm-abc");
        assert!(deserialized.expires_at.is_none());
        assert!(deserialized.max_budget_usd.is_none());
    }

    #[test]
    fn approved_email_info_roundtrips() {
        let json = r#"{
            "id": "test-id",
            "email": "user@example.com",
            "display_name": "Test User",
            "provider_key_id": null,
            "max_budget_usd": "50.00",
            "rpm_limit": 60,
            "tpm_limit": null,
            "default_expiry_hours": 720,
            "is_active": true,
            "created_at": "2025-01-01"
        }"#;
        let info: ApprovedEmailInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.email, "user@example.com");
        assert_eq!(info.rpm_limit, Some(60));
        assert!(info.is_active);
        assert_eq!(info.default_expiry_hours, Some(720));
        assert!(info.provider_key_id.is_none());
    }
}
