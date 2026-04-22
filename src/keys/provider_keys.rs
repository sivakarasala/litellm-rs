#[cfg(feature = "ssr")]
use crate::error::AppError;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::ProviderKeyId;

/// Provider key as returned to the UI — API key is masked, never exposed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderKeyInfo {
    pub id: ProviderKeyId,
    pub name: String,
    pub provider: String,
    pub base_url: String,
    pub is_active: bool,
    pub key_preview: String,
    pub created_at: String,
}

// ---- Encryption helpers (SSR only) ----

#[cfg(feature = "ssr")]
pub mod crypto {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    use crate::error::AppError;

    /// Parse the 64-char hex master key from configuration.
    pub fn master_key() -> Result<[u8; 32], AppError> {
        let config = crate::configuration::get_configuration()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let hex_key = secrecy::ExposeSecret::expose_secret(&config.encryption.master_key);
        parse_hex_key(hex_key)
    }

    /// Parse a hex-encoded 32-byte key.
    pub fn parse_hex_key(hex_key: &str) -> Result<[u8; 32], AppError> {
        let bytes = hex::decode(hex_key)
            .map_err(|_| AppError::Internal("Invalid master key hex".into()))?;
        bytes
            .try_into()
            .map_err(|_| AppError::Internal("Master key must be 32 bytes (64 hex chars)".into()))
    }

    /// Encrypt with AES-256-GCM using a provided key. Returns (ciphertext, nonce).
    pub fn encrypt_with_key(
        key: &[u8; 32],
        plaintext: &str,
    ) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        use aes_gcm::aead::OsRng;
        use aes_gcm::AeadCore;

        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| AppError::Internal(e.to_string()))?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption failed: {e}")))?;
        Ok((ciphertext, nonce.to_vec()))
    }

    /// Decrypt with AES-256-GCM using a provided key.
    pub fn decrypt_with_key(
        key: &[u8; 32],
        ciphertext: &[u8],
        nonce_bytes: &[u8],
    ) -> Result<String, AppError> {
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| AppError::Internal(e.to_string()))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::Internal("Decryption failed — wrong key?".into()))?;
        String::from_utf8(plaintext)
            .map_err(|e| AppError::Internal(format!("Decrypted key is not UTF-8: {e}")))
    }

    /// Encrypt using the master key from configuration.
    pub fn encrypt_api_key(plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        let key = master_key()?;
        encrypt_with_key(&key, plaintext)
    }

    /// Decrypt using the master key from configuration.
    pub fn decrypt_api_key(ciphertext: &[u8], nonce_bytes: &[u8]) -> Result<String, AppError> {
        let key = master_key()?;
        decrypt_with_key(&key, ciphertext, nonce_bytes)
    }

    /// Mask an API key for display: "sk-proj-abc...xyz"
    pub fn mask_api_key(key: &str) -> String {
        if key.len() <= 8 {
            "sk-****".to_string()
        } else {
            format!("{}...{}", &key[..7], &key[key.len() - 4..])
        }
    }
}

#[server]
pub async fn list_provider_keys() -> Result<Vec<ProviderKeyInfo>, ServerFnError> {
    use crate::auth::session::require_admin;

    require_admin().await?;
    let pool = crate::db::db().await?;

    let rows = sqlx::query!(
        r#"SELECT id, name, provider, base_url, is_active,
                  api_key_encrypted, api_key_nonce, created_at
           FROM provider_keys
           ORDER BY created_at DESC"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut keys = Vec::with_capacity(rows.len());
    for r in rows {
        let decrypted = crypto::decrypt_api_key(&r.api_key_encrypted, &r.api_key_nonce)?;
        keys.push(ProviderKeyInfo {
            id: ProviderKeyId::from_uuid(r.id),
            name: r.name,
            provider: r.provider,
            base_url: r.base_url,
            is_active: r.is_active,
            key_preview: crypto::mask_api_key(&decrypted),
            created_at: r.created_at.to_string(),
        });
    }
    Ok(keys)
}

#[server]
pub async fn add_provider_key(
    name: String,
    provider: String,
    api_key: String,
    base_url: String,
) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("Name is required".into()).into());
    }
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err(AppError::Validation("API key is required".into()).into());
    }
    let base_url = if base_url.trim().is_empty() {
        "https://api.openai.com".to_string()
    } else {
        base_url.trim().to_string()
    };

    let (encrypted, nonce) = crypto::encrypt_api_key(&api_key)?;
    let user_uuid = user.id.as_uuid()?;

    sqlx::query!(
        r#"INSERT INTO provider_keys (name, provider, api_key_encrypted, api_key_nonce, base_url, created_by)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
        name,
        provider,
        encrypted,
        nonce,
        base_url,
        user_uuid
    )
    .execute(&pool)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "provider_key.created",
        Some("provider_key"),
        None,
        Some(serde_json::json!({"name": name, "provider": provider})),
        None,
    )
    .await;

    Ok(())
}

