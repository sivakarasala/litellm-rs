use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use super::client::{build_upstream_headers, extract_bearer_token, http_client, resolve_key};
use super::token_counter::calculate_cost;
use super::types::{EmbeddingRequest, EmbeddingResponse, OpenAIError};
use super::usage::{record_usage, UsageRecord};

/// POST /v1/embeddings — embeddings proxy.
#[utoipa::path(
    post,
    path = "/v1/embeddings",
    tag = "proxy",
    request_body = EmbeddingRequest,
    responses(
        (status = 200, description = "Embedding response", body = EmbeddingResponse),
        (status = 401, description = "Invalid or missing API key", body = OpenAIError),
        (status = 402, description = "Budget exceeded", body = OpenAIError),
        (status = 429, description = "Rate limit exceeded", body = OpenAIError),
    ),
    security(("bearer_token" = []))
)]
pub async fn embeddings(
    headers: HeaderMap,
    Json(body): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, (StatusCode, Json<OpenAIError>)> {
    let bearer = extract_bearer_token(&headers)
        .map_err(|(status, msg)| (status, Json(OpenAIError::new(msg, "invalid_request_error"))))?;

    let resolved = resolve_key(&bearer).await.map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(OpenAIError::new(e.to_string(), "invalid_request_error")),
        )
    })?;

    // Check model allowlist
    if let Some(ref allowed) = resolved.allowed_models {
        if !allowed.iter().any(|m| m == &body.model) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(OpenAIError::new(
                    format!("Model '{}' is not allowed for this key", body.model),
                    "invalid_request_error",
                )),
            ));
        }
    }

    // Check rate limits
    if let Some(rpm_limit) = resolved.rpm_limit {
        if let Err(msg) =
            super::rate_limit::rate_limiter().check_rpm(resolved.virtual_key_id, rpm_limit)
        {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(OpenAIError::new(msg, "rate_limit_error")),
            ));
        }
    }

    // Check budget
    if let Some(max_budget) = resolved.max_budget_usd {
        if let Err(e) = super::budget::check_budget(resolved.virtual_key_id, max_budget).await {
            return Err((
                StatusCode::PAYMENT_REQUIRED,
                Json(OpenAIError::new(e.to_string(), "budget_exceeded")),
            ));
        }
    }

    let url = format!("{}/v1/embeddings", resolved.base_url);
    let upstream_headers = build_upstream_headers(&headers, &resolved.api_key);

    let start = std::time::Instant::now();

    let response = http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
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

    let latency_ms = start.elapsed().as_millis() as i32;
    let status_code = response.status().as_u16() as i32;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        let openai_error: OpenAIError = serde_json::from_str(&error_body)
            .unwrap_or_else(|_| OpenAIError::new(error_body, "upstream_error"));
        return Err((
            StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(openai_error),
        ));
    }

    let resp: EmbeddingResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(OpenAIError::new(
                format!("Failed to parse upstream response: {}", e),
                "server_error",
            )),
        )
    })?;

    // Extract usage (embeddings have no output tokens)
    let (input_tokens, total_tokens) = match &resp.usage {
        Some(usage) => (usage.prompt_tokens, usage.total_tokens),
        None => (0, 0),
    };

    let cost = calculate_cost(&body.model, input_tokens, 0);

    tokio::spawn(async move {
        let _ = record_usage(UsageRecord {
            virtual_key_id: resolved.virtual_key_id,
            provider_key_id: resolved.provider_key_id,
            model: body.model,
            endpoint: "/v1/embeddings".to_string(),
            input_tokens: input_tokens as i32,
            output_tokens: 0,
            total_tokens: total_tokens as i32,
            cost_usd: cost,
            cached: false,
            status_code,
            latency_ms,
        })
        .await;
    });

    Ok(Json(resp))
}
