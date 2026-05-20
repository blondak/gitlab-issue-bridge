use std::{collections::HashSet, path::{Path as FsPath, PathBuf}};

use axum::{
    body::Body,
    extract::{multipart::Field, Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use bridge_core::{
    issue_sync::{persist_gitlab_comment_and_attachments, persist_gitlab_issue_attachments},
    secrets::decrypt_secret,
};
use tokio::fs;
use uuid::Uuid;

use crate::{
    dto::{
        attachment_to_dto, AttachmentRow, CommentDto, CommentRow, CreateIssueCommentRequest,
        CreateProjectIssueRequest, GitLabCommentImportResponse, IssueUploadDto,
        IssueAccessAssignmentDto, IssueAccessOverviewDto, IssueAccessUserOptionDto, IssueCapabilitiesDto,
        IssueDetailDto, IssueDto, IssuePermissionRow, IssueRow, ProjectIntegrationRow,
        UpdateIssueAccessRequest, UpdateIssueRequest, UserRow,
    },
    error::{internal_error, ApiError, ApiResult},
    services::{
        auth as auth_service,
        gitlab::{
            create_issue_comment, create_project_issue, fetch_issue_web_url, import_issue_comments,
            update_project_issue, upload_project_attachment, GitLabCreateIssueCommentInput,
            GitLabCreateIssueInput, GitLabIssueImportInput, GitLabUpdateIssueInput,
            GitLabUploadAttachmentInput,
        },
    },
    state::AppState,
};

const READ_PERMISSIONS: [&str; 4] = ["read", "comment", "edit", "admin"];
const MANAGEABLE_PERMISSIONS: [&str; 4] = ["read", "comment", "edit", "admin"];
const COMMENT_PERMISSIONS: [&str; 3] = ["comment", "edit", "admin"];
const EDIT_PERMISSIONS: [&str; 2] = ["edit", "admin"];

#[derive(sqlx::FromRow)]
struct IssueUploadRow {
    id: Uuid,
    project_id: Uuid,
    issue_id: Option<Uuid>,
    uploaded_by_user_id: Uuid,
    filename: String,
    content_type: String,
    byte_size: i64,
    storage_path: String,
    proxy_path: String,
}

pub async fn list_issues(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<IssueDto>>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let issues = if current_user.is_admin {
        sqlx::query_as::<_, IssueRow>(
            r#"
            SELECT
                issues.id,
                issues.project_id,
                projects.slug AS project_slug,
                projects.name AS project_name,
                COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                issues.title,
                issues.description,
                issues.state,
                issues.sync_state,
                issues.last_source,
                issues.version,
                issues.created_at,
                issues.updated_at
            FROM issues
            JOIN projects ON projects.id = issues.project_id
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, IssueRow>(
            r#"
            SELECT DISTINCT
                issues.id,
                issues.project_id,
                projects.slug AS project_slug,
                projects.name AS project_name,
                COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                issues.title,
                issues.description,
                issues.state,
                issues.sync_state,
                issues.last_source,
                issues.version,
                issues.created_at,
                issues.updated_at
            FROM issues
            JOIN projects ON projects.id = issues.project_id
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
            ORDER BY updated_at DESC
            "#,
        )
        .bind(current_user.id.to_string())
        .bind(current_user.email.clone())
        .bind(["view", "admin"].as_slice())
        .bind(READ_PERMISSIONS.as_slice())
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    };

    let mut response = Vec::with_capacity(issues.len());
    for issue in issues {
        let capabilities = issue_capabilities_for_user(&state, &issue, &current_user).await?;
        response.push(IssueDto::from_parts(issue, capabilities));
    }

    Ok(Json(response))
}

pub async fn create_issue(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectIssueRequest>,
) -> ApiResult<(StatusCode, Json<IssueDto>)> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_issue_write_access(&state, project_id, &current_user).await?;

    if request.title.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Issue title is required"));
    }

    let referenced_uploads =
        fetch_referenced_uploads(state.pool.as_ref(), project_id, None, current_user.id, request.description.as_deref().unwrap_or_default()).await?;

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
    .map_err(internal_error)?;

    let issue = if let Some(integration) = integration.filter(|value| value.sync_enabled) {
        let encrypted_token = integration
            .token_encrypted
            .as_ref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

        let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        let rewritten_description = replace_temp_uploads_with_gitlab_uploads(
            request.description.as_deref().unwrap_or_default(),
            &referenced_uploads,
            &integration,
            token.clone(),
        )
        .await?;

        let gitlab_issue = create_project_issue(GitLabCreateIssueInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            token,
            verify_tls: integration.verify_tls,
            title: request.title.trim().to_string(),
            description: rewritten_description,
        })
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

        let issue = insert_issue_row(
            state.pool.as_ref(),
            project_id,
            Some(gitlab_issue.iid),
            &gitlab_issue.title,
            &gitlab_issue.description,
            &gitlab_issue.state,
            "idle",
            gitlab_issue.created_at,
            gitlab_issue.updated_at,
        )
        .await?;

        persist_gitlab_issue_attachments(
            state.pool.as_ref(),
            issue.id,
            &integration.gitlab_base_url,
            &gitlab_issue.description,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        issue
    } else {
        let issue = insert_issue_row(
            state.pool.as_ref(),
            project_id,
            None,
            request.title.trim(),
            request.description.as_deref().unwrap_or_default(),
            "open",
            "local",
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
        .await?;

        let rewritten_description = persist_local_issue_attachments(
            state.pool.as_ref(),
            issue.id,
            current_user.id,
            &current_user.full_name,
            request.description.as_deref().unwrap_or_default(),
            &referenced_uploads,
            &state,
        )
        .await?;

        let issue = if rewritten_description != issue.description {
            sqlx::query_as::<_, IssueRow>(
                r#"
                UPDATE issues
                SET description = $2,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING
                    issues.id,
                    issues.project_id,
                    (SELECT slug FROM projects WHERE id = issues.project_id) AS project_slug,
                    (SELECT name FROM projects WHERE id = issues.project_id) AS project_name,
                    COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                    issues.title,
                    issues.description,
                    issues.state,
                    issues.sync_state,
                    issues.last_source,
                    issues.version,
                    issues.created_at,
                    issues.updated_at
                "#,
            )
            .bind(issue.id)
            .bind(&rewritten_description)
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?
        } else {
            issue
        };

        issue
    };

    if !referenced_uploads.is_empty() {
        cleanup_consumed_uploads(state.pool.as_ref(), &referenced_uploads).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
        VALUES ($1, 'user', $2, 'admin', 'allow')
        "#,
    )
    .bind(issue.id)
    .bind(current_user.id.to_string())
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let capabilities = issue_capabilities_for_user(&state, &issue, &current_user).await?;

    Ok((StatusCode::CREATED, Json(IssueDto::from_parts(issue, capabilities))))
}

pub async fn get_issue_detail(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<IssueDetailDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    let issue = fetch_authorized_issue(&state, issue_id, &current_user).await?;

    if let Some(integration) = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(issue.project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    {
        persist_gitlab_issue_attachments(
            state.pool.as_ref(),
            issue_id,
            &integration.gitlab_base_url,
            &issue.description,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    }

    let comments = sqlx::query_as::<_, CommentRow>(
        r#"
        SELECT id, issue_id, gitlab_note_id, discussion_id, individual_note, reply_to_gitlab_note_id, author_external_id, author_name, body_raw, system_note, sync_state, created_at, updated_at
        FROM issue_comments
        WHERE issue_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(issue_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let attachments = sqlx::query_as::<_, AttachmentRow>(
        r#"
        SELECT id, issue_id, comment_id, filename, content_type, byte_size, external_url, proxy_path, storage_backend, storage_path, inline, created_by_external_id, sync_state, created_at
        FROM issue_attachments
        WHERE issue_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(issue_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let comment_dtos = comments
        .into_iter()
        .map(|comment| {
            let comment_attachments = attachments
                .iter()
                .filter(|attachment| attachment.comment_id == Some(comment.id))
                .map(attachment_to_dto)
                .collect();

            CommentDto {
                id: comment.id,
                gitlab_note_id: comment.gitlab_note_id,
                discussion_id: comment.discussion_id,
                individual_note: comment.individual_note,
                reply_to_gitlab_note_id: comment.reply_to_gitlab_note_id,
                author_external_id: comment.author_external_id,
                author_name: comment.author_name,
                body_raw: comment.body_raw,
                system_note: comment.system_note,
                sync_state: comment.sync_state,
                attachments: comment_attachments,
                created_at: comment.created_at,
            }
        })
        .collect();

    let issue_attachments = attachments
        .iter()
        .filter(|attachment| attachment.comment_id.is_none())
        .map(attachment_to_dto)
        .collect();

    Ok(Json(IssueDetailDto {
        issue: IssueDto::from_parts(
            issue.clone(),
            issue_capabilities_for_user(&state, &issue, &current_user).await?,
        ),
        comments: comment_dtos,
        issue_attachments,
    }))
}

pub async fn update_issue(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateIssueRequest>,
) -> ApiResult<Json<IssueDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    let issue = fetch_authorized_issue(&state, issue_id, &current_user).await?;

    let title = request
        .title
        .as_deref()
        .unwrap_or(&issue.title)
        .trim()
        .to_string();
    let description = request
        .description
        .as_deref()
        .unwrap_or(&issue.description)
        .to_string();
    let requested_state = request
        .state
        .as_deref()
        .map(normalize_issue_state)
        .transpose()?
        .unwrap_or_else(|| normalized_existing_issue_state(&issue.state));

    if title.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Issue title is required"));
    }

    let content_changed = title != issue.title || description != issue.description;
    let current_state = normalized_existing_issue_state(&issue.state);
    let state_changed = requested_state != current_state;

    if content_changed {
        ensure_issue_edit_access(&state, issue_id, &current_user).await?;
    }

    if state_changed {
        ensure_issue_state_access(&state, issue_id, &current_user).await?;
    }

    if !content_changed && !state_changed {
        let capabilities = issue_capabilities_for_user(&state, &issue, &current_user).await?;
        return Ok(Json(IssueDto::from_parts(issue, capabilities)));
    }

    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(issue.project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let updated_issue = if let Some(integration) = integration.filter(|value| value.sync_enabled && issue.gitlab_issue_iid > 0) {
        let encrypted_token = integration
            .token_encrypted
            .as_ref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

        let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        let state_event = if state_changed {
            Some(match requested_state.as_str() {
                "closed" => "close".to_string(),
                "open" => "reopen".to_string(),
                _ => unreachable!("issue state is normalized before sync"),
            })
        } else {
            None
        };

        let gitlab_issue = update_project_issue(GitLabUpdateIssueInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            gitlab_issue_iid: issue.gitlab_issue_iid,
            token,
            verify_tls: integration.verify_tls,
            title,
            description,
            state_event,
        })
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

        let updated_issue = update_issue_row(
            state.pool.as_ref(),
            issue_id,
            &gitlab_issue.title,
            &gitlab_issue.description,
            &gitlab_issue.state,
            "idle",
            "gitlab",
            gitlab_issue.updated_at,
        )
        .await?;

        persist_gitlab_issue_attachments(
            state.pool.as_ref(),
            issue_id,
            &integration.gitlab_base_url,
            &gitlab_issue.description,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        updated_issue
    } else {
        update_issue_row(
            state.pool.as_ref(),
            issue_id,
            &title,
            &description,
            &requested_state,
            "local",
            "bridge",
            chrono::Utc::now(),
        )
        .await?
    };

    let capabilities = issue_capabilities_for_user(&state, &updated_issue, &current_user).await?;
    Ok(Json(IssueDto::from_parts(updated_issue, capabilities)))
}

pub async fn redirect_issue_to_gitlab(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Redirect> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    let issue = fetch_authorized_issue(&state, issue_id, &current_user).await?;

    if issue.gitlab_issue_iid <= 0 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Issue is not linked to GitLab"));
    }

    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(issue.project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "GitLab integration not found"))?;

    let encrypted_token = integration
        .token_encrypted
        .as_ref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

    let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let issue_web_url = fetch_issue_web_url(GitLabIssueImportInput {
        gitlab_api_base_url: integration.gitlab_api_base_url,
        gitlab_project_id: integration.gitlab_project_id,
        token,
        verify_tls: integration.verify_tls,
    }, issue.gitlab_issue_iid)
    .await
    .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

    Ok(Redirect::temporary(&issue_web_url))
}

pub async fn redirect_note_to_gitlab(
    Path((issue_id, note_id)): Path<(Uuid, i64)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Redirect> {
    let redirect = redirect_issue_to_gitlab(Path(issue_id), State(state), headers).await?;
    let location = redirect
        .into_response()
        .headers()
        .get(axum::http::header::LOCATION)
        .and_then(|value: &axum::http::HeaderValue| value.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "GitLab redirect URL missing"))?
        .to_string();

    Ok(Redirect::temporary(&format!("{location}#note_{note_id}")))
}

pub async fn create_issue_comment_handler(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateIssueCommentRequest>,
) -> ApiResult<(StatusCode, Json<CommentDto>)> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    let issue = fetch_authorized_issue_for_comment(&state, issue_id, &current_user).await?;

    if request.body.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Comment body is required"));
    }

    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(issue.project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if let Some(integration) = integration.filter(|value| value.sync_enabled && issue.gitlab_issue_iid > 0) {
        let encrypted_token = integration
            .token_encrypted
            .as_ref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

        let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        let uploads =
            fetch_referenced_uploads(state.pool.as_ref(), issue.project_id, Some(issue_id), current_user.id, request.body.trim()).await?;
        let body_for_gitlab = replace_temp_uploads_with_gitlab_uploads(
            request.body.trim(),
            &uploads,
            &integration,
            token.clone(),
        )
        .await?;

        let reply_discussion_id = if let Some(reply_to_note_id) = request.reply_to_note_id {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                SELECT discussion_id
                FROM issue_comments
                WHERE issue_id = $1
                  AND gitlab_note_id = $2
                LIMIT 1
                "#,
            )
            .bind(issue_id)
            .bind(reply_to_note_id)
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?
        } else {
            None
        };

        if request.reply_to_note_id.is_some() && reply_discussion_id.is_none() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Reply target discussion could not be resolved",
            ));
        }

        let gitlab_comment = create_issue_comment(GitLabCreateIssueCommentInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            gitlab_issue_iid: issue.gitlab_issue_iid,
            token,
            verify_tls: integration.verify_tls,
            body: body_for_gitlab,
            discussion_id: reply_discussion_id,
            reply_to_note_id: request.reply_to_note_id,
        })
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

        let comment_id = persist_gitlab_comment_and_attachments(
            state.pool.as_ref(),
            issue_id,
            &integration.gitlab_base_url,
            &gitlab_comment,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let comment = fetch_comment_dto(state.pool.as_ref(), comment_id).await?;

        cleanup_consumed_uploads(state.pool.as_ref(), &uploads).await?;

        sqlx::query(
            r#"
            UPDATE issues
            SET updated_at = GREATEST(updated_at, $2)
            WHERE id = $1
            "#,
        )
        .bind(issue_id)
        .bind(gitlab_comment.updated_at)
        .execute(state.pool.as_ref())
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        Ok((StatusCode::CREATED, Json(comment)))
    } else {
        let uploads =
            fetch_referenced_uploads(state.pool.as_ref(), issue.project_id, Some(issue_id), current_user.id, request.body.trim()).await?;
        let next_local_note_id = next_local_note_id(state.pool.as_ref(), issue_id).await?;
        let local_discussion_id = if let Some(reply_to_note_id) = request.reply_to_note_id {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                SELECT discussion_id
                FROM issue_comments
                WHERE issue_id = $1
                  AND gitlab_note_id = $2
                LIMIT 1
                "#,
            )
            .bind(issue_id)
            .bind(reply_to_note_id)
            .fetch_one(state.pool.as_ref())
            .await
            .map_err(internal_error)?
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Reply target could not be resolved"))?
        } else {
            format!("local:discussion:{}", Uuid::new_v4())
        };
        let (body_raw, attachments) = persist_local_comment_attachments(
            state.pool.as_ref(),
            issue_id,
            current_user.id,
            &current_user.full_name,
            request.body.trim(),
            &uploads,
            &state,
        )
        .await?;

        let comment_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO issue_comments (
                issue_id,
                gitlab_note_id,
                discussion_id,
                individual_note,
                reply_to_gitlab_note_id,
                author_external_id,
                author_name,
                body_raw,
                system_note,
                sync_state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, FALSE, $4, $5, $6, $7, FALSE, 'local', NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(issue_id)
        .bind(next_local_note_id)
        .bind(&local_discussion_id)
        .bind(request.reply_to_note_id)
        .bind(format!("issuehub:user:{}", current_user.id))
        .bind(current_user.full_name.clone())
        .bind(&body_raw)
        .fetch_one(state.pool.as_ref())
        .await
        .map_err(internal_error)?;

        attach_local_attachments_to_comment(state.pool.as_ref(), comment_id, &attachments).await?;
        cleanup_consumed_uploads(state.pool.as_ref(), &uploads).await?;

        sqlx::query("UPDATE issues SET updated_at = NOW() WHERE id = $1")
            .bind(issue_id)
            .execute(state.pool.as_ref())
            .await
            .map_err(internal_error)?;

        Ok((
            StatusCode::CREATED,
            Json(CommentDto {
                id: comment_id,
                gitlab_note_id: next_local_note_id,
                author_external_id: format!("issuehub:user:{}", current_user.id),
                author_name: current_user.full_name,
                body_raw,
                discussion_id: Some(local_discussion_id),
                individual_note: false,
                reply_to_gitlab_note_id: request.reply_to_note_id,
                system_note: false,
                sync_state: "local".to_string(),
                attachments,
                created_at: chrono::Utc::now(),
            }),
        ))
    }
}

