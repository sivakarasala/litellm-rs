# litellm-rs

A lightweight LLM proxy server built in Rust. Drop-in replacement for OpenAI API with virtual API key management, usage tracking, rate limiting, and budget controls.

Built with Leptos 0.8 + Axum 0.8 + SQLx + PostgreSQL.

## Quick Start

```bash
# Prerequisites
cargo install cargo-leptos
rustup target add wasm32-unknown-unknown

# Clone and setup
git clone https://github.com/sivakarasala/litellm-rs.git && cd litellm-rs
./scripts/init_db.sh

# Run
cargo leptos watch
# Dashboard: http://localhost:3000
# Proxy:     http://localhost:3000/v1
```

First visit: create your admin account at `http://localhost:3000`, add an OpenAI API key in Settings, create a virtual key, and start proxying.

## Features

- **OpenAI-compatible proxy** — `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/models`
- **Virtual API keys** — clients use `sk-litellm-xxx`, server maps to real provider keys
- **Streaming support** — SSE passthrough for chat completions with usage tracking
- **Usage tracking** — input/output tokens, cost per request, per-model pricing
- **Rate limiting** — RPM/TPM per virtual key (in-memory sliding window)
- **Budget enforcement** — per-key spend caps with automatic rejection
- **Self-service tokens** — public `/request-token` page for approved emails
- **Dashboard** — admin UI for keys, usage, audit log, and settings
- **Provider key encryption** — AES-256-GCM at rest

## Usage

Change `base_url` in any OpenAI SDK to point at litellm-rs. Zero code changes needed beyond that.

### Python

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-litellm-your-virtual-key",
    base_url="http://localhost:3000/v1"
)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}]
)
print(response.choices[0].message.content)

# Streaming
stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
    stream=True
)
for chunk in stream:
    print(chunk.choices[0].delta.content or "", end="")

# Embeddings
embedding = client.embeddings.create(
    model="text-embedding-3-small",
    input="The quick brown fox"
)
print(embedding.data[0].embedding[:5])
```

### Node.js

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  apiKey: 'sk-litellm-your-virtual-key',
  baseURL: 'http://localhost:3000/v1',
});

const response = await client.chat.completions.create({
  model: 'gpt-4o',
  messages: [{ role: 'user', content: 'Hello!' }],
});
console.log(response.choices[0].message.content);
```

### curl

```bash
curl http://localhost:3000/v1/chat/completions \
  -H "Authorization: Bearer sk-litellm-your-virtual-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

### Rust (async-openai)

```rust
use async_openai::{Client, config::OpenAIConfig};

let config = OpenAIConfig::new()
    .with_api_key("sk-litellm-your-virtual-key")
    .with_api_base("http://localhost:3000/v1");

let client = Client::with_config(config);
```

## API Endpoints

### Proxy (auth: `Authorization: Bearer sk-litellm-xxx`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | Chat completions (streaming + non-streaming) |
| POST | `/v1/completions` | Legacy completions |
| POST | `/v1/embeddings` | Text embeddings |
| GET | `/v1/models` | List available models |

### Management (auth: session cookie)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/health_check` | Health check |

### Public (no auth)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/request-token` | Self-service token request form |

### Swagger UI

