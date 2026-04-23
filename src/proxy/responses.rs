use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use serde_json::Value;
use tokio_stream::wrappers::ReceiverStream;

use super::client::{build_upstream_headers, extract_bearer_token, http_client, resolve_key};
use super::token_counter::calculate_cost;
use super::types::OpenAIError;
use super::usage::{record_usage, UsageRecord};

/// POST /v1/responses — OpenAI Responses API proxy (streaming + non-streaming).
#[utoipa::path(
    post,
    path = "/v1/responses",
    tag = "proxy",
    request_body = Value,
    responses(
        (status = 200, description = "Response", body = Value),
        (status = 401, description = "Invalid or missing API key", body = OpenAIError),
        (status = 402, description = "Budget exceeded", body = OpenAIError),
        (status = 429, description = "Rate limit exceeded", body = OpenAIError),
    ),
    security(("bearer_token" = []))
)]
pub async fn responses(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let bearer = match extract_bearer_token(&headers) {
        Ok(b) => b,
        Err((status, msg)) => {
            return error_response(status, &msg, "invalid_request_error");
        }
    };

    let resolved = match resolve_key(&bearer).await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                &e.to_string(),
                "invalid_request_error",
            );
        }
    };

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(ref allowed) = resolved.allowed_models {
        if !allowed.iter().any(|m| m == &model) {
            return error_response(
                StatusCode::FORBIDDEN,
                &format!("Model '{}' is not allowed for this key", model),
                "invalid_request_error",
            );
        }
    }

    if let Some(rpm_limit) = resolved.rpm_limit {
        if let Err(msg) =
            super::rate_limit::rate_limiter().check_rpm(resolved.virtual_key_id, rpm_limit)
        {
            return error_response(StatusCode::TOO_MANY_REQUESTS, &msg, "rate_limit_error");
        }
    }

    if let Some(max_budget) = resolved.max_budget_usd {
        if let Err(e) = super::budget::check_budget(resolved.virtual_key_id, max_budget).await {
            return error_response(
                StatusCode::PAYMENT_REQUIRED,
                &e.to_string(),
                "budget_exceeded",
            );
        }
    }

    let is_streaming = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_streaming {
        handle_streaming(headers, body, model, resolved).await
    } else {
        handle_non_streaming(headers, body, model, resolved).await
    }
}

async fn handle_non_streaming(
    headers: HeaderMap,
    body: Value,
    model: String,
    resolved: super::client::ResolvedKey,
) -> Response {
    let url = format!("{}/v1/responses", resolved.base_url);
    let upstream_headers = build_upstream_headers(&headers, &resolved.api_key);
    let start = std::time::Instant::now();

    let response = match http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Upstream request failed: {}", e),
                "server_error",
            );
        }
    };

    let latency_ms = start.elapsed().as_millis() as i32;
    let status_code = response.status().as_u16() as i32;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        let openai_error: OpenAIError = serde_json::from_str(&error_body)
            .unwrap_or_else(|_| OpenAIError::new(error_body, "upstream_error"));
        return (
            StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(openai_error),
        )
            .into_response();
    }

    let resp: Value = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Failed to parse upstream response: {}", e),
                "server_error",
            );
        }
    };

    let (input_tokens, output_tokens, total_tokens) = extract_usage(&resp);
    let cost = calculate_cost(&model, input_tokens, output_tokens);

    tokio::spawn(async move {
        let _ = record_usage(UsageRecord {
            virtual_key_id: resolved.virtual_key_id,
            provider_key_id: resolved.provider_key_id,
            model,
            endpoint: "/v1/responses".to_string(),
            input_tokens: input_tokens as i32,
            output_tokens: output_tokens as i32,
            total_tokens: total_tokens as i32,
            cost_usd: cost,
            cached: false,
            status_code,
            latency_ms,
        })
        .await;
    });

    Json(resp).into_response()
}

async fn handle_streaming(
    headers: HeaderMap,
    body: Value,
    model: String,
    resolved: super::client::ResolvedKey,
) -> Response {
    let url = format!("{}/v1/responses", resolved.base_url);
    let upstream_headers = build_upstream_headers(&headers, &resolved.api_key);
    let start = std::time::Instant::now();

    let response = match http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Upstream request failed: {}", e),
                "server_error",
            );
        }
    };

    if !response.status().is_success() {
        let status_code = response.status().as_u16();
        let error_body = response.text().await.unwrap_or_default();
        let openai_error: OpenAIError = serde_json::from_str(&error_body)
            .unwrap_or_else(|_| OpenAIError::new(error_body, "upstream_error"));
        return (
            StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(openai_error),
        )
            .into_response();
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::io::Error>>(32);
    let vk_id = resolved.virtual_key_id;
    let pk_id = resolved.provider_key_id;

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut usage_data: (u32, u32, u32) = (0, 0, 0);

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = match chunk_result {
                Ok(bytes) => bytes,
                Err(_) => break,
            };

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(pos) = buffer.find("\n\n") {
                let event = buffer[..pos + 2].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in event.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                            if parsed.get("type").and_then(|t| t.as_str())
                                == Some("response.completed")
                            {
                                if let Some(resp) = parsed.get("response") {
                                    usage_data = extract_usage(resp);
                                }
                            }
                        }
                    }
                }

                if tx.send(Ok(event)).await.is_err() {
                    break;
                }
            }
        }

        if !buffer.is_empty() {
            let _ = tx.send(Ok(buffer)).await;
        }

        let latency_ms = start.elapsed().as_millis() as i32;
        let (input_tokens, output_tokens, total_tokens) = usage_data;
        let cost = calculate_cost(&model, input_tokens, output_tokens);

        let _ = record_usage(UsageRecord {
            virtual_key_id: vk_id,
            provider_key_id: pk_id,
            model,
            endpoint: "/v1/responses".to_string(),
            input_tokens: input_tokens as i32,
            output_tokens: output_tokens as i32,
            total_tokens: total_tokens as i32,
            cost_usd: cost,
            cached: false,
            status_code: 200,
            latency_ms,
        })
        .await;
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(body)
        .unwrap_or_else(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build streaming response",
                "server_error",
            )
        })
}

/// Extract (input_tokens, output_tokens, total_tokens) from a Responses API JSON object.
fn extract_usage(value: &Value) -> (u32, u32, u32) {
    if let Some(usage) = value.get("usage") {
        let input = usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let output = usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let total = usage
            .get("total_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(input as u64 + output as u64) as u32;
        (input, output, total)
    } else {
        (0, 0, 0)
    }
}

fn error_response(status: StatusCode, message: &str, error_type: &str) -> Response {
    let error = OpenAIError::new(message, error_type);
    (status, Json(error)).into_response()
}
