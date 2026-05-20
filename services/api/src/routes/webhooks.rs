use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use bridge_core::secrets::decrypt_secret;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    dto::{GitLabWebhookResponse, ProjectIntegrationRow},
    error::{internal_error, ApiError, ApiResult},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct GitLabWebhookPayload {
    object_kind: Option<String>,
    project: Option<GitLabWebhookProject>,
    issue: Option<GitLabWebhookIssueRef>,
    object_attributes: Option<GitLabWebhookObjectAttributes>,
}

#[derive(Debug, Deserialize)]
struct GitLabWebhookProject {
    id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GitLabWebhookIssueRef {
    iid: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GitLabWebhookObjectAttributes {
    iid: Option<i64>,
    noteable_type: Option<String>,
}

pub async fn receive_gitlab_webhook(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GitLabWebhookPayload>,
) -> ApiResult<(StatusCode, Json<GitLabWebhookResponse>)> {
    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "GitLab integration not found"))?;

    verify_webhook_secret(&state, &integration, &headers)?;
    verify_project_match(&integration, &payload)?;

    let event_type = headers
        .get("X-Gitlab-Event")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| payload.object_kind.as_deref().unwrap_or("unknown"))
        .to_string();

    let object_kind = payload.object_kind.as_deref().unwrap_or_default();

    let response = match object_kind {
        "issue" => enqueue_issue_event(state.pool.as_ref(), project_id, &payload, &event_type).await?,
        "note" => enqueue_note_event(state.pool.as_ref(), project_id, &payload, &event_type).await?,
        _ => GitLabWebhookResponse {
            status: "ignored".to_string(),
            event_type,
            handled: false,
            job_id: None,
            issue_id: None,
            issue_iid: payload.object_attributes.as_ref().and_then(|attrs| attrs.iid),
        },
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}

async fn enqueue_issue_event(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    payload: &GitLabWebhookPayload,
    event_type: &str,
) -> ApiResult<GitLabWebhookResponse> {
    let issue_iid = payload
        .object_attributes
        .as_ref()
        .and_then(|attrs| attrs.iid)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Webhook payload did not include issue iid"))?;
    let job_id = enqueue_webhook_job(pool, project_id, issue_iid, false, event_type, payload).await?;

    Ok(GitLabWebhookResponse {
        status: "queued".to_string(),
        event_type: event_type.to_string(),
        handled: true,
        job_id: Some(job_id),
        issue_id: None,
        issue_iid: Some(issue_iid),
    })
}

async fn enqueue_note_event(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    payload: &GitLabWebhookPayload,
    event_type: &str,
) -> ApiResult<GitLabWebhookResponse> {
    let noteable_type = payload
        .object_attributes
        .as_ref()
        .and_then(|attrs| attrs.noteable_type.as_deref())
        .unwrap_or_default();

    if noteable_type != "Issue" {
        return Ok(GitLabWebhookResponse {
            status: "ignored".to_string(),
            event_type: event_type.to_string(),
            handled: false,
            job_id: None,
            issue_id: None,
            issue_iid: None,
        });
    }

    let issue_iid = payload
        .issue
        .as_ref()
        .and_then(|issue| issue.iid)
        .or_else(|| payload.object_attributes.as_ref().and_then(|attrs| attrs.iid))
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Webhook payload did not include note issue iid"))?;
    let job_id = enqueue_webhook_job(pool, project_id, issue_iid, true, event_type, payload).await?;

    Ok(GitLabWebhookResponse {
        status: "queued".to_string(),
        event_type: event_type.to_string(),
        handled: true,
        job_id: Some(job_id),
        issue_id: None,
        issue_iid: Some(issue_iid),
    })
}

async fn enqueue_webhook_job(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    issue_iid: i64,
    sync_comments: bool,
    event_type: &str,
    payload: &GitLabWebhookPayload,
) -> ApiResult<Uuid> {
    let dedupe_key = format!(
        "gitlab-webhook:{project_id}:{issue_iid}:{}",
        if sync_comments { "comments" } else { "issue" }
    );

    let job_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO jobs (topic, payload, dedupe_key)
        VALUES ($1, $2, $3)
        ON CONFLICT (dedupe_key)
        DO UPDATE SET
            payload = EXCLUDED.payload,
            status = 'pending',
            attempt_count = 0,
            available_at = NOW(),
            locked_at = NULL,
            locked_by = NULL,
            last_error = NULL,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind("gitlab.webhook.received")
    .bind(json!({
        "project_id": project_id,
        "issue_iid": issue_iid,
        "sync_comments": sync_comments,
        "event_type": event_type,
        "object_kind": payload.object_kind,
    }))
    .bind(&dedupe_key)
    .fetch_one(pool)
    .await
    .map_err(internal_error)?;

    record_webhook_audit(pool, project_id, job_id, issue_iid, sync_comments, event_type, &dedupe_key).await?;

    Ok(job_id)
}

async fn record_webhook_audit(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    job_id: Uuid,
    issue_iid: i64,
    sync_comments: bool,
    event_type: &str,
    dedupe_key: &str,
) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_log (entity_type, entity_id, action, actor, payload)
        VALUES ('project', $1, 'gitlab.webhook.queued', 'gitlab-webhook', $2)
        "#,
    )
    .bind(project_id)
    .bind(json!({
        "job_id": job_id,
        "issue_iid": issue_iid,
        "sync_comments": sync_comments,
        "event_type": event_type,
        "dedupe_key": dedupe_key,
    }))
    .execute(pool)
    .await
    .map_err(internal_error)?;

    Ok(())
}

fn verify_webhook_secret(
    state: &AppState,
    integration: &ProjectIntegrationRow,
    headers: &HeaderMap,
) -> ApiResult<()> {
    let header_token = headers
        .get("X-Gitlab-Token")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Missing GitLab webhook token"))?;

    let encrypted_secret = integration
        .webhook_secret_encrypted
        .as_ref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab webhook secret is not configured"))?;

    let expected = decrypt_secret(&state.config.secret_encryption_key, encrypted_secret)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    if expected != header_token {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Invalid GitLab webhook token"));
    }

    Ok(())
}

fn verify_project_match(integration: &ProjectIntegrationRow, payload: &GitLabWebhookPayload) -> ApiResult<()> {
    if let Some(project_id) = payload.project.as_ref().and_then(|project| project.id) {
        if project_id != integration.gitlab_project_id {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Webhook project did not match integration"));
        }
    }

    Ok(())
}