pub async fn upload_issue_attachment(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<IssueUploadDto>)> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    let _issue = fetch_authorized_issue_for_comment(&state, issue_id, &current_user).await?;
    enforce_upload_rate_limit(&state, current_user.id)?;

    let field = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "File is required"))?;

    let (file_name, content_type, bytes) = read_validated_upload_field(field, &state).await?;

    let upload_id = Uuid::new_v4();
    let upload_dir = attachment_uploads_dir(&state);
    fs::create_dir_all(&upload_dir)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let storage_path = upload_dir.join(format!("{}_{}", upload_id, file_name));
    fs::write(&storage_path, &bytes)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let proxy_path = format!("/api/v1/uploads/{upload_id}/download");
    sqlx::query(
        r#"
        INSERT INTO issue_uploads (
            id,
            project_id,
            issue_id,
            uploaded_by_user_id,
            filename,
            content_type,
            byte_size,
            storage_path,
            proxy_path
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(upload_id)
    .bind(_issue.project_id)
    .bind(issue_id)
    .bind(current_user.id)
    .bind(&file_name)
    .bind(&content_type)
    .bind(bytes.len() as i64)
    .bind(path_to_string(&storage_path))
    .bind(&proxy_path)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let markdown = if content_type.starts_with("image/") {
        format!("![{}]({})", file_name, proxy_path)
    } else {
        format!("[{}]({})", file_name, proxy_path)
    };

    Ok((
        StatusCode::CREATED,
        Json(IssueUploadDto {
            upload_id,
            filename: file_name,
            content_type,
            byte_size: bytes.len() as i64,
            proxy_path,
            markdown,
        }),
    ))
}

pub async fn upload_project_issue_attachment(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<IssueUploadDto>)> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_issue_write_access(&state, project_id, &current_user).await?;
    enforce_upload_rate_limit(&state, current_user.id)?;

    let field = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "File is required"))?;

    let (file_name, content_type, bytes) = read_validated_upload_field(field, &state).await?;

    let upload_id = Uuid::new_v4();
    let upload_dir = attachment_uploads_dir(&state);
    fs::create_dir_all(&upload_dir)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let storage_path = upload_dir.join(format!("{}_{}", upload_id, file_name));
    fs::write(&storage_path, &bytes)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let proxy_path = format!("/api/v1/uploads/{upload_id}/download");
    sqlx::query(
        r#"
        INSERT INTO issue_uploads (
            id,
            project_id,
            issue_id,
            uploaded_by_user_id,
            filename,
            content_type,
            byte_size,
            storage_path,
            proxy_path
        )
        VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(upload_id)
    .bind(project_id)
    .bind(current_user.id)
    .bind(&file_name)
    .bind(&content_type)
    .bind(bytes.len() as i64)
    .bind(path_to_string(&storage_path))
    .bind(&proxy_path)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let markdown = if content_type.starts_with("image/") {
        format!("![{}]({})", file_name, proxy_path)
    } else {
        format!("[{}]({})", file_name, proxy_path)
    };

    Ok((
        StatusCode::CREATED,
        Json(IssueUploadDto {
            upload_id,
            filename: file_name,
            content_type,
            byte_size: bytes.len() as i64,
            proxy_path,
            markdown,
        }),
    ))
}

