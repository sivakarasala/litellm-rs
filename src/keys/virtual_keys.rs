#[cfg(feature = "ssr")]
use crate::error::AppError;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::{ProviderKeyId, VirtualKeyId};

/// Duration options for virtual key expiry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ExpiryDuration {
    Hours1,
    Hours6,
    Hours24,
    Days7,
    Days30,
    Days90,
    Never,
}

#[cfg(feature = "ssr")]
impl ExpiryDuration {
    /// Convert to a concrete datetime from now, or None for Never.
    pub fn to_expires_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        match self {
            ExpiryDuration::Hours1 => Some(now + Duration::hours(1)),
            ExpiryDuration::Hours6 => Some(now + Duration::hours(6)),
            ExpiryDuration::Hours24 => Some(now + Duration::hours(24)),
            ExpiryDuration::Days7 => Some(now + Duration::days(7)),
            ExpiryDuration::Days30 => Some(now + Duration::days(30)),
            ExpiryDuration::Days90 => Some(now + Duration::days(90)),
            ExpiryDuration::Never => None,
        }
    }

    /// Parse from a form string value.
    pub fn from_form_value(s: &str) -> Result<Self, AppError> {
        match s {
            "Hours1" => Ok(Self::Hours1),
            "Hours6" => Ok(Self::Hours6),
            "Hours24" => Ok(Self::Hours24),
            "Days7" => Ok(Self::Days7),
            "Days30" => Ok(Self::Days30),
            "Days90" => Ok(Self::Days90),
            "Never" => Ok(Self::Never),
            _ => Err(AppError::Validation("Invalid expiry option".into())),
        }
    }
}

/// Virtual key info returned to the UI — never includes the hash or raw key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualKeyInfo {
    pub id: VirtualKeyId,
    pub key_prefix: String,
    pub name: String,
    pub provider_key_id: ProviderKeyId,
    pub is_active: bool,
    pub expires_at: Option<String>,
    pub max_budget_usd: Option<String>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub created_at: String,
}

/// Result of creating a virtual key — includes the raw key (shown once).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualKeyCreated {
    pub id: VirtualKeyId,
    pub raw_key: String,
    pub name: String,
    pub expires_at: Option<String>,
}

// ---- Key generation (SSR only) ----

#[cfg(feature = "ssr")]
pub mod keygen {
    use crate::error::AppError;

    const KEY_PREFIX: &str = "sk-litellm-";
    const KEY_RANDOM_BYTES: usize = 32;

    /// Generate a random virtual key: `sk-litellm-<64 hex chars>`.
    pub fn generate_key() -> String {
        use rand::RngCore;
        let mut bytes = [0u8; KEY_RANDOM_BYTES];
        rand::thread_rng().fill_bytes(&mut bytes);
        format!("{}{}", KEY_PREFIX, hex::encode(bytes))
    }

    /// Extract the display prefix from a raw key: first 15 chars + "...".
    pub fn extract_prefix(key: &str) -> String {
        if key.len() <= 15 {
            key.to_string()
        } else {
            format!("{}...", &key[..15])
        }
    }

    /// Hash a virtual key with argon2 for secure storage.
    pub fn hash_key(key: &str) -> Result<String, AppError> {
        use argon2::{
            password_hash::{rand_core::OsRng, SaltString},
            Argon2, PasswordHasher,
        };

        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(key.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AppError::Internal(format!("Key hashing failed: {e}")))
    }

    /// Verify a raw key against a stored argon2 hash.
    pub fn verify_key(key: &str, hash: &str) -> Result<bool, AppError> {
        use argon2::{Argon2, PasswordHash, PasswordVerifier};

        let parsed = PasswordHash::new(hash)
            .map_err(|e| AppError::Internal(format!("Invalid hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(key.as_bytes(), &parsed)
            .is_ok())
    }
}

#[server]
pub async fn list_virtual_keys() -> Result<Vec<VirtualKeyInfo>, ServerFnError> {
    use crate::auth::session::require_admin;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT id, key_prefix, name, provider_key_id, is_active,
                  expires_at, max_budget_usd, rpm_limit, tpm_limit, created_at
           FROM virtual_keys
           ORDER BY created_at DESC"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| VirtualKeyInfo {
            id: VirtualKeyId::from_uuid(r.id),
            key_prefix: r.key_prefix,
            name: r.name,
            provider_key_id: ProviderKeyId::from_uuid(r.provider_key_id),
            is_active: r.is_active,
            expires_at: r.expires_at.map(|t| t.to_string()),
            max_budget_usd: r.max_budget_usd.map(|d| d.to_string()),
            rpm_limit: r.rpm_limit,
            tpm_limit: r.tpm_limit,
            created_at: r.created_at.to_string(),
        })
        .collect())
}

