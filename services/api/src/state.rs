use std::sync::Arc;

use bridge_core::config::AppConfig;
use sqlx::PgPool;

use crate::services::rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<PgPool>,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
}
