#![recursion_limit = "512"]

mod app;
mod dto;
mod error;
mod repositories;
mod routes;
mod schema;
mod services;
mod state;

use std::{env, net::SocketAddr, sync::Arc};

use app::build_router;
use bridge_core::config::AppConfig;
use schema::migrate_and_backfill;
use sqlx::postgres::PgPoolOptions;
use services::rate_limit::RateLimiter;
use state::AppState;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "issuehub_api=info,tower_http=info".into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://issue_bridge:issue_bridge@localhost:5432/issue_bridge".to_string());
    let port = env::var("API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);

    let config = AppConfig::from_env();
    error::set_debug_error_responses(config.debug);

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    migrate_and_backfill(&pool, &config).await?;

    let state = AppState {
        pool: Arc::new(pool),
        config,
        rate_limiter: RateLimiter::default(),
    };

    let app = build_router(state);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    info!("api listening on {}", address);

    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
