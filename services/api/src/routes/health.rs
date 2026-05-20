use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{
    dto::{
        AdminHealthResponse, HealthCheckDto, HealthResponse, OverviewResponse, QueueHealthDto,
        ReadinessResponse, RecentJobFailureDto, WorkerHeartbeatDto,
    },
    error::{internal_error, ApiResult},
    services::auth as auth_service,
    state::AppState,
};

const SERVICE_NAME: &str = "api";

#[derive(Debug, FromRow)]
struct QueueHealthRow {
    pending_jobs: i64,
    processing_jobs: i64,
    done_jobs: i64,
    dead_jobs: i64,
    stale_processing_jobs: i64,
    oldest_pending_seconds: i64,
    smtp_failed_jobs: i64,
    webhook_failed_jobs: i64,
}

#[derive(Debug, FromRow)]
struct WorkerHeartbeatRow {
    worker_id: String,
    status: String,
    heartbeat_at: DateTime<Utc>,
    heartbeat_age_seconds: i64,
    last_job_id: Option<Uuid>,
    last_job_topic: Option<String>,
    last_error: Option<String>,
    processed_jobs: i64,
    failed_jobs: i64,
}

struct OperationalHealth {
    status: String,
    generated_at: DateTime<Utc>,
    checks: Vec<HealthCheckDto>,
    queue: QueueHealthDto,
    workers: Vec<WorkerHeartbeatDto>,
    recent_failed_jobs: Vec<RecentJobFailureDto>,
}

impl OperationalHealth {
    fn http_status(&self) -> StatusCode {
        if self.status == "ok" {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

pub async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    let _ = state.config;
    Json(HealthResponse {
        status: "ok".to_string(),
        service: SERVICE_NAME.to_string(),
    })
}

pub async fn get_liveness(State(state): State<AppState>) -> Json<HealthResponse> {
    let _ = state.config;
    Json(HealthResponse {
        status: "ok".to_string(),
        service: SERVICE_NAME.to_string(),
    })
}

pub async fn get_readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    let health = collect_operational_health(&state).await;
    let status = health.http_status();

    (
        status,
        Json(ReadinessResponse {
            status: health.status,
            service: SERVICE_NAME.to_string(),
            checks: health.checks,
        }),
    )
}

pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let health = collect_operational_health(&state).await;
    let db_up = check_is_ok(&health.checks, "db");
    let http_status = if db_up {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        http_status,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        render_prometheus_metrics(&health),
    )
}

pub async fn get_admin_health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<(StatusCode, Json<AdminHealthResponse>)> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let health = collect_operational_health(&state).await;
    let status = health.http_status();

    Ok((
        status,
        Json(AdminHealthResponse {
            status: health.status,
            service: SERVICE_NAME.to_string(),
            generated_at: health.generated_at,
            checks: health.checks,
            queue: health.queue,
            workers: health.workers,
            recent_failed_jobs: health.recent_failed_jobs,
        }),
    ))
}