pub async fn download_issue_upload(
    Path(upload_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let upload = sqlx::query_as::<_, IssueUploadRow>(
        r#"
        SELECT id, project_id, issue_id, uploaded_by_user_id, filename, content_type, byte_size, storage_path, proxy_path
        FROM issue_uploads
        WHERE id = $1
          AND consumed_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(upload_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Upload not found"))?;

    if let Some(issue_id) = upload.issue_id {
        let _issue = fetch_authorized_issue(&state, issue_id, &current_user).await?;
    } else {
        ensure_project_issue_write_access(&state, upload.project_id, &current_user).await?;
        if !current_user.is_admin && upload.uploaded_by_user_id != current_user.id {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "Upload not found"));
        }
    }

    let bytes = fs::read(&upload.storage_path)
        .await
        .map_err(|error| ApiError::new(StatusCode::NOT_FOUND, error.to_string()))?;

    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&upload.content_type)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("inline; filename=\"{}\"", upload.filename))
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );

    Ok(response)
}

pub async fn delete_issue_upload(
    Path(upload_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let upload = sqlx::query_as::<_, IssueUploadRow>(
        r#"
        SELECT id, project_id, issue_id, uploaded_by_user_id, filename, content_type, byte_size, storage_path, proxy_path
        FROM issue_uploads
        WHERE id = $1
          AND consumed_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(upload_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Upload not found"))?;

    if let Some(issue_id) = upload.issue_id {
        let _issue = fetch_authorized_issue_for_comment(&state, issue_id, &current_user).await?;
    } else {
        ensure_project_issue_write_access(&state, upload.project_id, &current_user).await?;
    }

    if !current_user.is_admin && upload.uploaded_by_user_id != current_user.id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Only the original uploader or an admin can delete this upload",
        ));
    }

    sqlx::query(
        r#"
        DELETE FROM issue_uploads
        WHERE id = $1
          AND consumed_at IS NULL
        "#,
    )
    .bind(upload_id)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    match fs::remove_file(&upload.storage_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())),
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_issue_access(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<IssueAccessOverviewDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_issue_manage_access(&state, issue_id, &current_user).await?;
    ensure_issue_exists(&state, issue_id).await?;

    let permissions = sqlx::query_as::<_, IssuePermissionRow>(
        r#"
        SELECT issue_id, subject_type, subject_id, permission, effect
        FROM issue_permissions
        WHERE issue_id = $1
          AND subject_type = 'user'
          AND effect = 'allow'
        ORDER BY created_at ASC
        "#,
    )
    .bind(issue_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let assigned_user_ids = permissions
        .iter()
        .map(|permission| permission.subject_id.clone())
        .collect::<HashSet<_>>();

    let users = sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        FROM users
        WHERE active = TRUE
        ORDER BY full_name ASC, email ASC
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let assignments = users
        .iter()
        .filter_map(|user| {
            permissions
                .iter()
                .find(|permission| permission.subject_id == user.id.to_string())
                .map(|permission| IssueAccessAssignmentDto {
                    user_id: user.id,
                    email: user.email.clone(),
                    full_name: user.full_name.clone(),
                    permission: permission.permission.clone(),
                })
        })
        .collect();

    let available_users = users
        .into_iter()
        .filter(|user| !assigned_user_ids.contains(&user.id.to_string()))
        .map(|user| IssueAccessUserOptionDto {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
        })
        .collect();

    Ok(Json(IssueAccessOverviewDto {
        assignments,
        available_users,
    }))
}

pub async fn update_issue_access(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateIssueAccessRequest>,
) -> ApiResult<Json<IssueAccessOverviewDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_issue_manage_access(&state, issue_id, &current_user).await?;
    ensure_issue_exists(&state, issue_id).await?;

    let mut seen = HashSet::new();
    for assignment in &request.assignments {
        if !MANAGEABLE_PERMISSIONS.contains(&assignment.permission.as_str()) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Permission must be one of read, comment, edit, admin",
            ));
        }

        if !seen.insert(assignment.user_id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Each user can only be assigned once per issue",
            ));
        }
    }

    let requested_user_ids = request
        .assignments
        .iter()
        .map(|assignment| assignment.user_id)
        .collect::<Vec<_>>();

    if !requested_user_ids.is_empty() {
        let matched_users = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM users
            WHERE active = TRUE
              AND id = ANY($1)
            "#,
        )
        .bind(&requested_user_ids)
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?;

        if matched_users.len() != requested_user_ids.len() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "One or more selected users do not exist or are inactive",
            ));
        }
    }

    let mut tx = state.pool.begin().await.map_err(internal_error)?;

    sqlx::query(
        r#"
        DELETE FROM issue_permissions
        WHERE issue_id = $1
          AND subject_type = 'user'
        "#,
    )
    .bind(issue_id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    for assignment in &request.assignments {
        sqlx::query(
            r#"
            INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
            VALUES ($1, 'user', $2, $3, 'allow')
            "#,
        )
        .bind(issue_id)
        .bind(assignment.user_id.to_string())
        .bind(&assignment.permission)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    }

    tx.commit().await.map_err(internal_error)?;

    get_issue_access(Path(issue_id), State(state), headers).await
}

