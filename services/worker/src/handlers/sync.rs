use anyhow::{anyhow, Context, Result};
use bridge_core::{
    config::AppConfig,
    gitlab::{
        create_project_issue, fetch_issue, import_issue_comments, import_project_issues,
        update_project_issue, GitLabCreateIssueInput, GitLabIssueImportInput, GitLabUpdateIssueInput,
    },
    issue_sync::{persist_gitlab_comment_and_attachments, persist_gitlab_issue_attachments, upsert_gitlab_issue_row},
    secrets::decrypt_secret,
};
use chrono::{Duration, Utc};
use lettre::{
    message::Mailbox,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde::Deserialize;
use serde_json::json;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{queue::Job, state::WorkerState};

pub async fn handle_job(state: &WorkerState, job: &Job) -> Result<()> {
    match job.topic.as_str() {
        "gitlab.webhook.received" => handle_gitlab_webhook(state, job).await,
        "issue.sync.pull" => handle_sync_pull(state, job).await,
        "issue.sync.push" => handle_sync_push(state, job).await,
        "issue.reconcile" => handle_reconcile(state, job).await,
        "user.invitation.send_email" => handle_user_invitation_email(state, job).await,
        "user.password_reset.send_email" => handle_password_reset_email(state, job).await,
        other => {
            warn!("unknown job topic {}", other);
            Ok(())
        }
    }
}

async fn handle_sync_pull(state: &WorkerState, job: &Job) -> Result<()> {
    let payload: SyncPullPayload =
        serde_json::from_value(job.payload.clone()).context("invalid issue.sync.pull payload")?;

    let integration = load_project_integration(&state.pool, payload.project_id).await?;

    let Some(integration) = integration else {
        info!("skipping sync.pull job {} because integration no longer exists", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.sync.pull.skipped",
            job,
            json!({
                "reason": "integration_missing",
                "project_id": payload.project_id,
                "issue_iid": payload.issue_iid,
                "sync_comments": payload.sync_comments,
            }),
        )
        .await?;
        return Ok(());
    };

    if !integration.sync_enabled {
        info!("skipping sync.pull job {} because sync is disabled", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.sync.pull.skipped",
            job,
            json!({
                "reason": "sync_disabled",
                "project_id": payload.project_id,
                "issue_iid": payload.issue_iid,
                "sync_comments": payload.sync_comments,
            }),
        )
        .await?;
        return Ok(());
    }

    let token = decrypt_integration_token(&state.config, &integration)?;

    let result = sync_issue_from_gitlab(
        &state.pool,
        payload.project_id,
        &integration,
        token,
        payload.issue_iid,
        payload.sync_comments,
    )
    .await
    .with_context(|| format!("sync.pull job {} failed", job.id))?;

    record_sync_audit(
        state,
        "issue",
        result.issue_id,
        "gitlab.sync.pull.completed",
        job,
        json!({
            "project_id": payload.project_id,
            "issue_iid": payload.issue_iid,
            "sync_comments": payload.sync_comments,
            "imported_comments": result.imported_comments,
        }),
    )
    .await?;

    info!(
        "sync.pull job {} pulled project {} issue #{} comments={}",
        job.id, payload.project_id, payload.issue_iid, payload.sync_comments
    );
    Ok(())
}

async fn handle_gitlab_webhook(state: &WorkerState, job: &Job) -> Result<()> {
    let payload: GitLabWebhookJobPayload =
        serde_json::from_value(job.payload.clone()).context("invalid gitlab webhook payload")?;

    let integration = load_project_integration(&state.pool, payload.project_id).await?;

    let Some(integration) = integration else {
        info!("skipping webhook job {} because integration no longer exists", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.webhook.skipped",
            job,
            json!({
                "reason": "integration_missing",
                "project_id": payload.project_id,
                "issue_iid": payload.issue_iid,
                "sync_comments": payload.sync_comments,
            }),
        )
        .await?;
        return Ok(());
    };

    if !integration.sync_enabled {
        info!("skipping webhook job {} because integration sync is disabled", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.webhook.skipped",
            job,
            json!({
                "reason": "sync_disabled",
                "project_id": payload.project_id,
                "issue_iid": payload.issue_iid,
                "sync_comments": payload.sync_comments,
            }),
        )
        .await?;
        return Ok(());
    }

    let token = decrypt_integration_token(&state.config, &integration)?;

    let result = sync_issue_from_gitlab(
        &state.pool,
        payload.project_id,
        &integration,
        token,
        payload.issue_iid,
        payload.sync_comments,
    )
    .await?;

    record_sync_audit(
        state,
        "issue",
        result.issue_id,
        "gitlab.webhook.processed",
        job,
        json!({
            "project_id": payload.project_id,
            "issue_iid": payload.issue_iid,
            "sync_comments": payload.sync_comments,
            "imported_comments": result.imported_comments,
        }),
    )
    .await?;

    info!(
        "processed GitLab webhook job {} for project {} issue #{} comments={}",
        job.id, payload.project_id, payload.issue_iid, payload.sync_comments
    );

    Ok(())
}

async fn handle_sync_push(state: &WorkerState, job: &Job) -> Result<()> {
    let payload: SyncPushPayload =
        serde_json::from_value(job.payload.clone()).context("invalid issue.sync.push payload")?;

    let issue = sqlx::query_as::<_, LocalIssueRow>(
        r#"
        SELECT id, project_id, gitlab_issue_iid, title, description, state
        FROM issues
        WHERE id = $1
        "#,
    )
    .bind(payload.issue_id)
    .fetch_optional(&state.pool)
    .await
    .context("failed to load issue for sync.push")?;

    let Some(issue) = issue else {
        info!("skipping sync.push job {} because issue no longer exists", job.id);
        record_sync_audit(
            state,
            "job",
            job.id,
            "gitlab.sync.push.skipped",
            job,
            json!({
                "reason": "issue_missing",
                "issue_id": payload.issue_id,
            }),
        )
        .await?;
        return Ok(());
    };

    let integration = load_project_integration(&state.pool, issue.project_id).await?;

    let Some(integration) = integration else {
        info!("skipping sync.push job {} because integration no longer exists", job.id);
        record_sync_audit(
            state,
            "issue",
            issue.id,
            "gitlab.sync.push.skipped",
            job,
            json!({
                "reason": "integration_missing",
                "project_id": issue.project_id,
                "issue_id": issue.id,
            }),
        )
        .await?;
        return Ok(());
    };

    if !integration.sync_enabled {
        info!("skipping sync.push job {} because sync is disabled", job.id);
        record_sync_audit(
            state,
            "issue",
            issue.id,
            "gitlab.sync.push.skipped",
            job,
            json!({
                "reason": "sync_disabled",
                "project_id": issue.project_id,
                "issue_id": issue.id,
            }),
        )
        .await?;
        return Ok(());
    }

    let token = decrypt_integration_token(&state.config, &integration)?;

    if issue.gitlab_issue_iid.is_none() {
        let gitlab_issue = create_project_issue(GitLabCreateIssueInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            token,
            verify_tls: integration.verify_tls,
            title: issue.title.clone(),
            description: issue.description.clone(),
        })
        .await
        .context("failed to create GitLab issue in sync.push")?;

        sqlx::query(
            r#"
            UPDATE issues
            SET gitlab_issue_iid = $2, last_source = 'gitlab', sync_state = 'idle', updated_at = $3
            WHERE id = $1
            "#,
        )
        .bind(issue.id)
        .bind(gitlab_issue.iid)
        .bind(gitlab_issue.updated_at)
        .execute(&state.pool)
        .await
        .context("failed to update issue after GitLab creation in sync.push")?;

        persist_gitlab_issue_attachments(&state.pool, issue.id, &integration.gitlab_base_url, &gitlab_issue.description)
            .await
            .context("failed to persist attachments after GitLab creation in sync.push")?;

        record_sync_audit(
            state,
            "issue",
            issue.id,
            "gitlab.sync.push.created",
            job,
            json!({
                "project_id": issue.project_id,
                "issue_id": issue.id,
                "issue_iid": gitlab_issue.iid,
            }),
        )
        .await?;

        info!(
            "sync.push job {} created GitLab issue #{} for local issue {}",
            job.id, gitlab_issue.iid, issue.id
        );
    } else if let Some(gitlab_issue_iid) = issue.gitlab_issue_iid {
        let state_event = if issue.state == "closed" {
            Some("close".to_string())
        } else {
            Some("reopen".to_string())
        };

        let gitlab_issue = update_project_issue(GitLabUpdateIssueInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            gitlab_issue_iid,
            token,
            verify_tls: integration.verify_tls,
            title: issue.title.clone(),
            description: issue.description.clone(),
            state_event,
        })
        .await
        .context("failed to update GitLab issue in sync.push")?;

        sqlx::query(
            r#"
            UPDATE issues
            SET last_source = 'gitlab', sync_state = 'idle', updated_at = $2
            WHERE id = $1
            "#,
        )
        .bind(issue.id)
        .bind(gitlab_issue.updated_at)
        .execute(&state.pool)
        .await
        .context("failed to update issue after GitLab update in sync.push")?;

        persist_gitlab_issue_attachments(&state.pool, issue.id, &integration.gitlab_base_url, &gitlab_issue.description)
            .await
            .context("failed to persist attachments after GitLab update in sync.push")?;

        record_sync_audit(
            state,
            "issue",
            issue.id,
            "gitlab.sync.push.updated",
            job,
            json!({
                "project_id": issue.project_id,
                "issue_id": issue.id,
                "issue_iid": gitlab_issue_iid,
                "state": issue.state,
            }),
        )
        .await?;

        info!(
            "sync.push job {} pushed issue {} to GitLab #{}",
            job.id,
            issue.id,
            gitlab_issue_iid
        );
    }

    Ok(())
}

