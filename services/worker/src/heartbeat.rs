use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing::error;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct WorkerHeartbeatHandle {
    inner: Arc<RwLock<HeartbeatSnapshot>>,
}

#[derive(Debug)]
struct HeartbeatSnapshot {
    status: String,
    last_job_id: Option<Uuid>,
    last_job_topic: Option<String>,
    last_error: Option<String>,
    processed_delta: i64,
    failed_delta: i64,
}

impl Default for HeartbeatSnapshot {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            last_job_id: None,
            last_job_topic: None,
            last_error: None,
            processed_delta: 0,
            failed_delta: 0,
        }
    }
}

impl WorkerHeartbeatHandle {
    pub async fn set_idle(&self) {
        let mut snapshot = self.inner.write().await;
        snapshot.status = "idle".to_string();
    }

    pub async fn set_processing(&self, job_id: Uuid, topic: &str) {
        let mut snapshot = self.inner.write().await;
        snapshot.status = "processing".to_string();
        snapshot.last_job_id = Some(job_id);
        snapshot.last_job_topic = Some(topic.to_string());
    }

    pub async fn set_queue_error(&self, error: &str) {
        let mut snapshot = self.inner.write().await;
        snapshot.status = "queue_error".to_string();
        snapshot.last_error = Some(error.to_string());
    }

    pub async fn record_success(&self, job_id: Uuid, topic: &str) {
        let mut snapshot = self.inner.write().await;
        snapshot.status = "idle".to_string();
        snapshot.last_job_id = Some(job_id);
        snapshot.last_job_topic = Some(topic.to_string());
        snapshot.processed_delta += 1;
    }

    pub async fn record_failure(&self, job_id: Uuid, topic: &str, error: &str) {
        let mut snapshot = self.inner.write().await;
        snapshot.status = "idle".to_string();
        snapshot.last_job_id = Some(job_id);
        snapshot.last_job_topic = Some(topic.to_string());
        snapshot.last_error = Some(error.to_string());
        snapshot.failed_delta += 1;
    }

    async fn flush(&self, pool: &PgPool, worker_id: &str) -> anyhow::Result<()> {
        let snapshot = {
            let mut snapshot = self.inner.write().await;
            let current = HeartbeatSnapshot {
                status: snapshot.status.clone(),
                last_job_id: snapshot.last_job_id,
                last_job_topic: snapshot.last_job_topic.clone(),
                last_error: snapshot.last_error.clone(),
                processed_delta: snapshot.processed_delta,
                failed_delta: snapshot.failed_delta,
            };
            snapshot.processed_delta = 0;
            snapshot.failed_delta = 0;
            current
        };

        record_heartbeat(
            pool,
            worker_id,
            &snapshot.status,
            snapshot
                .last_job_id
                .zip(snapshot.last_job_topic.as_deref())
                .map(|(id, topic)| (id, topic)),
            snapshot.last_error.as_deref(),
            snapshot.processed_delta,
            snapshot.failed_delta,
        )
        .await
    }
}

pub fn spawn_heartbeat_loop(
    pool: PgPool,
    worker_id: String,
    interval: Duration,
    handle: WorkerHeartbeatHandle,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(error) = handle.flush(&pool, &worker_id).await {
                error!("worker heartbeat failed: {error}");
            }
            tokio::time::sleep(interval).await;
        }
    })
}

pub async fn record_heartbeat(
    pool: &PgPool,
    worker_id: &str,
    status: &str,
    last_job: Option<(Uuid, &str)>,
    last_error: Option<&str>,
    processed_delta: i64,
    failed_delta: i64,
) -> anyhow::Result<()> {
    let (last_job_id, last_job_topic) = last_job
        .map(|(id, topic)| (Some(id), Some(topic.to_string())))
        .unwrap_or((None, None));

    sqlx::query(
        r#"
        INSERT INTO worker_heartbeats (
            worker_id,
            status,
            heartbeat_at,
            last_job_id,
            last_job_topic,
            last_error,
            processed_jobs,
            failed_jobs,
            updated_at
        )
        VALUES ($1, $2, NOW(), $3, $4, $5, GREATEST($6, 0), GREATEST($7, 0), NOW())
        ON CONFLICT (worker_id)
        DO UPDATE SET
            status = EXCLUDED.status,
            heartbeat_at = NOW(),
            last_job_id = COALESCE(EXCLUDED.last_job_id, worker_heartbeats.last_job_id),
            last_job_topic = COALESCE(EXCLUDED.last_job_topic, worker_heartbeats.last_job_topic),
            last_error = CASE
                WHEN EXCLUDED.last_error IS NULL AND EXCLUDED.status IN ('idle', 'processing') THEN worker_heartbeats.last_error
                ELSE EXCLUDED.last_error
            END,
            processed_jobs = worker_heartbeats.processed_jobs + GREATEST($6, 0),
            failed_jobs = worker_heartbeats.failed_jobs + GREATEST($7, 0),
            updated_at = NOW()
        "#,
    )
    .bind(worker_id)
    .bind(status)
    .bind(last_job_id)
    .bind(last_job_topic)
    .bind(last_error)
    .bind(processed_delta)
    .bind(failed_delta)
    .execute(pool)
    .await?;

    Ok(())
}