pub async fn remove_issue_permission(
    Path((issue_id, subject_type, subject_id)): Path<(Uuid, String, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_issue_manage_access(&state, issue_id, &current_user).await?;

    if subject_type != "user" && subject_type != "email" {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Subject type must be user or email"));
    }

    let deleted = sqlx::query(
        r#"
        DELETE FROM issue_permissions
        WHERE issue_id = $1
          AND subject_type = $2
          AND subject_id = $3
        "#,
    )
    .bind(issue_id)
    .bind(&subject_type)
    .bind(&subject_id)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if deleted.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Permission not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync_issue_comments(
    Path(issue_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<GitLabCommentImportResponse>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_issue_sync_access(&state, issue_id, &current_user).await?;

    let issue = fetch_issue_by_id(&state, issue_id).await?;
    let response = sync_issue_comments_for_issue(&state, &issue).await?;

    Ok(Json(response))
}

pub(crate) async fn sync_issue_comments_for_issue(
    state: &AppState,
    issue: &IssueRow,
) -> ApiResult<GitLabCommentImportResponse> {
    let issue_id = issue.id;

    if issue.gitlab_issue_iid <= 0 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Issue is not linked to GitLab"));
    }

    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(issue.project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "GitLab integration not found"))?;

    if !integration.sync_enabled {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "GitLab integration exists but sync is disabled",
        ));
    }

    let encrypted_token = integration
        .token_encrypted
        .as_ref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

    let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let imported_comments = import_issue_comments(
        GitLabIssueImportInput {
            gitlab_api_base_url: integration.gitlab_api_base_url,
            gitlab_project_id: integration.gitlab_project_id,
            token,
            verify_tls: integration.verify_tls,
        },
        issue.gitlab_issue_iid,
    )
    .await
    .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

    let existing_note_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT gitlab_note_id
        FROM issue_comments
        WHERE issue_id = $1
        "#,
    )
    .bind(issue_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .into_iter()
    .collect::<HashSet<_>>();

    let mut created_count = 0usize;
    let mut updated_count = 0usize;
    let mut last_comment_activity = issue.updated_at;

    for comment in &imported_comments {
        if existing_note_ids.contains(&comment.note_id) {
            updated_count += 1;
        } else {
            created_count += 1;
        }

        if comment.updated_at > last_comment_activity {
            last_comment_activity = comment.updated_at;
        }

        persist_gitlab_comment_and_attachments(
            state.pool.as_ref(),
            issue_id,
            &integration.gitlab_base_url,
            comment,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
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
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(GitLabCommentImportResponse {
        imported_count: imported_comments.len(),
        created_count,
        updated_count,
    })
}

async fn fetch_authorized_issue(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<IssueRow> {
    let issue = if current_user.is_admin {
        sqlx::query_as::<_, IssueRow>(
            r#"
            SELECT
                issues.id,
                issues.project_id,
                projects.slug AS project_slug,
                projects.name AS project_name,
                COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                issues.title,
                issues.description,
                issues.state,
                issues.sync_state,
                issues.last_source,
                issues.version,
                issues.created_at,
                issues.updated_at
            FROM issues
            JOIN projects ON projects.id = issues.project_id
            WHERE issues.id = $1
            "#,
        )
        .bind(issue_id)
        .fetch_optional(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, IssueRow>(
            r#"
            SELECT DISTINCT
                issues.id,
                issues.project_id,
                projects.slug AS project_slug,
                projects.name AS project_name,
                COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                issues.title,
                issues.description,
                issues.state,
                issues.sync_state,
                issues.last_source,
                issues.version,
                issues.created_at,
                issues.updated_at
            FROM issues
            JOIN projects ON projects.id = issues.project_id
            LEFT JOIN project_permissions
              ON project_permissions.project_id = issues.project_id
             AND project_permissions.effect = 'allow'
             AND (
               (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $2)
               OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $4)
             )
             AND project_permissions.permission = ANY($3)
            LEFT JOIN issue_permissions
              ON issue_permissions.issue_id = issues.id
             AND issue_permissions.subject_type = 'user'
             AND issue_permissions.subject_id = $2
             AND issue_permissions.effect = 'allow'
             AND issue_permissions.permission = ANY($5)
            WHERE issues.id = $1
              AND (
                project_permissions.project_id IS NOT NULL
                OR issue_permissions.issue_id IS NOT NULL
              )
            "#,
        )
        .bind(issue_id)
        .bind(current_user.id.to_string())
        .bind(["view", "admin"].as_slice())
        .bind(current_user.email.clone())
        .bind(READ_PERMISSIONS.as_slice())
        .fetch_optional(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    };

    issue.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Issue not found"))
}

async fn ensure_project_issue_write_access(
    state: &AppState,
    project_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    if current_user.is_admin {
        return Ok(());
    }

    if !has_project_permission(
        state.pool.as_ref(),
        project_id,
        current_user,
        &["create_issue", "admin"],
    )
    .await?
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Project create-issue permission required",
        ));
    }

    Ok(())
}

async fn has_project_permission(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    current_user: &crate::dto::UserDto,
    permissions: &[&str],
) -> ApiResult<bool> {
    let project_permission = sqlx::query_scalar::<_, String>(
        r#"
        SELECT permission
        FROM project_permissions
        WHERE project_id = $1
          AND effect = 'allow'
          AND (
            (subject_type = 'user' AND subject_id = $2)
            OR (subject_type = 'email' AND subject_id = $3)
          )
          AND permission = ANY($4)
        LIMIT 1
        "#,
    )
    .bind(project_id)
    .bind(current_user.id.to_string())
    .bind(current_user.email.clone())
    .bind(permissions)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?;

    Ok(project_permission.is_some())
}

async fn has_issue_permission(
    pool: &sqlx::PgPool,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
    permissions: &[&str],
) -> ApiResult<bool> {
    let issue_permission = sqlx::query_scalar::<_, String>(
        r#"
        SELECT permission
        FROM issue_permissions
        WHERE issue_id = $1
          AND subject_type = 'user'
          AND subject_id = $2
          AND effect = 'allow'
          AND permission = ANY($3)
        LIMIT 1
        "#,
    )
    .bind(issue_id)
    .bind(current_user.id.to_string())
    .bind(permissions)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?;

    Ok(issue_permission.is_some())
}

async fn ensure_issue_manage_access(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    if current_user.is_admin {
        return Ok(());
    }

    let issue = fetch_issue_by_id(state, issue_id).await?;
    let can_manage = has_issue_permission(state.pool.as_ref(), issue_id, current_user, &["admin"]).await?
        || has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["admin"]).await?;

    if !can_manage {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Issue admin permission required"));
    }

    Ok(())
}

