use std::{env, time::Duration};

use anyhow::Context;
use bridge_core::config::AppConfig;
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    heartbeat::{spawn_heartbeat_loop, WorkerHeartbeatHandle},
    handlers::sync::{cleanup_expired_uploads, handle_job},
    queue::{fetch_next_job, mark_done, reschedule_failed},
    schema,
    state::WorkerState,
};

pub async fn run() -> anyhow::Result<()> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://issue_bridge:issue_bridge@localhost:5432/issue_bridge".to_string());
    let config = AppConfig::from_env();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    schema::migrate(&pool, &config).await?;

    let state = WorkerState {
        pool,
        worker_id: format!("worker-{}", Uuid::new_v4()),
        config,
    };

    info!("worker started as {}", state.worker_id);
    let mut last_upload_cleanup = std::time::Instant::now() - Duration::from_secs(3600);
    let heartbeat_interval = Duration::from_secs(state.config.worker_heartbeat_interval_seconds);
    let heartbeat = WorkerHeartbeatHandle::default();
    let _heartbeat_task = spawn_heartbeat_loop(
        state.pool.clone(),
        state.worker_id.clone(),
        heartbeat_interval,
        heartbeat.clone(),
    );
    let queue_stale_after_seconds = state.config.queue_stale_after_seconds.min(i32::MAX as u64) as i32;

    loop {
        if last_upload_cleanup.elapsed() >= Duration::from_secs(900) {
            match cleanup_expired_uploads(&state).await {
                Ok(cleaned) => {
                    if cleaned > 0 {
                        info!("expired upload cleanup removed {} files", cleaned);
                    }
                }
                Err(error) => error!("expired upload cleanup failed: {error}"),
            }
            last_upload_cleanup = std::time::Instant::now();
        }

        match fetch_next_job(&state.pool, &state.worker_id, queue_stale_after_seconds).await {
            Ok(Some(job)) => {
                info!("processing job {} topic={}", job.id, job.topic);
                heartbeat.set_processing(job.id, &job.topic).await;

                match handle_job(&state, &job).await {
                    Ok(()) => {
                        mark_done(&state.pool, &state.worker_id, &job).await?;
                        heartbeat.record_success(job.id, &job.topic).await;
                    }
                    Err(error) => {
                        error!("job processing failed: {error}");
                        reschedule_failed(&state.pool, &state.worker_id, &job, &error).await?;
                        heartbeat.record_failure(job.id, &job.topic, &error.to_string()).await;
                    }
                }
            }
            Ok(None) => {
                heartbeat.set_idle().await;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(error) => {
                error!("queue polling failed: {error}");
                heartbeat.set_queue_error(&error.to_string()).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
