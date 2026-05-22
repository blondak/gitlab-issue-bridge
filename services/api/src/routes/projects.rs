use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, Json};
use bridge_core::{
    issue_sync::{persist_gitlab_issue_attachments, upsert_gitlab_issue_row},
    secrets::{decrypt_secret, encrypt_secret},
};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{
    dto::{
        CreateProjectRequest, GitLabIssueImportResponse, GitLabIntegrationValidationResponse,
        ProjectCapabilitiesDto,
        ProjectAccessAssignmentDto, ProjectAccessOverviewDto, ProjectAccessUserOptionDto,
        ProjectIssuePermissionDto, ProjectDto, ProjectIntegrationRow, ProjectPermissionRow,
        ProjectRow, UpdateProjectAccessRequest,
        UpdateProjectRequest, UpsertProjectGitLabIntegrationRequest, ValidateProjectGitLabIntegrationRequest,
    },
    error::{internal_error, ApiError, ApiResult},
    services::auth as auth_service,
    services::gitlab::{import_project_issues, validate_integration, GitLabIssueImportInput, GitLabValidationInput},
    state::AppState,
};

const PROJECT_PERMISSION_OPTIONS: [&str; 3] = ["view", "create_issue", "admin"];

pub async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ProjectDto>>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let projects = if current_user.is_admin {
        sqlx::query_as::<_, ProjectRow>(
            r#"
            SELECT id, slug, name, description, active
            FROM projects
            ORDER BY name ASC
            "#,
        )
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, ProjectRow>(
            r#"
            SELECT DISTINCT projects.id, projects.slug, projects.name, projects.description, projects.active
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
            ORDER BY projects.name ASC
            "#,
        )
        .bind(current_user.id.to_string())
        .bind(current_user.email.clone())
        .bind(["view", "create_issue", "admin"].as_slice())
        .bind(["read", "comment", "edit", "admin"].as_slice())
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    };

    let integrations = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let mut response = Vec::with_capacity(projects.len());
    for project in projects {
        let integration = integrations
            .iter()
            .find(|integration| integration.project_id == project.id)
            .cloned();
        let capabilities = if current_user.is_admin {
            ProjectCapabilitiesDto {
                can_view: true,
                can_create_issue: true,
                can_manage: true,
            }
        } else {
            let can_create_issue = has_project_permission(
                state.pool.as_ref(),
                project.id,
                &current_user,
                &["create_issue", "admin"],
            )
            .await?;
            let can_manage = has_project_permission(
                state.pool.as_ref(),
                project.id,
                &current_user,
                &["admin"],
            )
            .await?;

            ProjectCapabilitiesDto {
                can_view: true,
                can_create_issue,
                can_manage,
            }
        };

        response.push(ProjectDto::from_parts(project, integration, capabilities));
    }

    Ok(Json(response))
}

pub async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<ProjectDto>)> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let row = sqlx::query_as::<_, (Uuid, String, String, String, bool)>(
        r#"
        INSERT INTO projects (slug, name, description)
        VALUES ($1, $2, $3)
        RETURNING id, slug, name, description, active
        "#,
    )
    .bind(request.slug)
    .bind(request.name)
    .bind(request.description.unwrap_or_default())
    .fetch_one(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectDto {
            id: row.0,
            slug: row.1,
            name: row.2,
            description: row.3,
            active: row.4,
            gitlab_integration: None,
            capabilities: ProjectCapabilitiesDto {
                can_view: true,
                can_create_issue: true,
                can_manage: true,
            },
        }),
    ))
}

