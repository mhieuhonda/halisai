mod ai;
mod auth;
mod config;
mod db;
mod error;
mod i18n;
mod routes;
mod state;
mod tools;
mod usage;
mod util;

use std::sync::Arc;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::ai::AiClient;
use crate::state::{AppState, RateLimiter};
use crate::tools::Tools;

/// Doc file .env (khong ghi de bien da co trong moi truong).
fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            if !k.is_empty() && std::env::var(k).is_err() {
                std::env::set_var(k, v);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "halisai=info,tower_http=info".into()),
        )
        .init();

    let cfg = Arc::new(config::Config::from_env());
    if cfg.google_client_id.is_empty() || cfg.google_client_secret.is_empty() {
        tracing::warn!("GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET chua duoc cau hinh!");
    }

    let pool = db::init_pool(&cfg.database_url).await;
    tracing::info!("database san sang");

    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("halisai/0.1")
        .build()
        .expect("tao http client");

    let ai = Arc::new(AiClient::new(&cfg.ai_base_url, &cfg.ai_model, cfg.ai_max_output_tokens));
    let tools = Arc::new(Tools::new());

    let state = AppState {
        pool,
        cfg: cfg.clone(),
        http,
        ai,
        tools,
        rl: Arc::new(RateLimiter::new()),
    };

    let csp = "default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; \
               script-src 'self' 'unsafe-eval'; connect-src 'self'; font-src 'self'; \
               frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

    let app = routes::router(state)
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(csp),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("referrer-policy"),
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .unwrap_or_else(|e| panic!("khong mo duoc cong {}: {e}", cfg.bind_addr));
    tracing::info!("halisai chay tai http://{}", cfg.bind_addr);
    axum::serve(listener, app).await.expect("server loi");
}