async fn ensure_issue_edit_access(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    if current_user.is_admin {
        return Ok(());
    }

    let issue = fetch_issue_by_id(state, issue_id).await?;
    let can_edit = has_issue_permission(state.pool.as_ref(), issue_id, current_user, EDIT_PERMISSIONS.as_slice()).await?
        || has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["admin"]).await?;

    if !can_edit {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Issue edit permission required"));
    }

    Ok(())
}

async fn ensure_issue_state_access(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    ensure_issue_edit_access(state, issue_id, current_user).await
}

async fn ensure_issue_sync_access(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    ensure_issue_manage_access(state, issue_id, current_user).await
}

async fn issue_capabilities_for_user(
    state: &AppState,
    issue: &IssueRow,
    current_user: &crate::dto::UserDto,
) -> ApiResult<IssueCapabilitiesDto> {
    if current_user.is_admin {
        let sync_enabled = has_active_gitlab_sync(state.pool.as_ref(), issue.project_id).await?;
        return Ok(IssueCapabilitiesDto {
            can_view: true,
            can_comment: true,
            can_edit: true,
            can_change_state: true,
            can_manage_access: true,
            can_sync_comments: sync_enabled,
        });
    }

    let project_can_view =
        has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["view", "admin"]).await?;
    let issue_can_view =
        has_issue_permission(state.pool.as_ref(), issue.id, current_user, READ_PERMISSIONS.as_slice()).await?;
    let can_comment =
        has_issue_permission(state.pool.as_ref(), issue.id, current_user, COMMENT_PERMISSIONS.as_slice()).await?
            || has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["admin"]).await?;
    let can_edit = has_issue_permission(state.pool.as_ref(), issue.id, current_user, EDIT_PERMISSIONS.as_slice()).await?
        || has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["admin"]).await?;
    let can_manage_access =
        has_issue_permission(state.pool.as_ref(), issue.id, current_user, &["admin"]).await?
            || has_project_permission(state.pool.as_ref(), issue.project_id, current_user, &["admin"]).await?;
    let sync_enabled = has_active_gitlab_sync(state.pool.as_ref(), issue.project_id).await?;

    Ok(IssueCapabilitiesDto {
        can_view: project_can_view || issue_can_view,
        can_comment,
        can_edit,
        can_change_state: can_edit,
        can_manage_access,
        can_sync_comments: can_manage_access && sync_enabled,
    })
}

async fn has_active_gitlab_sync(pool: &sqlx::PgPool, project_id: Uuid) -> ApiResult<bool> {
    let sync_enabled = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .unwrap_or(false);

    Ok(sync_enabled)
}

async fn fetch_authorized_issue_for_comment(
    state: &AppState,
    issue_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<IssueRow> {
    let issue = if current_user.is_admin {
        fetch_issue_by_id(state, issue_id).await?
    } else {
        sqlx::query_as::<_, IssueRow>(
            r#"
            SELECT DISTINCT
                issues.id,
                issues.project_id,
                projects.slug AS project_slug,
                projects.name AS project_name,
                COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
                issues.title,
                issues.description,
                issues.state,
                issues.sync_state,
                issues.last_source,
                issues.version,
                issues.created_at,
                issues.updated_at
            FROM issues
            JOIN projects ON projects.id = issues.project_id
            LEFT JOIN project_permissions
              ON project_permissions.project_id = issues.project_id
             AND project_permissions.effect = 'allow'
             AND (
               (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $2)
               OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $4)
             )
             AND project_permissions.permission = 'admin'
            LEFT JOIN issue_permissions
              ON issue_permissions.issue_id = issues.id
             AND issue_permissions.subject_type = 'user'
             AND issue_permissions.subject_id = $2
             AND issue_permissions.effect = 'allow'
             AND issue_permissions.permission = ANY($3)
            WHERE issues.id = $1
              AND (
                project_permissions.project_id IS NOT NULL
                OR issue_permissions.issue_id IS NOT NULL
              )
            "#,
        )
        .bind(issue_id)
        .bind(current_user.id.to_string())
        .bind(COMMENT_PERMISSIONS.as_slice())
        .bind(current_user.email.clone())
        .fetch_optional(state.pool.as_ref())
        .await
        .map_err(internal_error)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Issue not found"))?
    };

    Ok(issue)
}

async fn ensure_issue_exists(state: &AppState, issue_id: Uuid) -> ApiResult<()> {
    fetch_issue_by_id(state, issue_id).await.map(|_| ())
}

async fn insert_issue_row(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    gitlab_issue_iid: Option<i64>,
    title: &str,
    description: &str,
    state: &str,
    sync_state: &str,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> ApiResult<IssueRow> {
    sqlx::query_as::<_, IssueRow>(
        r#"
        INSERT INTO issues (
            project_id,
            gitlab_issue_iid,
            title,
            description,
            state,
            created_at,
            updated_at,
            sync_state,
            last_source,
            version
        )
        SELECT
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            'bridge',
            1
        FROM projects
        WHERE projects.id = $1
        RETURNING
            issues.id,
            issues.project_id,
            (SELECT slug FROM projects WHERE id = issues.project_id) AS project_slug,
            (SELECT name FROM projects WHERE id = issues.project_id) AS project_name,
            COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
            issues.title,
            issues.description,
            issues.state,
            issues.sync_state,
            issues.last_source,
            issues.version,
            issues.created_at,
            issues.updated_at
        "#,
    )
    .bind(project_id)
    .bind(gitlab_issue_iid)
    .bind(title)
    .bind(description)
    .bind(state)
    .bind(created_at)
    .bind(updated_at)
    .bind(sync_state)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Project not found"))
}

async fn update_issue_row(
    pool: &sqlx::PgPool,
    issue_id: Uuid,
    title: &str,
    description: &str,
    state: &str,
    sync_state: &str,
    last_source: &str,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> ApiResult<IssueRow> {
    sqlx::query_as::<_, IssueRow>(
        r#"
        UPDATE issues
        SET title = $2,
            description = $3,
            state = $4,
            sync_state = $5,
            last_source = $6,
            updated_at = $7,
            version = version + 1
        WHERE id = $1
        RETURNING
            issues.id,
            issues.project_id,
            (SELECT slug FROM projects WHERE id = issues.project_id) AS project_slug,
            (SELECT name FROM projects WHERE id = issues.project_id) AS project_name,
            COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
            issues.title,
            issues.description,
            issues.state,
            issues.sync_state,
            issues.last_source,
            issues.version,
            issues.created_at,
            issues.updated_at
        "#,
    )
    .bind(issue_id)
    .bind(title)
    .bind(description)
    .bind(state)
    .bind(sync_state)
    .bind(last_source)
    .bind(updated_at)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Issue not found"))
}

async fn fetch_issue_by_id(state: &AppState, issue_id: Uuid) -> ApiResult<IssueRow> {
    sqlx::query_as::<_, IssueRow>(
        r#"
        SELECT
            issues.id,
            issues.project_id,
            projects.slug AS project_slug,
            projects.name AS project_name,
            COALESCE(issues.gitlab_issue_iid, 0) AS gitlab_issue_iid,
            issues.title,
            issues.description,
            issues.state,
            issues.sync_state,
            issues.last_source,
            issues.version,
            issues.created_at,
            issues.updated_at
        FROM issues
        JOIN projects ON projects.id = issues.project_id
        WHERE issues.id = $1
        "#,
    )
    .bind(issue_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Issue not found"))
}

fn normalize_issue_state(value: &str) -> ApiResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "open" | "opened" => Ok("open".to_string()),
        "closed" => Ok("closed".to_string()),
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Issue state must be open or closed",
        )),
    }
}

fn normalized_existing_issue_state(value: &str) -> String {
    normalize_issue_state(value).unwrap_or_else(|_| value.to_ascii_lowercase())
}

fn content_type_from_filename(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

fn enforce_upload_rate_limit(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let limits = &state.config.rate_limits;
    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.uploads_per_user,
        format!("upload:user:{}", user_id),
    )
}