pub async fn upsert_gitlab_integration(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpsertProjectGitLabIntegrationRequest>,
) -> ApiResult<Json<ProjectDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;

    let exists: Option<(Uuid, String, String, String, bool)> = sqlx::query_as(
        r#"
        SELECT id, slug, name, description, active
        FROM projects
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let project = exists.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Project not found"))?;
    let token_encrypted = encrypt_secret(&state.config.secret_encryption_key, &request.token)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let webhook_secret_encrypted =
        encrypt_secret(&state.config.secret_encryption_key, &request.webhook_secret)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO project_gitlab_integrations (
            project_id,
            gitlab_base_url,
            gitlab_api_base_url,
            gitlab_project_id,
            token,
            webhook_secret,
            token_encrypted,
            webhook_secret_encrypted,
            verify_tls,
            sync_enabled
        )
        VALUES ($1, $2, $3, $4, '', '', $5, $6, $7, $8)
        ON CONFLICT (project_id)
        DO UPDATE SET
            gitlab_base_url = EXCLUDED.gitlab_base_url,
            gitlab_api_base_url = EXCLUDED.gitlab_api_base_url,
            gitlab_project_id = EXCLUDED.gitlab_project_id,
            token = EXCLUDED.token,
            webhook_secret = EXCLUDED.webhook_secret,
            token_encrypted = EXCLUDED.token_encrypted,
            webhook_secret_encrypted = EXCLUDED.webhook_secret_encrypted,
            verify_tls = EXCLUDED.verify_tls,
            sync_enabled = EXCLUDED.sync_enabled,
            updated_at = NOW()
        "#,
    )
    .bind(project_id)
    .bind(&request.gitlab_base_url)
    .bind(&request.gitlab_api_base_url)
    .bind(request.gitlab_project_id)
    .bind(&token_encrypted)
    .bind(&webhook_secret_encrypted)
    .bind(request.verify_tls)
    .bind(request.sync_enabled)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        INSERT INTO project_integrations (
            project_id,
            provider,
            base_url,
            api_base_url,
            external_project_id,
            token_encrypted,
            webhook_secret_encrypted,
            verify_tls,
            sync_enabled,
            settings
        )
        VALUES ($1, 'gitlab', $2, $3, $4, $5, $6, $7, $8, jsonb_build_object('legacy_table', 'project_gitlab_integrations'))
        ON CONFLICT (project_id, provider)
        DO UPDATE SET
            base_url = EXCLUDED.base_url,
            api_base_url = EXCLUDED.api_base_url,
            external_project_id = EXCLUDED.external_project_id,
            token_encrypted = EXCLUDED.token_encrypted,
            webhook_secret_encrypted = EXCLUDED.webhook_secret_encrypted,
            verify_tls = EXCLUDED.verify_tls,
            sync_enabled = EXCLUDED.sync_enabled,
            settings = EXCLUDED.settings,
            updated_at = NOW()
        "#,
    )
    .bind(project_id)
    .bind(&request.gitlab_base_url)
    .bind(&request.gitlab_api_base_url)
    .bind(request.gitlab_project_id.to_string())
    .bind(&token_encrypted)
    .bind(&webhook_secret_encrypted)
    .bind(request.verify_tls)
    .bind(request.sync_enabled)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let integration = sqlx::query_as::<_, ProjectIntegrationRow>(
        r#"
        SELECT id, project_id, gitlab_base_url, gitlab_api_base_url, gitlab_project_id, token_encrypted, webhook_secret_encrypted, verify_tls, sync_enabled
        FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .fetch_one(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(Json(ProjectDto::from_parts(
        ProjectRow {
            id: project.0,
            slug: project.1,
            name: project.2,
            description: project.3,
            active: project.4,
        },
        Some(integration),
        ProjectCapabilitiesDto {
            can_view: true,
            can_create_issue: true,
            can_manage: true,
        },
    )))
}

async fn has_project_permission(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    current_user: &crate::dto::UserDto,
    permissions: &[&str],
) -> ApiResult<bool> {
    let permission = sqlx::query_scalar::<_, String>(
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

    Ok(permission.is_some())
}

async fn ensure_project_manage_access(
    state: &AppState,
    project_id: Uuid,
    current_user: &crate::dto::UserDto,
) -> ApiResult<()> {
    if current_user.is_admin {
        return Ok(());
    }

    if !has_project_permission(state.pool.as_ref(), project_id, current_user, &["admin"]).await? {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Project admin permission required"));
    }

    Ok(())
}

pub async fn update_project(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProjectRequest>,
) -> ApiResult<Json<ProjectDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;

    let project = sqlx::query_as::<_, ProjectRow>(
        r#"
        UPDATE projects
        SET slug = $2,
            name = $3,
            description = $4,
            active = $5,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, slug, name, description, active
        "#,
    )
    .bind(project_id)
    .bind(request.slug)
    .bind(request.name)
    .bind(request.description.unwrap_or_default())
    .bind(request.active)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Project not found"))?;

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

    Ok(Json(ProjectDto::from_parts(
        project,
        integration,
        ProjectCapabilitiesDto {
            can_view: true,
            can_create_issue: true,
            can_manage: true,
        },
    )))
}

pub async fn validate_gitlab_integration(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ValidateProjectGitLabIntegrationRequest>,
) -> ApiResult<Json<GitLabIntegrationValidationResponse>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;

    let _ = &request.gitlab_base_url;
    let existing_integration = sqlx::query_as::<_, ProjectIntegrationRow>(
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

    let token = match request.token.as_deref() {
        Some(token) if !token.trim().is_empty() => token.trim().to_string(),
        _ => {
            let encrypted = existing_integration
                .as_ref()
                .and_then(|integration| integration.token_encrypted.clone())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "Token is required for validation when no saved encrypted token exists",
                    )
                })?;

            decrypt_secret(&state.config.secret_encryption_key, &encrypted)
                .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        }
    };

    let result = validate_integration(GitLabValidationInput {
        gitlab_api_base_url: request.gitlab_api_base_url,
        gitlab_project_id: request.gitlab_project_id,
        token,
        verify_tls: request.verify_tls,
    })
    .await
    .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

    Ok(Json(GitLabIntegrationValidationResponse {
        valid: true,
        project_name: result.project_name,
        web_url: result.web_url,
        visibility: result.visibility,
    }))
}