async fn collect_operational_health(state: &AppState) -> OperationalHealth {
    let pool = state.pool.as_ref();
    let generated_at = Utc::now();
    let mut checks = Vec::new();
    let mut queue = empty_queue_health();
    let mut workers = Vec::new();
    let mut recent_failed_jobs = Vec::new();

    match sqlx::query_scalar::<_, i64>("SELECT 1::BIGINT").fetch_one(pool).await {
        Ok(_) => checks.push(check(
            "db",
            true,
            Some("PostgreSQL connection is healthy".to_string()),
        )),
        Err(error) => {
            checks.push(check("db", false, Some(error.to_string())));
            checks.push(check(
                "schema",
                false,
                Some("skipped because DB is unavailable".to_string()),
            ));
            checks.push(check(
                "queue",
                false,
                Some("skipped because DB is unavailable".to_string()),
            ));
            checks.push(check(
                "worker",
                false,
                Some("skipped because DB is unavailable".to_string()),
            ));
            return OperationalHealth {
                status: readiness_status(&checks),
                generated_at,
                checks,
                queue,
                workers,
                recent_failed_jobs,
            };
        }
    }

    let schema_ok = match required_tables_present(pool).await {
        Ok(missing_tables) if missing_tables.is_empty() => {
            checks.push(check(
                "schema",
                true,
                Some("required tables are present".to_string()),
            ));
            true
        }
        Ok(missing_tables) => {
            checks.push(check(
                "schema",
                false,
                Some(format!("missing tables: {}", missing_tables.join(", "))),
            ));
            false
        }
        Err(error) => {
            checks.push(check("schema", false, Some(error.to_string())));
            false
        }
    };

    if !schema_ok {
        checks.push(check(
            "queue",
            false,
            Some("skipped because required tables are missing".to_string()),
        ));
        checks.push(check(
            "worker",
            false,
            Some("skipped because required tables are missing".to_string()),
        ));
        return OperationalHealth {
            status: readiness_status(&checks),
            generated_at,
            checks,
            queue,
            workers,
            recent_failed_jobs,
        };
    }

    match load_queue_health(pool, state.config.queue_stale_after_seconds).await {
        Ok(queue_health) => {
            queue = queue_health;
            if queue.stale_processing_jobs > 0 {
                checks.push(check(
                    "queue",
                    false,
                    Some(format!("{} processing jobs are stale", queue.stale_processing_jobs)),
                ));
            } else {
                checks.push(check(
                    "queue",
                    true,
                    Some(format!(
                        "queue reachable; pending={}, processing={}, dead={}",
                        queue.pending_jobs, queue.processing_jobs, queue.dead_jobs
                    )),
                ));
            }
        }
        Err(error) => checks.push(check("queue", false, Some(error.to_string()))),
    }

    match load_worker_heartbeats(pool, state.config.worker_stale_after_seconds).await {
        Ok(worker_health) => {
            workers = worker_health;
            let healthy_workers = workers.iter().filter(|worker| worker.healthy).count();
            if healthy_workers > 0 {
                checks.push(check(
                    "worker",
                    true,
                    Some(format!("{healthy_workers}/{} workers healthy", workers.len())),
                ));
            } else {
                checks.push(check(
                    "worker",
                    false,
                    Some("no healthy worker heartbeat found".to_string()),
                ));
            }
        }
        Err(error) => checks.push(check("worker", false, Some(error.to_string()))),
    }

    if let Ok(jobs) = load_recent_failed_jobs(pool).await {
        recent_failed_jobs = jobs;
    }

    OperationalHealth {
        status: readiness_status(&checks),
        generated_at,
        checks,
        queue,
        workers,
        recent_failed_jobs,
    }
}

async fn required_tables_present(pool: &PgPool) -> Result<Vec<&'static str>, sqlx::Error> {
    let required_tables = ["jobs", "worker_heartbeats"];
    let mut missing = Vec::new();

    for table in required_tables {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = 'public'
                  AND table_name = $1
            )
            "#,
        )
        .bind(table)
        .fetch_one(pool)
        .await?;

        if !exists {
            missing.push(table);
        }
    }

    Ok(missing)
}

async fn load_queue_health(
    pool: &PgPool,
    stale_after_seconds: u64,
) -> Result<QueueHealthDto, sqlx::Error> {
    let stale_after_seconds = stale_after_seconds.min(i32::MAX as u64) as i32;
    let row = sqlx::query_as::<_, QueueHealthRow>(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'pending')::BIGINT AS pending_jobs,
            COUNT(*) FILTER (WHERE status = 'processing')::BIGINT AS processing_jobs,
            COUNT(*) FILTER (WHERE status = 'done')::BIGINT AS done_jobs,
            COUNT(*) FILTER (WHERE status = 'dead')::BIGINT AS dead_jobs,
            COUNT(*) FILTER (
                WHERE status = 'processing'
                  AND locked_at < NOW() - make_interval(secs => $1)
            )::BIGINT AS stale_processing_jobs,
            COALESCE(
                EXTRACT(EPOCH FROM (
                    NOW() - MIN(available_at) FILTER (WHERE status = 'pending' AND available_at <= NOW())
                ))::BIGINT,
                0
            ) AS oldest_pending_seconds,
            COUNT(*) FILTER (
                WHERE topic IN ('user.invitation.send_email', 'user.password_reset.send_email')
                  AND (status = 'dead' OR last_error IS NOT NULL)
            )::BIGINT AS smtp_failed_jobs,
            COUNT(*) FILTER (
                WHERE topic = 'gitlab.webhook.received'
                  AND (status = 'dead' OR last_error IS NOT NULL)
            )::BIGINT AS webhook_failed_jobs
        FROM jobs
        "#,
    )
    .bind(stale_after_seconds)
    .fetch_one(pool)
    .await?;

    Ok(QueueHealthDto {
        pending_jobs: row.pending_jobs,
        processing_jobs: row.processing_jobs,
        done_jobs: row.done_jobs,
        dead_jobs: row.dead_jobs,
        stale_processing_jobs: row.stale_processing_jobs,
        oldest_pending_seconds: row.oldest_pending_seconds,
        smtp_failed_jobs: row.smtp_failed_jobs,
        webhook_failed_jobs: row.webhook_failed_jobs,
    })
}

