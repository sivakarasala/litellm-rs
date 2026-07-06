# Swagger try-out samples — litellm-rs

Base URL: https://litellm-rs-43r6l.ondigitalocean.app
Swagger UI: /api/swagger-ui/ — click **Authorize**, paste a virtual key (`sk-...`), then **Try it out** on any endpoint below.

> Use `"stream": false` in Swagger — it buffers SSE, so streaming looks broken there even when it isn't.

---

## POST /v1/chat/completions

```json
{
  "model": "gpt-4o-mini",
  "messages": [
    { "role": "system", "content": "You are a concise assistant." },
    { "role": "user", "content": "In one line: why do API gateways matter?" }
  ],
  "stream": false,
  "temperature": 0.7,
  "max_tokens": 100
}
```

## POST /v1/completions

```json
{
  "model": "gpt-3.5-turbo-instruct",
  "prompt": "Write a one-line haiku about rate limits:",
  "stream": false,
  "max_tokens": 60
}
```

## POST /v1/embeddings

```json
{
  "model": "text-embedding-3-small",
  "input": "litellm-rs: keys, budgets, limits, attribution"
}
```

Array input also works:

```json
{
  "model": "text-embedding-3-small",
  "input": ["first chunk to embed", "second chunk to embed"]
}
```

## POST /v1/responses

```json
{
  "model": "gpt-4o-mini",
  "input": "In one line: what does a budget cap protect against?",
  "max_output_tokens": 100
}
```

## GET /v1/models

No body — just Authorize and execute. Returns the models your key can reach.

---

## Demo beat 4 in Swagger

Re-run the chat completion body above repeatedly (or bump `max_tokens` to 2000) until the key's budget cap trips. The response will be the **402** documented right on the page:

```json
{
  "error": {
    "message": "Budget exceeded for this key",
    "type": "insufficient_quota"
  }
}
```

---

# Baking these into Swagger UI (utoipa patch)

To have **Try it out pre-fill these examples** instead of auto-generated stubs, add an
`example` to each `request_body` in the `#[utoipa::path]` macros. utoipa re-exports
serde_json, so `use serde_json::json;` at the top of each handler file.

### src/proxy/chat_completions.rs

```rust
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    tag = "proxy",
    request_body(
        content = ChatCompletionRequest,
        example = json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": "You are a concise assistant."},
                {"role": "user", "content": "In one line: why do API gateways matter?"}
            ],
            "stream": false,
            "temperature": 0.7,
            "max_tokens": 100
        })
    ),
    responses(
        (status = 200, description = "Completion response", body = ChatCompletionResponse),
        (status = 401, description = "Invalid or missing API key", body = OpenAIError),
        (status = 402, description = "Budget exceeded", body = OpenAIError),
        (status = 429, description = "Rate limit exceeded", body = OpenAIError),
    ),
    security(("bearer_token" = []))
)]
```

### src/proxy/completions.rs

```rust
    request_body(
        content = CompletionRequest,
        example = json!({
            "model": "gpt-3.5-turbo-instruct",
            "prompt": "Write a one-line haiku about rate limits:",
            "stream": false,
            "max_tokens": 60
        })
    ),
```

### src/proxy/embeddings.rs

```rust
    request_body(
        content = EmbeddingRequest,
        example = json!({
            "model": "text-embedding-3-small",
            "input": "litellm-rs: keys, budgets, limits, attribution"
        })
    ),
```

### src/proxy/responses.rs

```rust
    request_body(
        content = Value,
        example = json!({
            "model": "gpt-4o-mini",
            "input": "In one line: what does a budget cap protect against?",
            "max_output_tokens": 100
        })
    ),
```

Notes:
- `json!` needs `use serde_json::json;` in each file (already a transitive dep; add to
  imports under the ssr feature gate if your handlers are feature-gated).
- Because `ChatCompletionRequest` flattens unknown fields into `extra`, the examples can
  include `max_tokens` etc. even though they aren't named struct fields — they pass
  through to the provider unchanged, which is the passthrough design working as intended.
- Rebuild and the examples appear in both the schema panel and the Try it out editor.
