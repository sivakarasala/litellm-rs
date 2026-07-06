use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use serde_json::Value;
use tokio_stream::wrappers::ReceiverStream;

use super::client::{extract_bearer_token, resolve_key, streaming_http_client, ResolvedKey};
use super::token_counter::calculate_cost_with_cache;
use super::usage::{record_usage, UsageRecord};

/// Default Anthropic API version sent when the client does not provide one.
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Token usage reported by the Anthropic Messages API.
///
/// Cache tokens are billed at different rates than regular input tokens
/// (cache writes at 1.25x, cache reads at 0.1x), so they are tracked
/// separately for cost calculation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

impl AnthropicUsage {
    pub fn total_input(&self) -> u32 {
        self.input_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }

    pub fn total(&self) -> u32 {
        self.total_input() + self.output_tokens
    }
}

/// POST /v1/messages — Anthropic Messages API passthrough (streaming + non-streaming).
///
/// The request body is forwarded as-is (parsed only to `serde_json::Value`)
/// so Anthropic-specific fields like `cache_control` breakpoints, thinking
/// configuration, and tool definitions survive untouched.
#[utoipa::path(
    post,
    path = "/v1/messages",
    tag = "proxy",
    responses(
        (status = 200, description = "Anthropic Messages API response (passthrough)"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 402, description = "Budget exceeded"),
        (status = 429, description = "Rate limit exceeded"),
    ),
    security(("bearer_token" = []))
)]
pub async fn messages(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let resolved = match authorize(&headers, &body).await {
        Ok(r) => r,
        Err(resp) => return *resp,
    };

    // Check rate limits
    if let Some(rpm_limit) = resolved.rpm_limit {
        if let Err(msg) =
            super::rate_limit::rate_limiter().check_rpm(resolved.virtual_key_id, rpm_limit)
        {
            return anthropic_error(StatusCode::TOO_MANY_REQUESTS, "rate_limit_error", &msg);
        }
    }

    // Check budget
    if let Some(max_budget) = resolved.max_budget_usd {
        if let Err(e) = super::budget::check_budget(resolved.virtual_key_id, max_budget).await {
            return anthropic_error(
                StatusCode::PAYMENT_REQUIRED,
                "invalid_request_error",
                &e.to_string(),
            );
        }
    }

    let is_streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);

    if is_streaming {
        handle_streaming(headers, body, resolved).await
    } else {
        handle_non_streaming(headers, body, resolved).await
    }
}

/// POST /v1/messages/count_tokens — Anthropic token counting passthrough.
///
/// This endpoint is free on the Anthropic API, so no usage is recorded,
/// but the virtual key is still validated. Claude Code calls this
/// frequently for context-window management, so it must not 404.
#[utoipa::path(
    post,
    path = "/v1/messages/count_tokens",
    tag = "proxy",
    responses(
        (status = 200, description = "Token count response (passthrough)"),
        (status = 401, description = "Invalid or missing API key"),
    ),
    security(("bearer_token" = []))
)]
pub async fn count_tokens(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let resolved = match authorize(&headers, &body).await {
        Ok(r) => r,
        Err(resp) => return *resp,
    };

    let url = format!("{}/v1/messages/count_tokens", resolved.base_url);
    let upstream_headers = build_anthropic_headers(&headers, &resolved.api_key);

    let response = match streaming_http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return anthropic_error(
                StatusCode::BAD_GATEWAY,
                "api_error",
                &format!("Upstream request failed: {}", e),
            );
        }
    };

    passthrough_body(response).await
}