pub async fn import_gitlab_issues(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<GitLabIssueImportResponse>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;

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

    let imported_issues = import_project_issues(GitLabIssueImportInput {
        gitlab_api_base_url: integration.gitlab_api_base_url.clone(),
        gitlab_project_id: integration.gitlab_project_id,
        token,
        verify_tls: integration.verify_tls,
    })
    .await
    .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;

    let existing_iids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT gitlab_issue_iid
        FROM issues
        WHERE project_id = $1
          AND gitlab_issue_iid IS NOT NULL
        "#,
    )
    .bind(project_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .into_iter()
    .collect::<HashSet<_>>();

    let mut created_count = 0usize;
    let mut updated_count = 0usize;

    for issue in &imported_issues {
        if existing_iids.contains(&issue.iid) {
            updated_count += 1;
        } else {
            created_count += 1;
        }

        let issue_id = upsert_gitlab_issue_row(state.pool.as_ref(), project_id, issue)
            .await
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        persist_gitlab_issue_attachments(
            state.pool.as_ref(),
            issue_id,
            &integration.gitlab_base_url,
            &issue.description,
        )
        .await
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    }

    Ok(Json(GitLabIssueImportResponse {
        imported_count: imported_issues.len(),
        created_count,
        updated_count,
    }))
}

