#![recursion_limit = "1024"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::routing::get;
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use litellm_rs::app::*;
    use litellm_rs::configuration;
    use litellm_rs::routes::{health_check, ApiDoc};
    use litellm_rs::telemetry::{get_subscriber, init_subscriber};
    use sqlx::postgres::PgPoolOptions;
    use tower_http::cors::{Any, CorsLayer};
    use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
    use tower_http::trace::TraceLayer;
    use tower_sessions::cookie::SameSite;
    use tower_sessions::{Expiry, SessionManagerLayer};
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    dotenvy::dotenv().ok();

    let subscriber = get_subscriber("litellm_rs".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let app_config = configuration::get_configuration().expect("Failed to read configuration");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy_with(app_config.database.connection_options());

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Could not run database migrations");

    litellm_rs::db::init_pool(pool.clone());

    // Session store
    let session_store = tower_sessions_sqlx_store::PostgresStore::new(pool.clone());
    session_store
        .migrate()
        .await
        .expect("Could not run session store migration");

    let is_production = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_default()
        .eq_ignore_ascii_case("production");

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(is_production)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::hours(24),
        ));

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let api_routes = Router::new().route("/health_check", get(health_check));

    // Proxy routes (OpenAI-compatible)
    let proxy_routes = Router::new()
        .route(
            "/chat/completions",
            axum::routing::post(litellm_rs::proxy::chat_completions::chat_completions),
        )
        .route(
            "/completions",
            axum::routing::post(litellm_rs::proxy::completions::completions),
        )
        .route(
            "/embeddings",
            axum::routing::post(litellm_rs::proxy::embeddings::embeddings),
        )
        .route(
            "/responses",
            axum::routing::post(litellm_rs::proxy::responses::responses),
        )
        .route("/models", get(litellm_rs::proxy::models::list_models))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let app = Router::new()
        .nest("/v1", proxy_routes)
        .nest("/api/v1", api_routes)
        .merge(SwaggerUi::new("/api/swagger-ui").url("/api/openapi.json", ApiDoc::openapi()))
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let pool = pool.clone();
                move || provide_context(pool.clone())
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .layer(session_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("unknown");
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                        status = tracing::field::Empty,
                        latency_ms = tracing::field::Empty,
                    )
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     span: &tracing::Span| {
                        span.record("status", response.status().as_u16());
                        span.record("latency_ms", latency.as_millis() as u64);
                        tracing::info!("response");
                    },
                ),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .with_state(leptos_options);

    tracing::info!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