/// Shared auth + model-allowlist gate for both Anthropic endpoints.
async fn authorize(headers: &HeaderMap, body: &Value) -> Result<ResolvedKey, Box<Response>> {
    let client_key = match extract_client_key(headers) {
        Ok(k) => k,
        Err((status, msg)) => {
            return Err(Box::new(anthropic_error(
                status,
                "authentication_error",
                &msg,
            )));
        }
    };

    let resolved = match resolve_key(&client_key).await {
        Ok(r) => r,
        Err(e) => {
            return Err(Box::new(anthropic_error(
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                &e.to_string(),
            )));
        }
    };

    if let Some(ref allowed) = resolved.allowed_models {
        let model = body.get("model").and_then(Value::as_str).unwrap_or("");
        if !allowed.iter().any(|m| m == model) {
            return Err(Box::new(anthropic_error(
                StatusCode::FORBIDDEN,
                "permission_error",
                &format!("Model '{}' is not allowed for this key", model),
            )));
        }
    }

    Ok(resolved)
}

async fn handle_non_streaming(headers: HeaderMap, body: Value, resolved: ResolvedKey) -> Response {
    let url = format!("{}/v1/messages", resolved.base_url);
    let upstream_headers = build_anthropic_headers(&headers, &resolved.api_key);
    let start = std::time::Instant::now();

    let response = match streaming_http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return anthropic_error(
                StatusCode::BAD_GATEWAY,
                "api_error",
                &format!("Upstream request failed: {}", e),
            );
        }
    };

    let latency_ms = start.elapsed().as_millis() as i32;
    let status = response.status();
    let status_code = status.as_u16() as i32;

    if !status.is_success() {
        // Pass the Anthropic error body through verbatim; clients like
        // Claude Code parse the Anthropic error shape.
        return passthrough_body(response).await;
    }

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return anthropic_error(
                StatusCode::BAD_GATEWAY,
                "api_error",
                &format!("Failed to read upstream response: {}", e),
            );
        }
    };

    let usage = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.get("usage").map(usage_from_value))
        .unwrap_or_default();

    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    spawn_record_usage(resolved, model, usage, status_code, latency_ms);

    // Return the upstream bytes untouched.
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "api_error",
                "Failed to build response",
            )
        })
}

async fn handle_streaming(headers: HeaderMap, body: Value, resolved: ResolvedKey) -> Response {
    let url = format!("{}/v1/messages", resolved.base_url);
    let upstream_headers = build_anthropic_headers(&headers, &resolved.api_key);
    let start = std::time::Instant::now();

    let response = match streaming_http_client()
        .post(&url)
        .headers(upstream_headers)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return anthropic_error(
                StatusCode::BAD_GATEWAY,
                "api_error",
                &format!("Upstream request failed: {}", e),
            );
        }
    };

    if !response.status().is_success() {
        return passthrough_body(response).await;
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::io::Error>>(32);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    tokio::spawn(async move {
        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut usage = AnthropicUsage::default();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = match chunk_result {
                Ok(bytes) => bytes,
                Err(_) => break,
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events
            while let Some(pos) = buffer.find("\n\n") {
                let event = buffer[..pos + 2].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in event.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            merge_sse_usage(&json, &mut usage);
                        }
                    }
                }

                if tx.send(Ok(event)).await.is_err() {
                    // Client disconnected
                    break;
                }
            }
        }

        // Send any remaining buffer
        if !buffer.is_empty() && tx.send(Ok(buffer)).await.is_err() {
            // Client disconnected
        }

        let latency_ms = start.elapsed().as_millis() as i32;
        spawn_record_usage(resolved, model, usage, 200, latency_ms);
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
            anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "api_error",
                "Failed to build streaming response",
            )
        })
}

/// Record usage in the background using the shared usage_logs pipeline.
fn spawn_record_usage(
    resolved: ResolvedKey,
    model: String,
    usage: AnthropicUsage,
    status_code: i32,
    latency_ms: i32,
) {
    let cost = calculate_cost_with_cache(
        &model,
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_creation_input_tokens,
        usage.cache_read_input_tokens,
    );
    let cached = usage.cache_read_input_tokens > 0;

    tokio::spawn(async move {
        let _ = record_usage(UsageRecord {
            virtual_key_id: resolved.virtual_key_id,
            provider_key_id: resolved.provider_key_id,
            model,
            endpoint: "/v1/messages".to_string(),
            input_tokens: usage.total_input() as i32,
            output_tokens: usage.output_tokens as i32,
            total_tokens: usage.total() as i32,
            cost_usd: cost,
            cached,
            status_code,
            latency_ms,
        })
        .await;
    });
}