async fn read_validated_upload_field(
    mut field: Field<'_>,
    state: &AppState,
) -> ApiResult<(String, String, Vec<u8>)> {
    let file_name = sanitize_filename(field.file_name().unwrap_or("attachment.bin"));
    let content_type = field
        .content_type()
        .map(normalize_content_type)
        .unwrap_or_else(|| content_type_from_filename(&file_name).to_string());

    ensure_upload_content_type_allowed(&content_type, state)?;

    let mut bytes = Vec::new();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?
    {
        if bytes.len().saturating_add(chunk.len()) > state.config.uploads.max_bytes {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("File exceeds maximum upload size of {} bytes", state.config.uploads.max_bytes),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok((file_name, content_type, bytes))
}

fn normalize_content_type(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}

fn ensure_upload_content_type_allowed(content_type: &str, state: &AppState) -> ApiResult<()> {
    let normalized = normalize_content_type(content_type);
    let allowed = &state.config.uploads.allowed_content_types;

    if allowed.iter().any(|item| item == "*" || item == &normalized) {
        return Ok(());
    }

    Err(ApiError::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        format!("Upload content type '{normalized}' is not allowed"),
    ))
}

fn attachment_uploads_dir(state: &AppState) -> PathBuf {
    FsPath::new(&state.config.attachments_dir).join("uploads")
}

fn attachment_store_dir(state: &AppState) -> PathBuf {
    FsPath::new(&state.config.attachments_dir).join("attachments")
}

fn path_to_string(path: &FsPath) -> String {
    path.to_string_lossy().to_string()
}

fn sanitize_filename(filename: &str) -> String {
    let basename = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("attachment.bin");
    let mut sanitized = basename
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '_') {
                char
            } else if char.is_ascii_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>();

    while sanitized.contains("..") {
        sanitized = sanitized.replace("..", ".");
    }

    let sanitized = sanitized
        .trim_matches(|char| matches!(char, '.' | '-' | '_'))
        .chars()
        .take(120)
        .collect::<String>();

    if sanitized.is_empty() {
        "attachment.bin".to_string()
    } else {
        sanitized
    }
}

async fn fetch_referenced_uploads(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    issue_id: Option<Uuid>,
    user_id: Uuid,
    body: &str,
) -> ApiResult<Vec<IssueUploadRow>> {
    let upload_ids = extract_local_upload_ids(body);
    if upload_ids.is_empty() {
        return Ok(Vec::new());
    }

    let uploads = if let Some(issue_id) = issue_id {
        sqlx::query_as::<_, IssueUploadRow>(
            r#"
            SELECT id, project_id, issue_id, uploaded_by_user_id, filename, content_type, byte_size, storage_path, proxy_path
            FROM issue_uploads
            WHERE project_id = $1
              AND issue_id = $2
              AND uploaded_by_user_id = $3
              AND consumed_at IS NULL
              AND id = ANY($4)
            "#,
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(user_id)
        .bind(&upload_ids)
        .fetch_all(pool)
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, IssueUploadRow>(
            r#"
            SELECT id, project_id, issue_id, uploaded_by_user_id, filename, content_type, byte_size, storage_path, proxy_path
            FROM issue_uploads
            WHERE project_id = $1
              AND issue_id IS NULL
              AND uploaded_by_user_id = $2
              AND consumed_at IS NULL
              AND id = ANY($3)
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .bind(&upload_ids)
        .fetch_all(pool)
        .await
        .map_err(internal_error)?
    };

    Ok(uploads)
}

fn extract_local_upload_ids(body: &str) -> Vec<Uuid> {
    let marker = "/api/v1/uploads/";
    let mut result = Vec::new();
    let mut rest = body;

    while let Some(index) = rest.find(marker) {
        let after = &rest[index + marker.len()..];
        if let Some(end) = after.find("/download") {
            let raw_id = &after[..end];
            if let Ok(upload_id) = Uuid::parse_str(raw_id) {
                result.push(upload_id);
            }
            rest = &after[end + "/download".len()..];
        } else {
            break;
        }
    }

    result.sort();
    result.dedup();
    result
}

async fn replace_temp_uploads_with_gitlab_uploads(
    body: &str,
    uploads: &[IssueUploadRow],
    integration: &ProjectIntegrationRow,
    token: String,
) -> ApiResult<String> {
    let mut rewritten = body.to_string();

    for upload in uploads {
        let uploaded = upload_project_attachment(GitLabUploadAttachmentInput {
            gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
            gitlab_project_id: integration.gitlab_project_id,
            token: token.clone(),
            verify_tls: integration.verify_tls,
            file_path: upload.storage_path.clone(),
            filename: upload.filename.clone(),
            content_type: upload.content_type.clone(),
        })
        .await
        .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

        rewritten = rewritten.replace(&upload.proxy_path, &uploaded.url);
    }

    Ok(rewritten)
}

async fn next_local_note_id(pool: &sqlx::PgPool, issue_id: Uuid) -> ApiResult<i64> {
    let min_note_id = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT MIN(gitlab_note_id)
        FROM issue_comments
        WHERE issue_id = $1
        "#,
    )
    .bind(issue_id)
    .fetch_one(pool)
    .await
    .map_err(internal_error)?;

    Ok(match min_note_id {
        Some(value) if value < 0 => value - 1,
        _ => -1,
    })
}

async fn persist_local_comment_attachments(
    pool: &sqlx::PgPool,
    issue_id: Uuid,
    user_id: Uuid,
    user_name: &str,
    body: &str,
    uploads: &[IssueUploadRow],
    state: &AppState,
) -> ApiResult<(String, Vec<crate::dto::AttachmentDto>)> {
    let mut rewritten_body = body.to_string();
    let mut attachment_dtos = Vec::new();

    if uploads.is_empty() {
        return Ok((rewritten_body, attachment_dtos));
    }

    let store_dir = attachment_store_dir(state);
    fs::create_dir_all(&store_dir)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    for upload in uploads {
        let attachment_id = Uuid::new_v4();
        let destination = store_dir.join(format!("{}_{}", attachment_id, upload.filename));
        fs::rename(&upload.storage_path, &destination)
            .await
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        let proxy_path = format!("/api/v1/attachments/{attachment_id}/download");
        rewritten_body = rewritten_body.replace(&upload.proxy_path, &proxy_path);

        sqlx::query(
            r#"
            INSERT INTO issue_attachments (
                id,
                issue_id,
                comment_id,
                filename,
                content_type,
                byte_size,
                external_url,
                proxy_path,
                storage_backend,
                storage_path,
                cache_state,
                cached_at,
                last_cache_error,
                inline,
                created_by_external_id,
                sync_state
            )
            VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, 'local', $8, 'local_authoritative', NOW(), NULL, $9, $10, 'local')
            "#,
        )
        .bind(attachment_id)
        .bind(issue_id)
        .bind(&upload.filename)
        .bind(&upload.content_type)
        .bind(upload.byte_size)
        .bind(&proxy_path)
        .bind(&proxy_path)
        .bind(path_to_string(&destination))
        .bind(upload.content_type.starts_with("image/"))
        .bind(format!("issuehub:user:{}:{user_name}", user_id))
        .execute(pool)
        .await
        .map_err(internal_error)?;

        attachment_dtos.push(crate::dto::AttachmentDto {
            id: attachment_id,
            filename: upload.filename.clone(),
            content_type: upload.content_type.clone(),
            byte_size: upload.byte_size,
            external_url: proxy_path.clone(),
            proxy_path,
            inline: upload.content_type.starts_with("image/"),
            sync_state: "local".to_string(),
        });
    }

    Ok((rewritten_body, attachment_dtos))
}