Interactive API docs at [http://localhost:3000/api/swagger-ui](http://localhost:3000/api/swagger-ui).

## Development

### Prerequisites

- Rust nightly: `rustup default nightly`
- WASM target: `rustup target add wasm32-unknown-unknown`
- cargo-leptos: `cargo install cargo-leptos`
- Dart Sass: [install guide](https://sass-lang.com/install/)
- PostgreSQL 15+
- sqlx-cli: `cargo install sqlx-cli --no-default-features --features rustls,postgres`

### Database Setup

```bash
./scripts/init_db.sh
# Or if Postgres is already running:
SKIP_DOCKER=true ./scripts/init_db.sh
```

### Run

```bash
cargo leptos watch
```

### Git Hooks

```bash
./scripts/setup-hooks.sh
```

Installs pre-commit: `cargo fmt --check` + `cargo clippy` + `cargo sqlx prepare`.

### Tests

```bash
cargo test --features ssr
```

## Docker

### With Docker Compose (includes PostgreSQL)

> Stop local PostgreSQL first if running — it will conflict on port 5432.

```bash
docker compose up
```

### Standalone (connect to existing DB)

```bash
docker build -t litellm-rs .
docker run -p 3000:3000 --env-file .env litellm-rs
```

Migrations run automatically on startup.

## Deployment

### DigitalOcean App Platform

```bash
doctl apps create --spec spec.yaml
```

Set environment variables in the DO dashboard.

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `APP_ENVIRONMENT` | Prod | Set to `production` |
| `APP_APPLICATION__HOST` | Prod | Bind address (`0.0.0.0`) |
| `APP_APPLICATION__PORT` | Prod | Server port (`3000`) |
| `APP_DATABASE__HOST` | Yes | PostgreSQL host |
| `APP_DATABASE__PORT` | Yes | PostgreSQL port |
| `APP_DATABASE__USERNAME` | Yes | Database user |
| `APP_DATABASE__PASSWORD` | Yes | Database password |
| `APP_DATABASE__DATABASE_NAME` | Yes | Database name |
| `APP_DATABASE__REQUIRE_SSL` | Prod | Require SSL (`true`) |
| `APP_ENCRYPTION__MASTER_KEY` | Yes | 64-char hex for AES-256-GCM |

Generate an encryption key:

```bash
openssl rand -hex 32
```

## Project Structure

```
src/
├── main.rs                    # Axum server bootstrap
├── lib.rs                     # Crate root + WASM hydration
├── app.rs                     # Leptos router, sidebar nav
├── configuration.rs           # YAML settings
├── db.rs                      # PgPool + UUID newtypes
├── error.rs                   # AppError enum
├── auth/                      # Session auth (argon2)
├── proxy/                     # OpenAI-compatible proxy
│   ├── chat_completions.rs    # Streaming + non-streaming
│   ├── completions.rs         # Legacy completions
│   ├── embeddings.rs          # Embeddings proxy
│   ├── models.rs              # List models
│   ├── client.rs              # reqwest client, key resolution
│   ├── types.rs               # OpenAI types with serde(flatten)
│   ├── token_counter.rs       # Model pricing + cost calc
│   ├── rate_limit.rs          # In-memory sliding window
│   ├── budget.rs              # Budget enforcement
│   └── usage.rs               # Usage recording
├── keys/                      # Key management
│   ├── virtual_keys.rs        # sk-litellm-xxx generation
│   ├── provider_keys.rs       # AES-256-GCM encryption
│   └── approved_emails.rs     # Self-service whitelist
├── pages/                     # Leptos UI pages
│   ├── dashboard/             # Stats + recent requests
│   ├── keys/                  # Virtual key CRUD
│   ├── usage/                 # Usage log table
│   ├── audit/                 # Audit log table
│   ├── settings/              # Provider keys + approved emails
│   ├── request_token/         # Public token request form
│   └── login/                 # Password login
└── routes/                    # Health check, Swagger
```

## Architecture

- **Virtual keys**: Argon2-hashed, never stored in plaintext
- **Provider keys**: AES-256-GCM encrypted at rest, master key in env
- **Rate limiting**: In-memory sliding window (resets on restart)
- **Proxy types**: `#[serde(flatten)] extra: Value` for OpenAI forward-compatibility
- **Streaming**: SSE passthrough with `stream_options: {include_usage: true}` injection for accurate token counting

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Connection refused on 5432 | Start PostgreSQL: `./scripts/init_db.sh` |
| Port 5432 in use (Docker) | Stop local Postgres: `brew services stop postgresql@16` |
| Missing WASM target | `rustup target add wasm32-unknown-unknown` |
| Sass not found | `npm install -g sass` |
| ENCRYPTION_KEY error | `openssl rand -hex 32` → set `APP_ENCRYPTION__MASTER_KEY` in `.env` |
| No active provider keys (503) | Add OpenAI key in Settings |
| Invalid API key (401) | Check key is active and not expired |

## License

MIT
