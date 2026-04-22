use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use super::client::{build_upstream_headers, extract_bearer_token, http_client, resolve_key};
use super::types::{ModelsResponse, OpenAIError};

/// GET /v1/models — list available models from upstream.
pub async fn list_models(
    headers: HeaderMap,
) -> Result<Json<ModelsResponse>, (StatusCode, Json<OpenAIError>)> {
    let bearer = extract_bearer_token(&headers)
        .map_err(|(status, msg)| (status, Json(OpenAIError::new(msg, "invalid_request_error"))))?;

    let resolved = resolve_key(&bearer).await.map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(OpenAIError::new(e.to_string(), "invalid_request_error")),
        )
    })?;

    let url = format!("{}/v1/models", resolved.base_url);
    let upstream_headers = build_upstream_headers(&headers, &resolved.api_key);

    let response = http_client()
        .get(&url)
        .headers(upstream_headers)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(OpenAIError::new(
                    format!("Upstream request failed: {}", e),
                    "server_error",
                )),
            )
        })?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        let openai_error: OpenAIError = serde_json::from_str(&error_body)
            .unwrap_or_else(|_| OpenAIError::new(error_body, "upstream_error"));
        return Err((StatusCode::BAD_GATEWAY, Json(openai_error)));
    }

    let resp: ModelsResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(OpenAIError::new(
                format!("Failed to parse upstream response: {}", e),
                "server_error",
            )),
        )
    })?;

    Ok(Json(resp))
}