async fn persist_local_issue_attachments(
    pool: &sqlx::PgPool,
    issue_id: Uuid,
    user_id: Uuid,
    user_name: &str,
    body: &str,
    uploads: &[IssueUploadRow],
    state: &AppState,
) -> ApiResult<String> {
    let mut rewritten_body = body.to_string();

    if uploads.is_empty() {
        return Ok(rewritten_body);
    }

    let store_dir = attachment_store_dir(state);
    fs::create_dir_all(&store_dir)
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    for upload in uploads {
        let attachment_id = Uuid::new_v4();
        let destination = store_dir.join(format!("{}_{}", attachment_id, upload.filename));
        fs::rename(&upload.storage_path, &destination)
            .await
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        let proxy_path = format!("/api/v1/attachments/{attachment_id}/download");
        rewritten_body = rewritten_body.replace(&upload.proxy_path, &proxy_path);

        sqlx::query(
            r#"
            INSERT INTO issue_attachments (
                id,
                issue_id,
                comment_id,
                filename,
                content_type,
                byte_size,
                external_url,
                proxy_path,
                storage_backend,
                storage_path,
                cache_state,
                cached_at,
                last_cache_error,
                inline,
                created_by_external_id,
                sync_state
            )
            VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, 'local', $8, 'local_authoritative', NOW(), NULL, $9, $10, 'local')
            "#,
        )
        .bind(attachment_id)
        .bind(issue_id)
        .bind(&upload.filename)
        .bind(&upload.content_type)
        .bind(upload.byte_size)
        .bind(&proxy_path)
        .bind(&proxy_path)
        .bind(path_to_string(&destination))
        .bind(upload.content_type.starts_with("image/"))
        .bind(format!("issuehub:user:{}:{user_name}", user_id))
        .execute(pool)
        .await
        .map_err(internal_error)?;
    }

    Ok(rewritten_body)
}

async fn attach_local_attachments_to_comment(
    pool: &sqlx::PgPool,
    comment_id: Uuid,
    attachments: &[crate::dto::AttachmentDto],
) -> ApiResult<()> {
    for attachment in attachments {
        sqlx::query(
            r#"
            UPDATE issue_attachments
            SET comment_id = $2
            WHERE id = $1
            "#,
        )
        .bind(attachment.id)
        .bind(comment_id)
        .execute(pool)
        .await
        .map_err(internal_error)?;
    }

    Ok(())
}