async fn load_worker_heartbeats(
    pool: &PgPool,
    stale_after_seconds: u64,
) -> Result<Vec<WorkerHeartbeatDto>, sqlx::Error> {
    let rows = sqlx::query_as::<_, WorkerHeartbeatRow>(
        r#"
        SELECT
            worker_id,
            status,
            heartbeat_at,
            COALESCE(EXTRACT(EPOCH FROM (NOW() - heartbeat_at))::BIGINT, 0) AS heartbeat_age_seconds,
            last_job_id,
            last_job_topic,
            last_error,
            processed_jobs,
            failed_jobs
        FROM worker_heartbeats
        ORDER BY heartbeat_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let healthy =
                row.heartbeat_age_seconds <= stale_after_seconds as i64 && row.status != "queue_error";
            WorkerHeartbeatDto {
                worker_id: row.worker_id,
                status: row.status,
                healthy,
                heartbeat_age_seconds: row.heartbeat_age_seconds,
                heartbeat_at: row.heartbeat_at,
                last_job_id: row.last_job_id,
                last_job_topic: row.last_job_topic,
                last_error: row.last_error,
                processed_jobs: row.processed_jobs,
                failed_jobs: row.failed_jobs,
            }
        })
        .collect())
}

async fn load_recent_failed_jobs(pool: &PgPool) -> Result<Vec<RecentJobFailureDto>, sqlx::Error> {
    sqlx::query_as::<_, RecentJobFailureDto>(
        r#"
        SELECT
            id,
            topic,
            status,
            attempt_count,
            last_error,
            updated_at
        FROM jobs
        WHERE status = 'dead'
           OR last_error IS NOT NULL
        ORDER BY updated_at DESC
        LIMIT 25
        "#,
    )
    .fetch_all(pool)
    .await
}

fn empty_queue_health() -> QueueHealthDto {
    QueueHealthDto {
        pending_jobs: 0,
        processing_jobs: 0,
        done_jobs: 0,
        dead_jobs: 0,
        stale_processing_jobs: 0,
        oldest_pending_seconds: 0,
        smtp_failed_jobs: 0,
        webhook_failed_jobs: 0,
    }
}

fn check(name: &str, ok: bool, message: Option<String>) -> HealthCheckDto {
    HealthCheckDto {
        name: name.to_string(),
        status: if ok { "ok" } else { "down" }.to_string(),
        message,
    }
}

fn readiness_status(checks: &[HealthCheckDto]) -> String {
    if checks.iter().all(|check| check.status == "ok") {
        "ok".to_string()
    } else {
        "degraded".to_string()
    }
}

fn check_is_ok(checks: &[HealthCheckDto], name: &str) -> bool {
    checks
        .iter()
        .any(|check| check.name == name && check.status == "ok")
}