async fn handle_reconcile(state: &WorkerState, job: &Job) -> Result<()> {
    let payload: ReconcilePayload =
        serde_json::from_value(job.payload.clone()).context("invalid issue.reconcile payload")?;

    let integration = load_project_integration(&state.pool, payload.project_id).await?;

    let Some(integration) = integration else {
        info!("skipping reconcile job {} because integration no longer exists", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.reconcile.skipped",
            job,
            json!({
                "reason": "integration_missing",
                "project_id": payload.project_id,
            }),
        )
        .await?;
        return Ok(());
    };

    if !integration.sync_enabled {
        info!("skipping reconcile job {} because sync is disabled", job.id);
        record_sync_audit(
            state,
            "project",
            payload.project_id,
            "gitlab.reconcile.skipped",
            job,
            json!({
                "reason": "sync_disabled",
                "project_id": payload.project_id,
            }),
        )
        .await?;
        return Ok(());
    }

    let token = decrypt_integration_token(&state.config, &integration)?;

    let issues = import_project_issues(GitLabIssueImportInput {
        gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
        gitlab_project_id: integration.gitlab_project_id,
        token,
        verify_tls: integration.verify_tls,
    })
    .await
    .context("failed to fetch issues from GitLab in reconcile")?;

    let count = issues.len();

    for issue in &issues {
        let issue_id = upsert_gitlab_issue_row(&state.pool, payload.project_id, issue)
            .await
            .context("failed to upsert issue in reconcile")?;
        persist_gitlab_issue_attachments(&state.pool, issue_id, &integration.gitlab_base_url, &issue.description)
            .await
            .context("failed to persist issue attachments in reconcile")?;
    }

    record_sync_audit(
        state,
        "project",
        payload.project_id,
        "gitlab.reconcile.completed",
        job,
        json!({
            "project_id": payload.project_id,
            "imported_issues": count,
        }),
    )
    .await?;

    info!(
        "reconcile job {} processed {} issues for project {}",
        job.id, count, payload.project_id
    );
    Ok(())
}