async fn cleanup_consumed_uploads(pool: &sqlx::PgPool, uploads: &[IssueUploadRow]) -> ApiResult<()> {
    for upload in uploads {
        sqlx::query(
            r#"
            UPDATE issue_uploads
            SET consumed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(upload.id)
        .execute(pool)
        .await
        .map_err(internal_error)?;
    }

    Ok(())
}

async fn fetch_comment_dto(
    pool: &sqlx::PgPool,
    comment_id: Uuid,
) -> ApiResult<CommentDto> {
    let comment = sqlx::query_as::<_, CommentRow>(
        r#"
        SELECT id, issue_id, gitlab_note_id, discussion_id, individual_note, reply_to_gitlab_note_id, author_external_id, author_name, body_raw, system_note, sync_state, created_at, updated_at
        FROM issue_comments
        WHERE id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_one(pool)
    .await
    .map_err(internal_error)?;

    let attachments = sqlx::query_as::<_, AttachmentRow>(
        r#"
        SELECT id, issue_id, comment_id, filename, content_type, byte_size, external_url, proxy_path, storage_backend, storage_path, inline, created_by_external_id, sync_state, created_at
        FROM issue_attachments
        WHERE comment_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(comment_id)
    .fetch_all(pool)
    .await
    .map_err(internal_error)?;

    Ok(CommentDto {
        id: comment_id,
        gitlab_note_id: comment.gitlab_note_id,
        discussion_id: comment.discussion_id,
        individual_note: comment.individual_note,
        reply_to_gitlab_note_id: comment.reply_to_gitlab_note_id,
        author_external_id: comment.author_external_id,
        author_name: comment.author_name,
        body_raw: comment.body_raw,
        system_note: comment.system_note,
        sync_state: comment.sync_state,
        attachments: attachments.iter().map(attachment_to_dto).collect(),
        created_at: comment.created_at,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bridge_core::config::AppConfig;
    use chrono::Utc;
    use sqlx::PgPool;

    use super::*;
    use crate::{dto::UserDto, services::rate_limit::RateLimiter};

    #[sqlx::test(migrations = false)]
    async fn permission_matrix_keeps_project_create_issue_from_granting_visibility(pool: PgPool) {
        setup_schema(&pool).await;
        let state = test_state(pool.clone());
        let project_id = seed_project(&pool, "phase3-permission-matrix").await;
        let issue = seed_issue(&pool, project_id, "Permission matrix issue").await;

        let admin = seed_user(&pool, "phase3-admin@example.invalid", true).await;
        let project_view = seed_user(&pool, "phase3-project-view@example.invalid", false).await;
        let project_create = seed_user(&pool, "phase3-project-create@example.invalid", false).await;
        let project_admin = seed_user(&pool, "phase3-project-admin@example.invalid", false).await;
        let issue_read = seed_user(&pool, "phase3-issue-read@example.invalid", false).await;
        let issue_comment = seed_user(&pool, "phase3-issue-comment@example.invalid", false).await;
        let issue_edit = seed_user(&pool, "phase3-issue-edit@example.invalid", false).await;
        let issue_admin = seed_user(&pool, "phase3-issue-admin@example.invalid", false).await;

        grant_project_permission(&pool, project_id, &project_view, "view").await;
        grant_project_permission(&pool, project_id, &project_create, "create_issue").await;
        grant_project_permission(&pool, project_id, &project_admin, "admin").await;
        grant_issue_permission(&pool, issue.id, &issue_read, "read").await;
        grant_issue_permission(&pool, issue.id, &issue_comment, "comment").await;
        grant_issue_permission(&pool, issue.id, &issue_edit, "edit").await;
        grant_issue_permission(&pool, issue.id, &issue_admin, "admin").await;

        let admin_caps = issue_capabilities_for_user(&state, &issue, &admin).await.unwrap();
        assert!(admin_caps.can_view);
        assert!(admin_caps.can_comment);
        assert!(admin_caps.can_edit);
        assert!(admin_caps.can_change_state);
        assert!(admin_caps.can_manage_access);

        let view_caps = issue_capabilities_for_user(&state, &issue, &project_view).await.unwrap();
        assert!(view_caps.can_view);
        assert!(!view_caps.can_comment);
        assert!(!view_caps.can_edit);
        assert!(!view_caps.can_change_state);
        assert!(!view_caps.can_manage_access);

        ensure_project_issue_write_access(&state, project_id, &project_create)
            .await
            .unwrap();
        let create_caps = issue_capabilities_for_user(&state, &issue, &project_create)
            .await
            .unwrap();
        assert!(!create_caps.can_view);
        assert!(!create_caps.can_comment);
        assert!(!create_caps.can_edit);
        assert!(!create_caps.can_manage_access);

        let project_admin_caps = issue_capabilities_for_user(&state, &issue, &project_admin)
            .await
            .unwrap();
        assert!(project_admin_caps.can_view);
        assert!(project_admin_caps.can_comment);
        assert!(project_admin_caps.can_edit);
        assert!(project_admin_caps.can_change_state);
        assert!(project_admin_caps.can_manage_access);

        let read_caps = issue_capabilities_for_user(&state, &issue, &issue_read)
            .await
            .unwrap();
        assert!(read_caps.can_view);
        assert!(!read_caps.can_comment);
        assert!(!read_caps.can_edit);
        assert!(!read_caps.can_manage_access);

        let comment_caps = issue_capabilities_for_user(&state, &issue, &issue_comment)
            .await
            .unwrap();
        assert!(comment_caps.can_view);
        assert!(comment_caps.can_comment);
        assert!(!comment_caps.can_edit);
        assert!(!comment_caps.can_change_state);
        assert!(!comment_caps.can_manage_access);

        let edit_caps = issue_capabilities_for_user(&state, &issue, &issue_edit)
            .await
            .unwrap();
        assert!(edit_caps.can_view);
        assert!(edit_caps.can_comment);
        assert!(edit_caps.can_edit);
        assert!(edit_caps.can_change_state);
        assert!(!edit_caps.can_manage_access);

        let issue_admin_caps = issue_capabilities_for_user(&state, &issue, &issue_admin)
            .await
            .unwrap();
        assert!(issue_admin_caps.can_view);
        assert!(issue_admin_caps.can_comment);
        assert!(issue_admin_caps.can_edit);
        assert!(issue_admin_caps.can_change_state);
        assert!(issue_admin_caps.can_manage_access);
    }

    #[sqlx::test(migrations = false)]
    async fn local_first_project_accepts_multiple_local_issues_comments_and_attachments(pool: PgPool) {
        setup_schema(&pool).await;
        let project_id = seed_project(&pool, "phase3-local-first").await;
        let user = seed_user(&pool, "phase3-local-user@example.invalid", false).await;

        let first = seed_issue(&pool, project_id, "First local issue").await;
        let second = seed_issue(&pool, project_id, "Second local issue").await;

        assert_eq!(first.gitlab_issue_iid, 0);
        assert_eq!(second.gitlab_issue_iid, 0);

        let local_issue_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM issues WHERE project_id = $1 AND gitlab_issue_iid IS NULL",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(local_issue_count, 2);

        let comment_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO issue_comments (
                issue_id,
                gitlab_note_id,
                author_external_id,
                author_name,
                body_raw,
                sync_state
            )
            VALUES ($1, 0, $2, $3, 'local comment', 'local')
            RETURNING id
            "#,
        )
        .bind(first.id)
        .bind(format!("issuehub:user:{}", user.id))
        .bind(&user.full_name)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO issue_attachments (
                issue_id,
                comment_id,
                filename,
                content_type,
                byte_size,
                external_url,
                proxy_path,
                storage_backend,
                storage_path,
                inline,
                created_by_external_id,
                sync_state
            )
            VALUES ($1, $2, 'local.txt', 'text/plain', 11, '/api/v1/attachments/local/download', '/api/v1/attachments/local/download', 'local', '/tmp/local.txt', FALSE, $3, 'local')
            "#,
        )
        .bind(first.id)
        .bind(comment_id)
        .bind(format!("issuehub:user:{}", user.id))
        .execute(&pool)
        .await
        .unwrap();

        let comment = fetch_comment_dto(&pool, comment_id).await.unwrap();
        assert_eq!(comment.body_raw, "local comment");
        assert_eq!(comment.attachments.len(), 1);
        assert_eq!(comment.attachments[0].filename, "local.txt");
        assert_eq!(comment.attachments[0].sync_state, "local");
    }

    #[sqlx::test(migrations = false)]
    async fn gitlab_disabled_project_keeps_core_capabilities_local(pool: PgPool) {
        setup_schema(&pool).await;
        let state = test_state(pool.clone());
        let project_id = seed_project(&pool, "phase3-gitlab-disabled").await;
        let issue = seed_issue(&pool, project_id, "GitLab disabled local issue").await;
        let issue_admin = seed_user(&pool, "phase3-local-admin@example.invalid", false).await;
        grant_issue_permission(&pool, issue.id, &issue_admin, "admin").await;

        let caps = issue_capabilities_for_user(&state, &issue, &issue_admin)
            .await
            .unwrap();
        assert!(caps.can_view);
        assert!(caps.can_comment);
        assert!(caps.can_edit);
        assert!(caps.can_change_state);
        assert!(caps.can_manage_access);
        assert!(!caps.can_sync_comments);

        assert!(!has_active_gitlab_sync(&pool, project_id).await.unwrap());
    }

    #[sqlx::test(migrations = false)]
    async fn gitlab_attachment_sync_preserves_cache_metadata_and_local_attachments(pool: PgPool) {
        setup_schema(&pool).await;
        let project_id = seed_project(&pool, "attachment-cache-preserve").await;
        let issue = seed_issue(&pool, project_id, "Attachment cache issue").await;
        let user = seed_user(&pool, "attachment-cache-user@example.invalid", false).await;

        sqlx::query(
            r#"
            INSERT INTO issue_attachments (
                issue_id,
                comment_id,
                filename,
                content_type,
                byte_size,
                external_url,
                proxy_path,
                storage_backend,
                storage_path,
                cache_state,
                cached_at,
                inline,
                created_by_external_id,
                sync_state
            )
            VALUES ($1, NULL, 'local.txt', 'text/plain', 12, '/api/v1/attachments/local/download', '/api/v1/attachments/local/download', 'local', '/tmp/local.txt', 'local_authoritative', NOW(), FALSE, $2, 'local')
            "#,
        )
        .bind(issue.id)
        .bind(format!("issuehub:user:{}", user.id))
        .execute(&pool)
        .await
        .unwrap();

        let body = "![diagram](/uploads/secret/cache.png)";
        persist_gitlab_issue_attachments(&pool, issue.id, "https://gitlab.example.test", body)
            .await
            .unwrap();

        let first_row = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT id, proxy_path
            FROM issue_attachments
            WHERE issue_id = $1
              AND storage_backend = 'gitlab'
              AND external_url = 'https://gitlab.example.test/uploads/secret/cache.png'
            "#,
        )
        .bind(issue.id)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            UPDATE issue_attachments
            SET storage_path = '/tmp/gitlab-cache/cache.png',
                byte_size = 42,
                cache_state = 'cached',
                cached_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(first_row.0)
        .execute(&pool)
        .await
        .unwrap();

        persist_gitlab_issue_attachments(&pool, issue.id, "https://gitlab.example.test", body)
            .await
            .unwrap();

        let cached_row = sqlx::query_as::<_, (Uuid, String, Option<String>, i64, String)>(
            r#"
            SELECT id, proxy_path, storage_path, byte_size, cache_state
            FROM issue_attachments
            WHERE issue_id = $1
              AND storage_backend = 'gitlab'
              AND external_url = 'https://gitlab.example.test/uploads/secret/cache.png'
            "#,
        )
        .bind(issue.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cached_row.0, first_row.0);
        assert_eq!(cached_row.1, first_row.1);
        assert_eq!(cached_row.2.as_deref(), Some("/tmp/gitlab-cache/cache.png"));
        assert_eq!(cached_row.3, 42);
        assert_eq!(cached_row.4, "cached");

        let local_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM issue_attachments
            WHERE issue_id = $1
              AND storage_backend = 'local'
            "#,
        )
        .bind(issue.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(local_count, 1);
    }

    fn test_state(pool: PgPool) -> AppState {
        AppState {
            pool: Arc::new(pool),
            config: AppConfig::default(),
            rate_limiter: RateLimiter::default(),
        }
    }

    async fn setup_schema(pool: &PgPool) {
        sqlx::raw_sql(include_str!("../../../../infra/postgres/init/001-init.sql"))
            .execute(pool)
            .await
            .unwrap();
    }

    async fn seed_project(pool: &PgPool, slug: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO projects (slug, name, description)
            VALUES ($1, $2, 'phase 3 automated test project')
            RETURNING id
            "#,
        )
        .bind(slug)
        .bind(slug.replace('-', " "))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn seed_issue(pool: &PgPool, project_id: Uuid, title: &str) -> IssueRow {
        let now = Utc::now();
        insert_issue_row(pool, project_id, None, title, "local issue", "open", "local", now, now)
            .await
            .unwrap()
    }

    async fn seed_user(pool: &PgPool, email: &str, is_admin: bool) -> UserDto {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO users (email, full_name, password_hash, is_admin, active)
            VALUES ($1, $2, 'test-password-hash', $3, TRUE)
            RETURNING id
            "#,
        )
        .bind(email)
        .bind(email.split('@').next().unwrap_or("test user"))
        .bind(is_admin)
        .fetch_one(pool)
        .await
        .unwrap();

        UserDto {
            id,
            email: email.to_string(),
            full_name: email.split('@').next().unwrap_or("test user").to_string(),
            preferred_language: None,
            is_admin,
        }
    }

    async fn grant_project_permission(pool: &PgPool, project_id: Uuid, user: &UserDto, permission: &str) {
        sqlx::query(
            r#"
            INSERT INTO project_permissions (project_id, subject_type, subject_id, permission, effect)
            VALUES ($1, 'user', $2, $3, 'allow')
            "#,
        )
        .bind(project_id)
        .bind(user.id.to_string())
        .bind(permission)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn grant_issue_permission(pool: &PgPool, issue_id: Uuid, user: &UserDto, permission: &str) {
        sqlx::query(
            r#"
            INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
            VALUES ($1, 'user', $2, $3, 'allow')
            "#,
        )
        .bind(issue_id)
        .bind(user.id.to_string())
        .bind(permission)
        .execute(pool)
        .await
        .unwrap();
    }
}
