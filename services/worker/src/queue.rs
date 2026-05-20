use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const MAX_ATTEMPTS: i32 = 5;

#[derive(Debug, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub topic: String,
    pub payload: Value,
    pub attempt_count: i32,
}

#[derive(Debug, FromRow)]
struct StaleJob {
    id: Uuid,
    topic: String,
    payload: Value,
    status: String,
    attempt_count: i32,
}

pub async fn fetch_next_job(
    pool: &PgPool,
    worker_id: &str,
    stale_processing_seconds: i32,
) -> anyhow::Result<Option<Job>> {
    recover_stale_processing_jobs(pool, worker_id, stale_processing_seconds).await?;

    let mut tx = pool.begin().await?;

    let job = sqlx::query_as::<_, Job>(
        r#"
        WITH next_job AS (
            SELECT id
            FROM jobs
            WHERE status = 'pending'
              AND available_at <= NOW()
            ORDER BY created_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        UPDATE jobs
        SET status = 'processing',
            locked_at = NOW(),
            locked_by = $1,
            updated_at = NOW()
        WHERE id IN (SELECT id FROM next_job)
        RETURNING id, topic, payload, attempt_count
        "#,
    )
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(job)
}

pub async fn mark_done(pool: &PgPool, worker_id: &str, job: &Job) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'done',
            locked_at = NULL,
            locked_by = NULL,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job.id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (entity_type, entity_id, action, actor, payload)
        VALUES ('job', $1, 'processed', $2, $3)
        "#,
    )
    .bind(job.id)
    .bind(worker_id)
    .bind(&job.payload)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn reschedule_failed(
    pool: &PgPool,
    worker_id: &str,
    job: &Job,
    error: &anyhow::Error,
) -> anyhow::Result<()> {
    let next_attempt = job.attempt_count + 1;
    let status = if next_attempt >= MAX_ATTEMPTS { "dead" } else { "pending" };
    let delay_seconds = retry_delay_seconds(next_attempt);

    sqlx::query(
        r#"
        UPDATE jobs
        SET status = $2,
            attempt_count = $3,
            last_error = $4,
            available_at = NOW() + make_interval(secs => $5),
            locked_at = NULL,
            locked_by = NULL,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job.id)
    .bind(status)
    .bind(next_attempt)
    .bind(error.to_string())
    .bind(delay_seconds)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (entity_type, entity_id, action, actor, payload)
        VALUES ('job', $1, $2, $3, $4)
        "#,
    )
    .bind(job.id)
    .bind(if status == "dead" { "dead" } else { "failed" })
    .bind(worker_id)
    .bind(serde_json::json!({
        "topic": &job.topic,
        "payload": &job.payload,
        "error": error.to_string(),
        "attempt_count": next_attempt,
        "next_status": status,
        "retry_delay_seconds": delay_seconds,
    }))
    .execute(pool)
    .await?;

    Ok(())
}