async fn handle_user_invitation_email(_state: &WorkerState, job: &Job) -> Result<()> {
    let payload: InvitationEmailPayload =
        serde_json::from_value(job.payload.clone()).context("invalid invitation email payload")?;

    let smtp = &_state.config.smtp;
    if smtp.host.is_empty() || smtp.from_email.is_empty() {
        return Err(anyhow!("SMTP is not configured"));
    }

    let from = Mailbox::new(
        Some(smtp.from_name.clone()),
        smtp.from_email
            .parse()
            .context("invalid SMTP_FROM_EMAIL address")?,
    );
    let to = payload
        .email
        .parse()
        .context("invalid invitation recipient address")?;

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject("IssueHub invitation")
        .body(invitation_email_body(&payload))?;

    let transport_builder = if smtp.starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
            .context("failed to create STARTTLS SMTP transport")?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)
            .context("failed to create SMTP transport")?
    };

    let transport = transport_builder
        .port(smtp.port)
        .credentials(Credentials::new(
            smtp.username.clone(),
            smtp.password.clone(),
        ))
        .build();

    transport
        .send(email)
        .await
        .context("failed to send invitation email")?;

    info!("invite email sent for job {} to {}", job.id, payload.email);
    Ok(())
}

async fn handle_password_reset_email(_state: &WorkerState, job: &Job) -> Result<()> {
    let payload: PasswordResetEmailPayload =
        serde_json::from_value(job.payload.clone()).context("invalid password reset email payload")?;

    let smtp = &_state.config.smtp;
    if smtp.host.is_empty() || smtp.from_email.is_empty() {
        return Err(anyhow!("SMTP is not configured"));
    }

    let from = Mailbox::new(
        Some(smtp.from_name.clone()),
        smtp.from_email
            .parse()
            .context("invalid SMTP_FROM_EMAIL address")?,
    );
    let to = payload
        .email
        .parse()
        .context("invalid password reset recipient address")?;

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject("IssueHub password recovery")
        .body(password_reset_email_body(&payload))?;

    let transport_builder = if smtp.starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
            .context("failed to create STARTTLS SMTP transport")?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)
            .context("failed to create SMTP transport")?
    };

    let transport = transport_builder
        .port(smtp.port)
        .credentials(Credentials::new(
            smtp.username.clone(),
            smtp.password.clone(),
        ))
        .build();

    transport
        .send(email)
        .await
        .context("failed to send password recovery email")?;

    info!("password recovery email sent for job {} to {}", job.id, payload.email);
    Ok(())
}