fn render_prometheus_metrics(health: &OperationalHealth) -> String {
    let ready = if health.status == "ok" { 1 } else { 0 };
    let db_up = if check_is_ok(&health.checks, "db") { 1 } else { 0 };
    let queue_up = if check_is_ok(&health.checks, "queue") { 1 } else { 0 };
    let workers_total = health.workers.len() as i64;
    let workers_healthy = health.workers.iter().filter(|worker| worker.healthy).count() as i64;

    let mut lines = vec![
        "# HELP issuehub_up 1 when the API process can serve metrics.".to_string(),
        "# TYPE issuehub_up gauge".to_string(),
        "issuehub_up 1".to_string(),
        "# HELP issuehub_ready 1 when readiness checks are healthy.".to_string(),
        "# TYPE issuehub_ready gauge".to_string(),
        format!("issuehub_ready {ready}"),
        "# HELP issuehub_db_up 1 when PostgreSQL is reachable.".to_string(),
        "# TYPE issuehub_db_up gauge".to_string(),
        format!("issuehub_db_up {db_up}"),
        "# HELP issuehub_queue_up 1 when queue checks are healthy.".to_string(),
        "# TYPE issuehub_queue_up gauge".to_string(),
        format!("issuehub_queue_up {queue_up}"),
        "# HELP issuehub_jobs_total Jobs by queue status.".to_string(),
        "# TYPE issuehub_jobs_total gauge".to_string(),
        format!("issuehub_jobs_total{{status=\"pending\"}} {}", health.queue.pending_jobs),
        format!(
            "issuehub_jobs_total{{status=\"processing\"}} {}",
            health.queue.processing_jobs
        ),
        format!("issuehub_jobs_total{{status=\"done\"}} {}", health.queue.done_jobs),
        format!("issuehub_jobs_total{{status=\"dead\"}} {}", health.queue.dead_jobs),
        "# HELP issuehub_jobs_stale_processing_total Processing jobs older than the configured stale threshold.".to_string(),
        "# TYPE issuehub_jobs_stale_processing_total gauge".to_string(),
        format!(
            "issuehub_jobs_stale_processing_total {}",
            health.queue.stale_processing_jobs
        ),
        "# HELP issuehub_jobs_oldest_pending_seconds Age of the oldest currently runnable pending job.".to_string(),
        "# TYPE issuehub_jobs_oldest_pending_seconds gauge".to_string(),
        format!(
            "issuehub_jobs_oldest_pending_seconds {}",
            health.queue.oldest_pending_seconds
        ),
        "# HELP issuehub_smtp_failed_jobs_total Jobs for SMTP-backed mail flows with last_error or dead status.".to_string(),
        "# TYPE issuehub_smtp_failed_jobs_total gauge".to_string(),
        format!(
            "issuehub_smtp_failed_jobs_total {}",
            health.queue.smtp_failed_jobs
        ),
        "# HELP issuehub_webhook_failed_jobs_total GitLab webhook jobs with last_error or dead status.".to_string(),
        "# TYPE issuehub_webhook_failed_jobs_total gauge".to_string(),
        format!(
            "issuehub_webhook_failed_jobs_total {}",
            health.queue.webhook_failed_jobs
        ),
        "# HELP issuehub_workers_total Known worker heartbeat rows.".to_string(),
        "# TYPE issuehub_workers_total gauge".to_string(),
        format!("issuehub_workers_total {workers_total}"),
        "# HELP issuehub_workers_healthy Fresh workers not reporting queue_error.".to_string(),
        "# TYPE issuehub_workers_healthy gauge".to_string(),
        format!("issuehub_workers_healthy {workers_healthy}"),
        "# HELP issuehub_worker_heartbeat_age_seconds Age of each worker heartbeat.".to_string(),
        "# TYPE issuehub_worker_heartbeat_age_seconds gauge".to_string(),
    ];

    for worker in &health.workers {
        let worker_id = prometheus_label_value(&worker.worker_id);
        lines.push(format!(
            "issuehub_worker_heartbeat_age_seconds{{worker_id=\"{worker_id}\"}} {}",
            worker.heartbeat_age_seconds
        ));
    }

    lines.push("# HELP issuehub_worker_processed_jobs_total Processed jobs reported by each worker.".to_string());
    lines.push("# TYPE issuehub_worker_processed_jobs_total counter".to_string());
    for worker in &health.workers {
        let worker_id = prometheus_label_value(&worker.worker_id);
        lines.push(format!(
            "issuehub_worker_processed_jobs_total{{worker_id=\"{worker_id}\"}} {}",
            worker.processed_jobs
        ));
    }

    lines.push("# HELP issuehub_worker_failed_jobs_total Failed jobs reported by each worker.".to_string());
    lines.push("# TYPE issuehub_worker_failed_jobs_total counter".to_string());
    for worker in &health.workers {
        let worker_id = prometheus_label_value(&worker.worker_id);
        lines.push(format!(
            "issuehub_worker_failed_jobs_total{{worker_id=\"{worker_id}\"}} {}",
            worker.failed_jobs
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

fn prometheus_label_value(value: &str) -> String {
    value
        .replace('\\', r"\\")
        .replace('"', "\\\"")
        .replace('\n', r"\n")
}

pub async fn get_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<OverviewResponse>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let (project_count, integrated_project_count, issue_count, pending_jobs, processing_jobs) =
        if current_user.is_admin {
            let project_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM projects")
                .fetch_one(state.pool.as_ref())
                .await
                .map_err(internal_error)?;

            let integrated_project_count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM project_gitlab_integrations WHERE sync_enabled = TRUE")
                    .fetch_one(state.pool.as_ref())
                    .await
                    .map_err(internal_error)?;

            let issue_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM issues")
                .fetch_one(state.pool.as_ref())
                .await
                .map_err(internal_error)?;

            let pending_jobs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE status = 'pending'")
                .fetch_one(state.pool.as_ref())
                .await
                .map_err(internal_error)?;

            let processing_jobs: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE status = 'processing'")
                    .fetch_one(state.pool.as_ref())
                    .await
                    .map_err(internal_error)?;

            (
                project_count.0,
                integrated_project_count.0,
                issue_count.0,
                pending_jobs.0,
                processing_jobs.0,
            )
        } else {
            let project_count: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(DISTINCT projects.id)
                FROM projects
                LEFT JOIN project_permissions
                  ON project_permissions.project_id = projects.id
                 AND project_permissions.effect = 'allow'
                 AND (
                   (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $1)
                   OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $2)
                 )
                 AND project_permissions.permission = ANY($3)
                LEFT JOIN issues ON issues.project_id = projects.id
                LEFT JOIN issue_permissions
                  ON issue_permissions.issue_id = issues.id
                 AND issue_permissions.subject_type = 'user'
                 AND issue_permissions.subject_id = $1
                 AND issue_permissions.effect = 'allow'
                 AND issue_permissions.permission = ANY($4)
                WHERE project_permissions.project_id IS NOT NULL
                   OR issue_permissions.issue_id IS NOT NULL
                "#,
            )
            .bind(current_user.id.to_string())
            .bind(current_user.email.clone())
            .bind(["view", "create_issue", "admin"].as_slice())
            .bind(["read", "comment", "edit", "admin"].as_slice())
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?;

            let integrated_project_count: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(DISTINCT projects.id)
                FROM projects
                JOIN project_gitlab_integrations
                  ON project_gitlab_integrations.project_id = projects.id
                 AND project_gitlab_integrations.sync_enabled = TRUE
                LEFT JOIN project_permissions
                  ON project_permissions.project_id = projects.id
                 AND project_permissions.effect = 'allow'
                 AND (
                   (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $1)
                   OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $2)
                 )
                 AND project_permissions.permission = ANY($3)
                LEFT JOIN issues ON issues.project_id = projects.id
                LEFT JOIN issue_permissions
                  ON issue_permissions.issue_id = issues.id
                 AND issue_permissions.subject_type = 'user'
                 AND issue_permissions.subject_id = $1
                 AND issue_permissions.effect = 'allow'
                 AND issue_permissions.permission = ANY($4)
                WHERE project_permissions.project_id IS NOT NULL
                   OR issue_permissions.issue_id IS NOT NULL
                "#,
            )
            .bind(current_user.id.to_string())
            .bind(current_user.email.clone())
            .bind(["view", "create_issue", "admin"].as_slice())
            .bind(["read", "comment", "edit", "admin"].as_slice())
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?;

            let issue_count: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(DISTINCT issues.id)
                FROM issues
                LEFT JOIN project_permissions
                  ON project_permissions.project_id = issues.project_id
                 AND project_permissions.effect = 'allow'
                 AND (
                   (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $1)
                   OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $2)
                 )
                 AND project_permissions.permission = ANY($3)
                LEFT JOIN issue_permissions
                  ON issue_permissions.issue_id = issues.id
                 AND issue_permissions.subject_type = 'user'
                 AND issue_permissions.subject_id = $1
                 AND issue_permissions.effect = 'allow'
                 AND issue_permissions.permission = ANY($4)
                WHERE project_permissions.project_id IS NOT NULL
                   OR issue_permissions.issue_id IS NOT NULL
                "#,
            )
            .bind(current_user.id.to_string())
            .bind(current_user.email.clone())
            .bind(["view", "admin"].as_slice())
            .bind(["read", "comment", "edit", "admin"].as_slice())
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?;

            (project_count.0, integrated_project_count.0, issue_count.0, 0, 0)
        };

    Ok(Json(OverviewResponse {
        project_count,
        integrated_project_count,
        issue_count,
        pending_jobs,
        processing_jobs,
    }))
}
