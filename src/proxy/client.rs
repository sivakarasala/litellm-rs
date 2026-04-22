use axum::http::StatusCode;
use reqwest::Client;
use std::sync::OnceLock;

use crate::error::AppError;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get or create a shared reqwest client for upstream API calls.
pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Allowed headers to forward to upstream (whitelist).
const FORWARD_HEADERS: &[&str] = &[
    "content-type",
    "accept",
    "openai-organization",
    "openai-project",
];

/// Build headers for upstream request: whitelist client headers + set auth.
pub fn build_upstream_headers(
    client_headers: &axum::http::HeaderMap,
    api_key: &str,
) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();

    for &name in FORWARD_HEADERS {
        if let Some(value) = client_headers.get(name) {
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
                {
                    headers.insert(header_name, header_value);
                }
            }
        }
    }

    // Always set Content-Type if not present
    headers
        .entry("content-type")
        .or_insert(reqwest::header::HeaderValue::from_static(
            "application/json",
        ));

    // Set Authorization with the real provider key
    if let Ok(auth_value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key)) {
        headers.insert("authorization", auth_value);
    }

    headers
}

/// Extract the bearer token from the Authorization header.
pub fn extract_bearer_token(
    headers: &axum::http::HeaderMap,
) -> Result<String, (StatusCode, String)> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header".to_string(),
            )
        })?;

    if !auth_header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format".to_string(),
        ));
    }

    Ok(auth_header[7..].to_string())
}

/// Resolved key context after validating the virtual key.
pub struct ResolvedKey {
    pub virtual_key_id: uuid::Uuid,
    pub provider_key_id: uuid::Uuid,
    pub api_key: String,
    pub base_url: String,
    pub allowed_models: Option<Vec<String>>,
    pub rpm_limit: Option<i32>,
    pub tpm_limit: Option<i32>,
    pub max_budget_usd: Option<rust_decimal::Decimal>,
}

/// Validate a virtual key and resolve the provider key for upstream calls.
pub async fn resolve_key(bearer_token: &str) -> Result<ResolvedKey, AppError> {
    use crate::keys::provider_keys::crypto;
    use crate::keys::virtual_keys::keygen;

    let pool = crate::db::db().await?;

    // Fetch all active virtual keys and check against the bearer token
    let keys = sqlx::query!(
        r#"SELECT vk.id, vk.key_hash, vk.provider_key_id, vk.is_active,
                  vk.expires_at, vk.allowed_models,
                  vk.rpm_limit, vk.tpm_limit, vk.max_budget_usd,
                  pk.api_key_encrypted, pk.api_key_nonce, pk.base_url, pk.is_active as provider_active
           FROM virtual_keys vk
           JOIN provider_keys pk ON vk.provider_key_id = pk.id
           WHERE vk.is_active = true"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e: sqlx::Error| AppError::Internal(e.to_string()))?;

    for key in keys {
        if keygen::verify_key(bearer_token, &key.key_hash)? {
            // Check expiry
            if let Some(expires_at) = key.expires_at {
                let now_str = chrono::Utc::now().to_rfc3339();
                if expires_at.to_string() < now_str {
                    return Err(AppError::Validation("API key has expired".into()));
                }
            }

            // Check provider key is active
            if !key.provider_active {
                return Err(AppError::Internal("Provider key is inactive".into()));
            }

            // Decrypt the provider API key
            let api_key = crypto::decrypt_api_key(&key.api_key_encrypted, &key.api_key_nonce)?;

            return Ok(ResolvedKey {
                virtual_key_id: key.id,
                provider_key_id: key.provider_key_id,
                api_key,
                base_url: key.base_url,
                allowed_models: key.allowed_models,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
                max_budget_usd: key.max_budget_usd,
            });
        }
    }

    Err(AppError::Validation("Invalid API key".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_headers_sets_auth() {
        let client_headers = axum::http::HeaderMap::new();
        let headers = build_upstream_headers(&client_headers, "sk-test-123");
        assert_eq!(
            headers.get("authorization").unwrap().to_str().unwrap(),
            "Bearer sk-test-123"
        );
    }

    #[test]
    fn build_headers_forwards_content_type() {
        let mut client_headers = axum::http::HeaderMap::new();
        client_headers.insert("content-type", "application/json".parse().unwrap());
        let headers = build_upstream_headers(&client_headers, "sk-test");
        assert_eq!(
            headers.get("content-type").unwrap().to_str().unwrap(),
            "application/json"
        );
    }

    #[test]
    fn build_headers_strips_unknown() {
        let mut client_headers = axum::http::HeaderMap::new();
        client_headers.insert("x-custom-header", "value".parse().unwrap());
        client_headers.insert("cookie", "session=abc".parse().unwrap());
        let headers = build_upstream_headers(&client_headers, "sk-test");
        assert!(headers.get("x-custom-header").is_none());
        assert!(headers.get("cookie").is_none());
    }

    #[test]
    fn extract_bearer_valid() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("authorization", "Bearer sk-litellm-abc".parse().unwrap());
        let token = extract_bearer_token(&headers).unwrap();
        assert_eq!(token, "sk-litellm-abc");
    }

    #[test]
    fn extract_bearer_missing() {
        let headers = axum::http::HeaderMap::new();
        let err = extract_bearer_token(&headers).unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn extract_bearer_wrong_format() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("authorization", "Basic abc123".parse().unwrap());
        let err = extract_bearer_token(&headers).unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
    }
}