async fn sync_issue_from_gitlab(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    integration: &WorkerProjectIntegrationRow,
    token: String,
    issue_iid: i64,
    sync_comments: bool,
) -> Result<SyncIssueResult> {
    let gl_input = GitLabIssueImportInput {
        gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
        gitlab_project_id: integration.gitlab_project_id,
        token: token.clone(),
        verify_tls: integration.verify_tls,
    };

    let issue = fetch_issue(gl_input.clone(), issue_iid)
        .await
        .context("failed to fetch issue from GitLab")?
        .into_summary();

    let issue_id = upsert_gitlab_issue_row(pool, project_id, &issue)
        .await
        .context("failed to upsert issue row")?;

    persist_gitlab_issue_attachments(pool, issue_id, &integration.gitlab_base_url, &issue.description)
        .await
        .context("failed to persist issue attachments")?;

    let mut imported_comments = 0usize;
    if sync_comments {
        let comments = import_issue_comments(gl_input, issue_iid)
            .await
            .context("failed to import issue comments from GitLab")?;
        imported_comments = comments.len();

        let mut last_comment_activity = issue.updated_at;
        for comment in &comments {
            if comment.updated_at > last_comment_activity {
                last_comment_activity = comment.updated_at;
            }
            persist_gitlab_comment_and_attachments(pool, issue_id, &integration.gitlab_base_url, comment)
                .await
                .context("failed to persist synced comment")?;
        }

        sqlx::query(
            r#"
            UPDATE issues
            SET updated_at = GREATEST(updated_at, $2)
            WHERE id = $1
            "#,
        )
        .bind(issue_id)
        .bind(last_comment_activity)
        .execute(pool)
        .await
        .context("failed to update issue last activity after comment sync")?;
    }

    Ok(SyncIssueResult {
        issue_id,
        imported_comments,
    })
}

async fn record_sync_audit(
    state: &WorkerState,
    entity_type: &str,
    entity_id: Uuid,
    action: &str,
    job: &Job,
    payload: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_log (entity_type, entity_id, action, actor, payload)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(&state.worker_id)
    .bind(json!({
        "job_id": job.id,
        "topic": &job.topic,
        "attempt_count": job.attempt_count,
        "details": payload,
    }))
    .execute(&state.pool)
    .await
    .context("failed to write sync audit entry")?;

    Ok(())
}

