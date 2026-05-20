use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};
use uuid::Uuid;

use crate::{
    dto::{EnqueueJobRequest, JobListItemDto, JobResponse},
    error::{internal_error, ApiResult},
    services::auth as auth_service,
    state::AppState,
};

pub async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<JobListItemDto>>> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let jobs = sqlx::query_as::<_, JobListItemDto>(
        r#"
        SELECT
            id,
            topic,
            status,
            attempt_count,
            locked_by,
            dedupe_key,
            last_error,
            available_at,
            created_at,
            updated_at
        FROM jobs
        ORDER BY created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(Json(jobs))
}

pub async fn enqueue_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EnqueueJobRequest>,
) -> ApiResult<(StatusCode, Json<JobResponse>)> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let row = sqlx::query_as::<_, (Uuid,)>(
        r#"
        INSERT INTO jobs (topic, payload, dedupe_key)
        VALUES ($1, $2, $3)
        ON CONFLICT (dedupe_key) WHERE dedupe_key IS NOT NULL
        DO UPDATE SET updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(request.topic)
    .bind(request.payload)
    .bind(request.dedupe_key)
    .fetch_one(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(JobResponse {
            id: row.0,
            status: "pending".to_string(),
        }),
    ))
}