/// Extract the virtual key from either `Authorization: Bearer` (Claude Code
/// with ANTHROPIC_AUTH_TOKEN) or `x-api-key` (native Anthropic SDK style).
pub fn extract_client_key(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    if let Ok(bearer) = extract_bearer_token(headers) {
        return Ok(bearer);
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization or x-api-key header".to_string(),
            )
        })
}

/// Headers forwarded from the client to the Anthropic API.
const ANTHROPIC_FORWARD_HEADERS: &[&str] = &["accept", "anthropic-version", "anthropic-beta"];

/// Build headers for the upstream Anthropic request.
///
/// Auth is sent as `x-api-key` (not `Authorization: Bearer`), and the
/// inbound client auth headers are never forwarded. `anthropic-version`
/// defaults to a known-good version if the client omits it.
pub fn build_anthropic_headers(
    client_headers: &HeaderMap,
    api_key: &str,
) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();

    for &name in ANTHROPIC_FORWARD_HEADERS {
        if let Some(value) = client_headers.get(name) {
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
                {
                    headers.insert(header_name, header_value);
                }
            }
        }
    }

    headers.insert(
        "content-type",
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    headers
        .entry("anthropic-version")
        .or_insert(reqwest::header::HeaderValue::from_static(
            DEFAULT_ANTHROPIC_VERSION,
        ));

    if let Ok(key_value) = reqwest::header::HeaderValue::from_str(api_key) {
        headers.insert("x-api-key", key_value);
    }

    headers
}

/// Read Anthropic usage fields from a `usage` JSON object.
pub fn usage_from_value(usage: &Value) -> AnthropicUsage {
    let get = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0) as u32;
    AnthropicUsage {
        input_tokens: get("input_tokens"),
        output_tokens: get("output_tokens"),
        cache_creation_input_tokens: get("cache_creation_input_tokens"),
        cache_read_input_tokens: get("cache_read_input_tokens"),
    }
}

/// Merge usage from an Anthropic SSE event into the accumulator.
///
/// `message_start` carries input and cache token counts; `message_delta`
/// carries the cumulative output token count (overwrite, not add).
pub fn merge_sse_usage(event: &Value, acc: &mut AnthropicUsage) {
    match event.get("type").and_then(Value::as_str) {
        Some("message_start") => {
            if let Some(usage) = event.get("message").and_then(|m| m.get("usage")) {
                let u = usage_from_value(usage);
                acc.input_tokens = u.input_tokens;
                acc.cache_creation_input_tokens = u.cache_creation_input_tokens;
                acc.cache_read_input_tokens = u.cache_read_input_tokens;
                acc.output_tokens = acc.output_tokens.max(u.output_tokens);
            }
        }
        Some("message_delta") => {
            if let Some(usage) = event.get("usage") {
                if let Some(out) = usage.get("output_tokens").and_then(Value::as_u64) {
                    acc.output_tokens = out as u32;
                }
            }
        }
        _ => {}
    }
}

/// Forward an upstream response body and status verbatim.
async fn passthrough_body(response: reqwest::Response) -> Response {
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let bytes = response.bytes().await.unwrap_or_default();

    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            anthropic_error(
                StatusCode::BAD_GATEWAY,
                "api_error",
                "Failed to forward upstream response",
            )
        })
}