async fn recover_stale_processing_jobs(
    pool: &PgPool,
    worker_id: &str,
    stale_processing_seconds: i32,
) -> anyhow::Result<()> {
    let rows = sqlx::query_as::<_, StaleJob>(
        r#"
        UPDATE jobs
        SET attempt_count = attempt_count + 1,
            status = CASE
                WHEN attempt_count + 1 >= $2 THEN 'dead'
                ELSE 'pending'
            END,
            last_error = 'Job lock expired while processing',
            available_at = NOW() + make_interval(secs => $3),
            locked_at = NULL,
            locked_by = NULL,
            updated_at = NOW()
        WHERE status = 'processing'
          AND locked_at < NOW() - make_interval(secs => $1)
        RETURNING id, topic, payload, status, attempt_count
        "#,
    )
    .bind(stale_processing_seconds)
    .bind(MAX_ATTEMPTS)
    .bind(retry_delay_seconds(1))
    .fetch_all(pool)
    .await?;

    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO audit_log (entity_type, entity_id, action, actor, payload)
            VALUES ('job', $1, $2, $3, $4)
            "#,
        )
        .bind(row.id)
        .bind(if row.status == "dead" {
            "stale_lock_dead"
        } else {
            "stale_lock_recovered"
        })
        .bind(worker_id)
        .bind(serde_json::json!({
            "topic": &row.topic,
            "payload": &row.payload,
            "attempt_count": row.attempt_count,
            "next_status": &row.status,
            "stale_after_seconds": stale_processing_seconds,
        }))
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn retry_delay_seconds(attempt: i32) -> i64 {
    let exponent = (attempt - 1).clamp(0, 5) as u32;
    (10_i64 * 2_i64.pow(exponent)).min(300)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;

    #[sqlx::test(migrations = false)]
    async fn fetch_and_mark_done_updates_queue_and_audit(pool: PgPool) {
        setup_schema(&pool).await;
        let job_id = insert_job(&pool, "phase3.queue.done", 0, "pending", None).await;

        let job = fetch_next_job(&pool, "worker-test", 300)
            .await
            .unwrap()
            .expect("pending job should be fetched");
        assert_eq!(job.id, job_id);
        assert_eq!(job.topic, "phase3.queue.done");

        let processing = job_status(&pool, job_id).await;
        assert_eq!(processing.status, "processing");
        assert_eq!(processing.locked_by.as_deref(), Some("worker-test"));

        mark_done(&pool, "worker-test", &job).await.unwrap();

        let done = job_status(&pool, job_id).await;
        assert_eq!(done.status, "done");
        assert!(done.locked_by.is_none());

        let audit_count = audit_count(&pool, job_id, "processed").await;
        assert_eq!(audit_count, 1);
    }

    #[sqlx::test(migrations = false)]
    async fn failed_job_is_retried_then_dead_lettered(pool: PgPool) {
        setup_schema(&pool).await;
        let retry_job_id = insert_job(&pool, "phase3.queue.retry", 0, "pending", None).await;
        let retry_job = fetch_next_job(&pool, "worker-test", 300)
            .await
            .unwrap()
            .expect("pending retry job should be fetched");

        reschedule_failed(&pool, "worker-test", &retry_job, &anyhow!("temporary failure"))
            .await
            .unwrap();

        let retry = job_status(&pool, retry_job_id).await;
        assert_eq!(retry.status, "pending");
        assert_eq!(retry.attempt_count, 1);
        assert_eq!(retry.last_error.as_deref(), Some("temporary failure"));
        assert!(audit_count(&pool, retry_job_id, "failed").await >= 1);

        let dead_job_id = insert_job(&pool, "phase3.queue.dead", 4, "pending", None).await;
        let dead_job = fetch_next_job(&pool, "worker-test", 300)
            .await
            .unwrap()
            .expect("pending dead job should be fetched");

        reschedule_failed(&pool, "worker-test", &dead_job, &anyhow!("permanent failure"))
            .await
            .unwrap();

        let dead = job_status(&pool, dead_job_id).await;
        assert_eq!(dead.status, "dead");
        assert_eq!(dead.attempt_count, 5);
        assert_eq!(dead.last_error.as_deref(), Some("permanent failure"));
        assert_eq!(audit_count(&pool, dead_job_id, "dead").await, 1);
    }

    #[sqlx::test(migrations = false)]
    async fn stale_processing_jobs_are_recovered_or_dead_lettered(pool: PgPool) {
        setup_schema(&pool).await;
        let stale_retry_id = insert_job(&pool, "phase3.queue.stale-retry", 0, "processing", Some(600)).await;

        let fetched = fetch_next_job(&pool, "worker-test", 300).await.unwrap();
        assert!(fetched.is_none());

        let recovered = job_status(&pool, stale_retry_id).await;
        assert_eq!(recovered.status, "pending");
        assert_eq!(recovered.attempt_count, 1);
        assert_eq!(
            recovered.last_error.as_deref(),
            Some("Job lock expired while processing")
        );
        assert_eq!(audit_count(&pool, stale_retry_id, "stale_lock_recovered").await, 1);

        let stale_dead_id = insert_job(&pool, "phase3.queue.stale-dead", 4, "processing", Some(600)).await;
        let fetched = fetch_next_job(&pool, "worker-test", 300).await.unwrap();
        assert!(fetched.is_none());

        let dead = job_status(&pool, stale_dead_id).await;
        assert_eq!(dead.status, "dead");
        assert_eq!(dead.attempt_count, 5);
        assert_eq!(audit_count(&pool, stale_dead_id, "stale_lock_dead").await, 1);
    }

    #[derive(Debug, sqlx::FromRow)]
    struct JobStatus {
        status: String,
        attempt_count: i32,
        locked_by: Option<String>,
        last_error: Option<String>,
    }

    async fn setup_schema(pool: &PgPool) {
        sqlx::raw_sql(include_str!("../../../infra/postgres/init/001-init.sql"))
            .execute(pool)
            .await
            .unwrap();
    }

    async fn insert_job(
        pool: &PgPool,
        topic: &str,
        attempt_count: i32,
        status: &str,
        locked_seconds_ago: Option<i32>,
    ) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO jobs (
                topic,
                payload,
                status,
                attempt_count,
                available_at,
                locked_at,
                locked_by
            )
            VALUES (
                $1,
                jsonb_build_object('topic', $1),
                $2,
                $3,
                NOW(),
                CASE WHEN $4::INTEGER IS NULL THEN NULL ELSE NOW() - make_interval(secs => $4) END,
                CASE WHEN $4::INTEGER IS NULL THEN NULL ELSE 'stale-worker' END
            )
            RETURNING id
            "#,
        )
        .bind(topic)
        .bind(status)
        .bind(attempt_count)
        .bind(locked_seconds_ago)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn job_status(pool: &PgPool, job_id: Uuid) -> JobStatus {
        sqlx::query_as::<_, JobStatus>(
            r#"
            SELECT status, attempt_count, locked_by, last_error
            FROM jobs
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn audit_count(pool: &PgPool, job_id: Uuid, action: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM audit_log
            WHERE entity_type = 'job'
              AND entity_id = $1
              AND action = $2
            "#,
        )
        .bind(job_id)
        .bind(action)
        .fetch_one(pool)
        .await
        .unwrap()
    }
}