async fn load_project_integration(
    pool: &sqlx::PgPool,
    project_id: Uuid,
) -> Result<Option<WorkerProjectIntegrationRow>> {
    sqlx::query_as::<_, WorkerProjectIntegrationRow>(
        r#"
        SELECT gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .context("failed to load GitLab integration")
}

fn decrypt_integration_token(config: &AppConfig, integration: &WorkerProjectIntegrationRow) -> Result<String> {
    let encrypted_token = integration
        .token_encrypted
        .as_ref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("GitLab token is not configured"))?;

    decrypt_secret(&config.secret_encryption_key, encrypted_token).context("failed to decrypt GitLab token")
}

#[derive(Debug, Deserialize)]
struct SyncPullPayload {
    project_id: Uuid,
    issue_iid: i64,
    sync_comments: bool,
}

#[derive(Debug, Deserialize)]
struct SyncPushPayload {
    issue_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct ReconcilePayload {
    project_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct LocalIssueRow {
    id: Uuid,
    project_id: Uuid,
    gitlab_issue_iid: Option<i64>,
    title: String,
    description: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct InvitationEmailPayload {
    email: String,
    invite_url: String,
    is_admin: bool,
    invited_by: String,
}

#[derive(Debug, Deserialize)]
struct PasswordResetEmailPayload {
    email: String,
    full_name: String,
    reset_url: String,
}

#[derive(Debug, Deserialize)]
struct GitLabWebhookJobPayload {
    project_id: Uuid,
    issue_iid: i64,
    sync_comments: bool,
}

struct SyncIssueResult {
    issue_id: Uuid,
    imported_comments: usize,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkerProjectIntegrationRow {
    gitlab_base_url: String,
    gitlab_api_base_url: String,
    gitlab_project_id: i64,
    token_encrypted: Option<String>,
    verify_tls: bool,
    sync_enabled: bool,
}

fn invitation_email_body(payload: &InvitationEmailPayload) -> String {
    let role = if payload.is_admin { "admin" } else { "member" };
    format!(
        "You were invited to IssueHub as {role} by {invited_by}.\n\nOpen this link to continue:\n{invite_url}\n",
        role = role,
        invited_by = payload.invited_by,
        invite_url = payload.invite_url
    )
}

fn password_reset_email_body(payload: &PasswordResetEmailPayload) -> String {
    format!(
        "Hello {full_name},\n\nA password recovery request was received for your IssueHub account.\n\nOpen this link to set a new password:\n{reset_url}\n\nIf you did not request this change, you can ignore this email.\n",
        full_name = payload.full_name,
        reset_url = payload.reset_url
    )
}

#[derive(Debug, sqlx::FromRow)]
struct ExpiredUploadRow {
    id: Uuid,
    storage_path: String,
}

pub async fn cleanup_expired_uploads(state: &WorkerState) -> Result<usize> {
    let cutoff = Utc::now() - Duration::hours(state.config.temp_upload_retention_hours);

    let uploads = sqlx::query_as::<_, ExpiredUploadRow>(
        r#"
        SELECT id, storage_path
        FROM issue_uploads
        WHERE consumed_at IS NULL
          AND created_at < $1
        "#,
    )
    .bind(cutoff)
    .fetch_all(&state.pool)
    .await
    .context("failed to load expired uploads")?;

    for upload in &uploads {
        match fs::remove_file(&upload.storage_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                warn!("failed to remove expired upload file {}: {}", upload.storage_path, error);
                continue;
            }
        }

        sqlx::query(
            r#"
            DELETE FROM issue_uploads
            WHERE id = $1
              AND consumed_at IS NULL
            "#,
        )
        .bind(upload.id)
        .execute(&state.pool)
        .await
        .with_context(|| format!("failed to delete expired upload {}", upload.id))?;
    }

    if !uploads.is_empty() {
        info!("cleaned up {} expired temp uploads", uploads.len());
    }

    Ok(uploads.len())
}