pub async fn delete_project(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let result = sqlx::query(
        r#"
        DELETE FROM projects
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Project not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_gitlab_integration(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;

    let result = sqlx::query(
        r#"
        DELETE FROM project_gitlab_integrations
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "GitLab integration not found",
        ));
    }

    sqlx::query(
        r#"
        DELETE FROM project_integrations
        WHERE project_id = $1
          AND provider = 'gitlab'
        "#,
    )
    .bind(project_id)
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_project_access(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> ApiResult<Json<ProjectAccessOverviewDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;
    ensure_project_exists(&state, project_id).await?;

    let permissions = sqlx::query_as::<_, ProjectPermissionRow>(
        r#"
        SELECT project_id, subject_type, subject_id, permission, effect
        FROM project_permissions
        WHERE project_id = $1
          AND effect = 'allow'
        ORDER BY created_at ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let users = sqlx::query_as::<_, crate::dto::UserRow>(
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

    let invitations = sqlx::query_as::<_, crate::dto::UserInvitationRow>(
        r#"
        SELECT id, email, invited_by_user_id, is_admin, status, expires_at, last_sent_at, accepted_at, created_at
        FROM user_invitations
        WHERE accepted_at IS NULL
          AND status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let assigned_keys = permissions
        .iter()
        .map(|permission| format!("{}:{}", permission.subject_type, permission.subject_id))
        .collect::<HashSet<_>>();

    let mut assignments = Vec::new();
    for permission in permissions {
        if permission.subject_type == "user" {
            if let Some(user) = users.iter().find(|user| user.id.to_string() == permission.subject_id) {
                assignments.push(ProjectAccessAssignmentDto {
                    subject_type: "user".to_string(),
                    subject_id: permission.subject_id,
                    display_name: user.full_name.clone(),
                    email: user.email.clone(),
                    permission: permission.permission,
                });
            }
        } else if permission.subject_type == "email" {
            assignments.push(ProjectAccessAssignmentDto {
                subject_type: "email".to_string(),
                subject_id: permission.subject_id.clone(),
                display_name: permission.subject_id.clone(),
                email: permission.subject_id,
                permission: permission.permission,
            });
        }
    }

    let mut available_subjects = Vec::new();
    for user in users {
        let key = format!("user:{}", user.id);
        if !assigned_keys.contains(&key) {
            available_subjects.push(ProjectAccessUserOptionDto {
                subject_type: "user".to_string(),
                subject_id: user.id.to_string(),
                display_name: user.full_name,
                email: user.email,
            });
        }
    }

    for invitation in invitations {
        let key = format!("email:{}", invitation.email);
        if !assigned_keys.contains(&key) {
            available_subjects.push(ProjectAccessUserOptionDto {
                subject_type: "email".to_string(),
                subject_id: invitation.email.clone(),
                display_name: invitation.email.clone(),
                email: invitation.email,
            });
        }
    }

    let issue_permissions = sqlx::query_as::<_, ProjectIssuePermissionDto>(
        r#"
        SELECT
            ip.issue_id,
            i.title AS issue_title,
            ip.subject_type,
            ip.subject_id,
            COALESCE(u.full_name, ip.subject_id) AS subject_display_name,
            COALESCE(u.email, ip.subject_id) AS subject_email,
            ip.permission
        FROM issue_permissions ip
        JOIN issues i ON i.id = ip.issue_id
        LEFT JOIN users u ON u.id::text = ip.subject_id AND ip.subject_type = 'user'
        WHERE i.project_id = $1
          AND ip.effect = 'allow'
        ORDER BY i.title ASC, subject_display_name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(Json(ProjectAccessOverviewDto {
        assignments,
        available_subjects,
        issue_permissions,
    }))
}

pub async fn update_project_access(
    Path(project_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<UpdateProjectAccessRequest>,
) -> ApiResult<Json<ProjectAccessOverviewDto>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    ensure_project_manage_access(&state, project_id, &current_user).await?;
    ensure_project_exists(&state, project_id).await?;

    let mut seen = HashSet::new();
    for assignment in &request.assignments {
        if !PROJECT_PERMISSION_OPTIONS.contains(&assignment.permission.as_str()) {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Permission must be one of view, create_issue, admin"));
        }
        if assignment.subject_type != "user" && assignment.subject_type != "email" {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Subject type must be user or email"));
        }
        let key = format!("{}:{}", assignment.subject_type, assignment.subject_id);
        if !seen.insert(key) {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Each subject can only be assigned once per project"));
        }
    }

    let mut tx = state.pool.begin().await.map_err(internal_error)?;
    sqlx::query(
        r#"
        DELETE FROM project_permissions
        WHERE project_id = $1
        "#,
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    for assignment in &request.assignments {
        sqlx::query(
            r#"
            INSERT INTO project_permissions (project_id, subject_type, subject_id, permission, effect)
            VALUES ($1, $2, $3, $4, 'allow')
            "#,
        )
        .bind(project_id)
        .bind(&assignment.subject_type)
        .bind(&assignment.subject_id)
        .bind(&assignment.permission)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    }

    tx.commit().await.map_err(internal_error)?;
    get_project_access(Path(project_id), State(state), headers).await
}

async fn ensure_project_exists(state: &AppState, project_id: Uuid) -> ApiResult<()> {
    let exists = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM projects
        WHERE id = $1
        "#,
    )
    .bind(project_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if exists.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Project not found"));
    }

    Ok(())
}