#[server]
pub async fn delete_provider_key(key_id: String) -> Result<(), ServerFnError> {
    use crate::auth::session::require_admin;

    let user = require_admin().await?;
    let pool = crate::db::db().await?;
    let key_uuid: uuid::Uuid = key_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid key ID".into()))?;
    let user_uuid = user.id.as_uuid()?;

    // Check for active virtual keys using this provider key
    let active_count = sqlx::query!(
        "SELECT COUNT(*) AS count FROM virtual_keys WHERE provider_key_id = $1 AND is_active = true",
        key_uuid
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    if active_count.count.unwrap_or(0) > 0 {
        return Err(AppError::Validation(format!(
            "{} active virtual key(s) use this provider key. Disable or reassign them first.",
            active_count.count.unwrap_or(0)
        ))
        .into());
    }

    sqlx::query!("DELETE FROM provider_keys WHERE id = $1", key_uuid)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    crate::audit::log_audit(
        &pool,
        Some(user_uuid),
        "provider_key.deleted",
        Some("provider_key"),
        Some(&key_id),
        None,
        None,
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::crypto;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let plaintext = "sk-proj-abc123xyz";

        let (ciphertext, nonce) = crypto::encrypt_with_key(&key, plaintext).unwrap();

        assert_ne!(
            ciphertext,
            plaintext.as_bytes(),
            "ciphertext must differ from plaintext"
        );
        assert_eq!(nonce.len(), 12, "AES-GCM nonce must be 12 bytes");

        let decrypted = crypto::decrypt_with_key(&key, &ciphertext, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let key2 = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000002",
        )
        .unwrap();

        let (ciphertext, nonce) = crypto::encrypt_with_key(&key1, "secret-key").unwrap();
        let result = crypto::decrypt_with_key(&key2, &ciphertext, &nonce);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Decryption failed"));
    }

    #[test]
    fn decrypt_with_tampered_ciphertext_fails() {
        let key = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();

        let (mut ciphertext, nonce) = crypto::encrypt_with_key(&key, "secret-key").unwrap();
        ciphertext[0] ^= 0xFF; // flip bits

        let result = crypto::decrypt_with_key(&key, &ciphertext, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn mask_long_key() {
        let masked = crypto::mask_api_key("sk-proj-abcdefghijklmnop");
        assert_eq!(masked, "sk-proj...mnop");
    }

    #[test]
    fn mask_short_key() {
        let masked = crypto::mask_api_key("sk-1234");
        assert_eq!(masked, "sk-****");
    }

    #[test]
    fn mask_exactly_8_chars() {
        let masked = crypto::mask_api_key("12345678");
        assert_eq!(masked, "sk-****");
    }

    #[test]
    fn mask_9_chars() {
        let masked = crypto::mask_api_key("123456789");
        assert_eq!(masked, "1234567...6789");
    }

    #[test]
    fn parse_hex_key_valid() {
        let result = crypto::parse_hex_key(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn parse_hex_key_invalid_hex() {
        let result = crypto::parse_hex_key("not-valid-hex");
        assert!(result.is_err());
    }

    #[test]
    fn parse_hex_key_wrong_length() {
        let result = crypto::parse_hex_key("0123456789abcdef"); // only 16 hex = 8 bytes
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("32 bytes"));
    }

    #[test]
    fn encrypt_empty_string() {
        let key = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let (ciphertext, nonce) = crypto::encrypt_with_key(&key, "").unwrap();
        let decrypted = crypto::decrypt_with_key(&key, &ciphertext, &nonce).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn each_encryption_produces_different_nonce() {
        let key = crypto::parse_hex_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let (_, nonce1) = crypto::encrypt_with_key(&key, "same-input").unwrap();
        let (_, nonce2) = crypto::encrypt_with_key(&key, "same-input").unwrap();
        assert_ne!(nonce1, nonce2, "each encryption must use a unique nonce");
    }
}