/// Build an Anthropic-format error response:
/// `{"type": "error", "error": {"type": ..., "message": ...}}`
fn anthropic_error(status: StatusCode, error_type: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": error_type,
            "message": message,
        }
    });
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_headers_use_x_api_key() {
        let client_headers = HeaderMap::new();
        let headers = build_anthropic_headers(&client_headers, "sk-ant-test");
        assert_eq!(
            headers.get("x-api-key").unwrap().to_str().unwrap(),
            "sk-ant-test"
        );
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn anthropic_headers_default_version() {
        let client_headers = HeaderMap::new();
        let headers = build_anthropic_headers(&client_headers, "sk-ant-test");
        assert_eq!(
            headers.get("anthropic-version").unwrap().to_str().unwrap(),
            DEFAULT_ANTHROPIC_VERSION
        );
    }

    #[test]
    fn anthropic_headers_forward_version_and_beta() {
        let mut client_headers = HeaderMap::new();
        client_headers.insert("anthropic-version", "2024-01-01".parse().unwrap());
        client_headers.insert(
            "anthropic-beta",
            "prompt-caching-2024-07-31".parse().unwrap(),
        );
        let headers = build_anthropic_headers(&client_headers, "sk-ant-test");
        assert_eq!(
            headers.get("anthropic-version").unwrap().to_str().unwrap(),
            "2024-01-01"
        );
        assert_eq!(
            headers.get("anthropic-beta").unwrap().to_str().unwrap(),
            "prompt-caching-2024-07-31"
        );
    }

    #[test]
    fn anthropic_headers_never_forward_client_auth() {
        let mut client_headers = HeaderMap::new();
        client_headers.insert(
            "authorization",
            "Bearer sk-litellm-virtual".parse().unwrap(),
        );
        client_headers.insert("x-api-key", "sk-litellm-virtual".parse().unwrap());
        let headers = build_anthropic_headers(&client_headers, "sk-ant-real");
        assert!(headers.get("authorization").is_none());
        assert_eq!(
            headers.get("x-api-key").unwrap().to_str().unwrap(),
            "sk-ant-real"
        );
    }

    #[test]
    fn extract_client_key_prefers_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer sk-litellm-abc".parse().unwrap());
        assert_eq!(extract_client_key(&headers).unwrap(), "sk-litellm-abc");
    }

    #[test]
    fn extract_client_key_falls_back_to_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "sk-litellm-xyz".parse().unwrap());
        assert_eq!(extract_client_key(&headers).unwrap(), "sk-litellm-xyz");
    }

    #[test]
    fn extract_client_key_missing() {
        let headers = HeaderMap::new();
        assert!(extract_client_key(&headers).is_err());
    }

    #[test]
    fn usage_parses_all_fields() {
        let v = serde_json::json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_creation_input_tokens": 2000,
            "cache_read_input_tokens": 8000
        });
        let u = usage_from_value(&v);
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.cache_creation_input_tokens, 2000);
        assert_eq!(u.cache_read_input_tokens, 8000);
        assert_eq!(u.total_input(), 10100);
        assert_eq!(u.total(), 10150);
    }

    #[test]
    fn sse_message_start_sets_input_and_cache() {
        let event = serde_json::json!({
            "type": "message_start",
            "message": {
                "usage": {
                    "input_tokens": 25,
                    "output_tokens": 1,
                    "cache_creation_input_tokens": 500,
                    "cache_read_input_tokens": 1500
                }
            }
        });
        let mut acc = AnthropicUsage::default();
        merge_sse_usage(&event, &mut acc);
        assert_eq!(acc.input_tokens, 25);
        assert_eq!(acc.cache_creation_input_tokens, 500);
        assert_eq!(acc.cache_read_input_tokens, 1500);
    }

    #[test]
    fn sse_message_delta_overwrites_output() {
        let mut acc = AnthropicUsage {
            input_tokens: 25,
            output_tokens: 1,
            ..Default::default()
        };
        let event = serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 320}
        });
        merge_sse_usage(&event, &mut acc);
        assert_eq!(acc.output_tokens, 320);
        assert_eq!(acc.input_tokens, 25);
    }

    #[test]
    fn sse_other_events_ignored() {
        let mut acc = AnthropicUsage::default();
        let event = serde_json::json!({"type": "content_block_delta", "delta": {"text": "hi"}});
        merge_sse_usage(&event, &mut acc);
        assert_eq!(acc, AnthropicUsage::default());
    }
}
