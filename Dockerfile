# Stage 1: Build
FROM rustlang/rust:nightly-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install cargo-leptos

# Install dart-sass
RUN curl -fsSL -L https://github.com/sass/dart-sass/releases/download/1.86.3/dart-sass-1.86.3-linux-x64.tar.gz \
    | tar xz -C /usr/local/bin --strip-components=1 dart-sass/sass

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY .sqlx .sqlx/
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "pub fn hydrate() {}" > src/lib.rs
RUN SQLX_OFFLINE=true cargo leptos build --release 2>/dev/null || true

# Copy actual source
COPY . .
RUN touch src/main.rs src/lib.rs
ENV SQLX_OFFLINE=true
RUN cargo leptos build --release

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/litellm-rs ./
COPY --from=builder /app/target/site ./target/site
COPY --from=builder /app/migrations ./migrations
COPY configuration configuration

ENV APP_ENVIRONMENT=production
ENV LEPTOS_SITE_ADDR=0.0.0.0:3000
ENV LEPTOS_SITE_ROOT=target/site

EXPOSE 3000

CMD ["./litellm-rs"]
