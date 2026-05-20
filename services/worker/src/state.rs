use bridge_core::config::AppConfig;
use sqlx::PgPool;

#[derive(Clone)]
pub struct WorkerState {
    pub pool: PgPool,
    pub worker_id: String,
    pub config: AppConfig,
}