#[server]
pub async fn create_virtual_key(
    name: String,
    provider_key_id: String,
    expiry: String,
    max_budget_usd: Option<String>,
    rpm_limit: Option<i32>,
    tpm_limit: Option<i32>,
) -> Result<VirtualKeyCreated, ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;

    let name = crate::auth::validate_name(&name)?;

    let provider_uuid: uuid::Uuid = provider_key_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid provider key ID".into()))?;

    // Verify provider key exists and is active
    let provider = sqlx::query!(
        "SELECT is_active FROM provider_keys WHERE id = $1",
        provider_uuid
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?
    .ok_or_else(|| AppError::Validation("Provider key not found".into()))?;

    if !provider.is_active {
        return Err(AppError::Validation("Provider key is inactive".into()).into());
    }

    let expiry_duration = ExpiryDuration::from_form_value(&expiry)?;
    let expires_at = expiry_duration.to_expires_at();

    let budget: Option<rust_decimal::Decimal> = max_budget_usd
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::Validation("Invalid budget amount".into()))
        })
        .transpose()?;

    let raw_key = keygen::generate_key();
    let key_hash = keygen::hash_key(&raw_key)?;
    let key_prefix = keygen::extract_prefix(&raw_key);
    let user_uuid = user.id.as_uuid()?;
    let expires_at_str = expires_at.map(|dt| dt.to_rfc3339());

    let rec = sqlx::query!(
        r#"INSERT INTO virtual_keys
           (key_hash, key_prefix, name, provider_key_id, max_budget_usd,
            rpm_limit, tpm_limit, expires_at, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9)
           RETURNING id"#,
        key_hash,
        key_prefix,
        name,
        provider_uuid,
        budget,
        rpm_limit,
        tpm_limit,
        expires_at_str as Option<String>,
        user_uuid
    )
    .fetch_one(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "virtual_key.created",
        Some("virtual_key"),
        Some(&rec.id.to_string()),
        Some(serde_json::json!({"name": name})),
        None,
    )
    .await;

    Ok(VirtualKeyCreated {
        id: VirtualKeyId::from_uuid(rec.id),
        raw_key,
        name,
        expires_at: expires_at.map(|t| t.to_string()),
    })
}

#[server]
pub async fn toggle_virtual_key(key_id: String, active: bool) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;
    let key_uuid: uuid::Uuid = key_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid key ID".into()))?;
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!(
        "UPDATE virtual_keys SET is_active = $1, updated_at = now() WHERE id = $2",
        active,
        key_uuid
    )
    .execute(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    let action = if active {
        "virtual_key.enabled"
    } else {
        "virtual_key.disabled"
    };

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        action,
        Some("virtual_key"),
        Some(&key_id),
        None,
        None,
    )
    .await;

    Ok(())
}

#[server]
pub async fn delete_virtual_key(key_id: String) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;
    let key_uuid: uuid::Uuid = key_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid key ID".into()))?;
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!("DELETE FROM virtual_keys WHERE id = $1", key_uuid)
        .execute(&pool)
        .await
        .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "virtual_key.deleted",
        Some("virtual_key"),
        Some(&key_id),
        None,
        None,
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::keygen;
    use super::ExpiryDuration;

    // ── Key generation ──

    #[test]
    fn generated_key_has_correct_prefix() {
        let key = keygen::generate_key();
        assert!(
            key.starts_with("sk-litellm-"),
            "key must start with sk-litellm-"
        );
    }

    #[test]
    fn generated_key_has_correct_length() {
        // "sk-litellm-" (11) + 64 hex chars = 75
        let key = keygen::generate_key();
        assert_eq!(key.len(), 75);
    }

    #[test]
    fn generated_keys_are_unique() {
        let key1 = keygen::generate_key();
        let key2 = keygen::generate_key();
        assert_ne!(key1, key2);
    }

    #[test]
    fn generated_key_suffix_is_valid_hex() {
        let key = keygen::generate_key();
        let hex_part = &key["sk-litellm-".len()..];
        assert!(hex::decode(hex_part).is_ok(), "suffix must be valid hex");
    }

    // ── Prefix extraction ──

    #[test]
    fn extract_prefix_long_key() {
        let key = "sk-litellm-abcdef1234567890abcdef";
        assert_eq!(keygen::extract_prefix(key), "sk-litellm-abcd...");
    }

    #[test]
    fn extract_prefix_short_key() {
        assert_eq!(keygen::extract_prefix("short"), "short");
    }

    #[test]
    fn extract_prefix_exactly_15() {
        assert_eq!(keygen::extract_prefix("123456789012345"), "123456789012345");
    }

    // ── Hashing ──

    #[test]
    fn hash_and_verify_roundtrip() {
        let key = keygen::generate_key();
        let hash = keygen::hash_key(&key).unwrap();
        assert!(keygen::verify_key(&key, &hash).unwrap());
    }

    #[test]
    fn verify_wrong_key_fails() {
        let key = keygen::generate_key();
        let hash = keygen::hash_key(&key).unwrap();
        let wrong_key = keygen::generate_key();
        assert!(!keygen::verify_key(&wrong_key, &hash).unwrap());
    }

    #[test]
    fn hash_is_argon2_format() {
        let key = keygen::generate_key();
        let hash = keygen::hash_key(&key).unwrap();
        assert_ne!(hash, key);
        assert!(hash.starts_with("$argon2"), "hash must be argon2 format");
    }

    // ── Expiry ──

    #[test]
    fn expiry_never_returns_none() {
        assert!(ExpiryDuration::Never.to_expires_at().is_none());
    }

    #[test]
    fn expiry_hours1_is_in_future() {
        let expires = ExpiryDuration::Hours1.to_expires_at().unwrap();
        assert!(expires > chrono::Utc::now());
    }

    #[test]
    fn expiry_days30_is_roughly_30_days() {
        let expires = ExpiryDuration::Days30.to_expires_at().unwrap();
        let diff = expires - chrono::Utc::now();
        let days = diff.num_days();
        assert!(days >= 29 && days <= 30);
    }

    // ── ExpiryDuration parsing ──

    #[test]
    fn parse_valid_expiry() {
        assert_eq!(
            ExpiryDuration::from_form_value("Hours1").unwrap(),
            ExpiryDuration::Hours1
        );
        assert_eq!(
            ExpiryDuration::from_form_value("Never").unwrap(),
            ExpiryDuration::Never
        );
    }

    #[test]
    fn parse_invalid_expiry() {
        assert!(ExpiryDuration::from_form_value("InvalidOption").is_err());
    }
}
