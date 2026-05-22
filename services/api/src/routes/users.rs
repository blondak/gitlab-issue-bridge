use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::{
    dto::{
        CreateInvitationRequest, ManagedUserDto, UpdateUserAccessRequest, UpdateUserRequest,
        UserAccessIssueOptionDto, UserAccessIssuePermissionRow, UserAccessOverviewDto,
        UserAccessProjectOptionDto, UserAccessProjectPermissionRow, UserInvitationDto, UserInvitationRow,
        UserManagementOverviewDto, UserRow,
    },
    error::{internal_error, ApiError, ApiResult},
    services::auth as auth_service,
    state::AppState,
};

pub async fn get_user_management_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<UserManagementOverviewDto>> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let users = sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        FROM users
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .into_iter()
    .map(ManagedUserDto::from)
    .collect();

    let invitations = sqlx::query_as::<_, UserInvitationRow>(
        r#"
        SELECT id, email, invited_by_user_id, is_admin, status, expires_at, last_sent_at, accepted_at, created_at
        FROM user_invitations
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .into_iter()
    .map(UserInvitationDto::from)
    .collect();

    Ok(Json(UserManagementOverviewDto { users, invitations }))
}

pub async fn update_user(
    Path(user_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateUserRequest>,
) -> ApiResult<Json<ManagedUserDto>> {
    let current_admin = auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    if current_admin.id == user_id && (!request.active || !request.is_admin) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "You cannot remove your own admin access or deactivate yourself",
        ));
    }

    let user = sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET full_name = $2,
            is_admin = $3,
            active = $4,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        "#,
    )
    .bind(user_id)
    .bind(request.full_name)
    .bind(request.is_admin)
    .bind(request.active)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "User not found"))?;

    Ok(Json(ManagedUserDto::from(user)))
}

pub async fn create_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateInvitationRequest>,
) -> ApiResult<(StatusCode, Json<UserInvitationDto>)> {
    let current_admin = auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;
    let email = request.email.trim().to_lowercase();

    let existing_user = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT 1
        FROM users
        WHERE email = $1
        LIMIT 1
        "#,
    )
    .bind(&email)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if existing_user.is_some() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "User with this email already exists",
        ));
    }

    let invite_token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(7);

    let invitation = if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM user_invitations
        WHERE email = $1
          AND status = 'pending'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&email)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    {
        sqlx::query_as::<_, UserInvitationRow>(
            r#"
            UPDATE user_invitations
            SET invite_token = $2,
                is_admin = $3,
                expires_at = $4,
                last_sent_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, invited_by_user_id, is_admin, status, expires_at, last_sent_at, accepted_at, created_at
            "#,
        )
        .bind(existing_id)
        .bind(&invite_token)
        .bind(request.is_admin)
        .bind(expires_at)
        .fetch_one(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    } else {
        sqlx::query_as::<_, UserInvitationRow>(
            r#"
            INSERT INTO user_invitations (email, invite_token, invited_by_user_id, is_admin, status, expires_at)
            VALUES ($1, $2, $3, $4, 'pending', $5)
            RETURNING id, email, invited_by_user_id, is_admin, status, expires_at, last_sent_at, accepted_at, created_at
            "#,
        )
        .bind(&email)
        .bind(&invite_token)
        .bind(current_admin.id)
        .bind(request.is_admin)
        .bind(expires_at)
        .fetch_one(state.pool.as_ref())
        .await
        .map_err(internal_error)?
    };

    enqueue_invitation_email(
        &state,
        invitation.id,
        &email,
        &invite_token,
        request.is_admin,
        &current_admin.email,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(UserInvitationDto::from(invitation))))
}

pub async fn resend_invitation(
    Path(invitation_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<UserInvitationDto>> {
    let current_admin = auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;
    let limits = &state.config.rate_limits;
    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.invitation_resend_per_admin,
        format!("invitation-resend:admin:{}", current_admin.id),
    )?;

    let invite_token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(7);

    let invitation = sqlx::query_as::<_, UserInvitationRow>(
        r#"
        UPDATE user_invitations
        SET invite_token = $2,
            expires_at = $3,
            last_sent_at = NOW(),
            updated_at = NOW(),
            status = 'pending'
        WHERE id = $1
          AND accepted_at IS NULL
        RETURNING id, email, invited_by_user_id, is_admin, status, expires_at, last_sent_at, accepted_at, created_at
        "#,
    )
    .bind(invitation_id)
    .bind(&invite_token)
    .bind(expires_at)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Invitation not found"))?;

    enqueue_invitation_email(
        &state,
        invitation.id,
        &invitation.email,
        &invite_token,
        invitation.is_admin,
        &current_admin.email,
    )
    .await?;

    Ok(Json(UserInvitationDto::from(invitation)))
}

pub async fn delete_invitation(
    Path(invitation_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let mut tx = state.pool.begin().await.map_err(internal_error)?;

    let deleted = sqlx::query(
        r#"
        DELETE FROM user_invitations
        WHERE id = $1
          AND accepted_at IS NULL
        "#,
    )
    .bind(invitation_id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    if deleted.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Invitation not found"));
    }

    sqlx::query(
        r#"
        DELETE FROM jobs
        WHERE dedupe_key = $1
        "#,
    )
    .bind(format!("invite-email:{invitation_id}"))
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    tx.commit().await.map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_user_access(
    Path(user_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<UserAccessOverviewDto>> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let user_exists = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT id FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if user_exists.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "User not found"));
    }

    let project_permissions = sqlx::query_as::<_, UserAccessProjectPermissionRow>(
        r#"
        SELECT pp.project_id, p.name AS project_name, pp.permission
        FROM project_permissions pp
        JOIN projects p ON p.id = pp.project_id
        WHERE pp.subject_type = 'user'
          AND pp.subject_id = $1
          AND pp.effect = 'allow'
        ORDER BY p.name ASC
        "#,
    )
    .bind(user_id.to_string())
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let assigned_project_ids: Vec<Uuid> = project_permissions.iter().map(|p| p.project_id).collect();

    let issue_permissions = sqlx::query_as::<_, UserAccessIssuePermissionRow>(
        r#"
        SELECT
            ip.issue_id,
            i.title AS issue_title,
            COALESCE(i.gitlab_issue_iid, 0) AS gitlab_issue_iid,
            p.id AS project_id,
            p.name AS project_name,
            ip.permission
        FROM issue_permissions ip
        JOIN issues i ON i.id = ip.issue_id
        JOIN projects p ON p.id = i.project_id
        WHERE ip.subject_type = 'user'
          AND ip.subject_id = $1
          AND ip.effect = 'allow'
        ORDER BY p.name ASC, i.title ASC
        "#,
    )
    .bind(user_id.to_string())
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let all_projects = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, name FROM projects ORDER BY name ASC
        "#,
    )
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let available_projects = all_projects
        .into_iter()
        .filter(|(id, _)| !assigned_project_ids.contains(id))
        .map(|(project_id, project_name)| UserAccessProjectOptionDto { project_id, project_name })
        .collect();

    let available_issues = sqlx::query_as::<_, UserAccessIssueOptionDto>(
        r#"
        SELECT
            i.id AS issue_id,
            i.title AS issue_title,
            COALESCE(i.gitlab_issue_iid, 0) AS gitlab_issue_iid,
            p.id AS project_id,
            p.name AS project_name
        FROM issues i
        JOIN projects p ON p.id = i.project_id
        WHERE NOT EXISTS (
            SELECT 1
            FROM issue_permissions ip
            WHERE ip.issue_id = i.id
              AND ip.subject_type = 'user'
              AND ip.subject_id = $1
              AND ip.effect = 'allow'
        )
        ORDER BY p.name ASC, i.title ASC
        "#,
    )
    .bind(user_id.to_string())
    .fetch_all(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(Json(UserAccessOverviewDto {
        project_permissions,
        issue_permissions,
        available_projects,
        available_issues,
    }))
}

pub async fn update_user_access(
    Path(user_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateUserAccessRequest>,
) -> ApiResult<Json<UserAccessOverviewDto>> {
    auth_service::require_admin_from_headers(state.pool.as_ref(), &headers).await?;

    let user_exists = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT id FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    if user_exists.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "User not found"));
    }

    const PROJECT_PERMISSION_OPTIONS: &[&str] = &["view", "create_issue", "admin"];
    const ISSUE_PERMISSION_OPTIONS: &[&str] = &["read", "comment", "edit", "admin"];

    let mut seen_project_ids = HashSet::new();
    for entry in &request.project_permissions {
        if !PROJECT_PERMISSION_OPTIONS.contains(&entry.permission.as_str()) {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Permission must be one of view, create_issue, admin"));
        }

        if !seen_project_ids.insert(entry.project_id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Each project can only be assigned once per user",
            ));
        }
    }

    let mut seen_issue_ids = HashSet::new();
    if let Some(issue_permissions) = &request.issue_permissions {
        for entry in issue_permissions {
            if !ISSUE_PERMISSION_OPTIONS.contains(&entry.permission.as_str()) {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "Permission must be one of read, comment, edit, admin",
                ));
            }

            if !seen_issue_ids.insert(entry.issue_id) {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "Each issue can only be assigned once per user",
                ));
            }
        }
    }

    let requested_project_ids = request
        .project_permissions
        .iter()
        .map(|entry| entry.project_id)
        .collect::<Vec<_>>();

    if !requested_project_ids.is_empty() {
        let matched_project_ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM projects
            WHERE id = ANY($1)
            "#,
        )
        .bind(&requested_project_ids)
        .fetch_all(state.pool.as_ref())
        .await
        .map_err(internal_error)?;

        if matched_project_ids.len() != requested_project_ids.len() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "One or more selected projects do not exist",
            ));
        }
    }

    if let Some(issue_permissions) = &request.issue_permissions {
        let requested_issue_ids = issue_permissions
            .iter()
            .map(|entry| entry.issue_id)
            .collect::<Vec<_>>();

        if !requested_issue_ids.is_empty() {
            let matched_issue_ids = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT id
                FROM issues
                WHERE id = ANY($1)
                "#,
            )
            .bind(&requested_issue_ids)
            .fetch_all(state.pool.as_ref())
            .await
            .map_err(internal_error)?;

            if matched_issue_ids.len() != requested_issue_ids.len() {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "One or more selected issues do not exist",
                ));
            }
        }
    }

    let mut tx = state.pool.begin().await.map_err(internal_error)?;

    sqlx::query(
        r#"
        DELETE FROM project_permissions
        WHERE subject_type = 'user'
          AND subject_id = $1
        "#,
    )
    .bind(user_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    for entry in &request.project_permissions {
        sqlx::query(
            r#"
            INSERT INTO project_permissions (project_id, subject_type, subject_id, permission, effect)
            VALUES ($1, 'user', $2, $3, 'allow')
            "#,
        )
        .bind(entry.project_id)
        .bind(user_id.to_string())
        .bind(&entry.permission)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    }

    if let Some(issue_permissions) = &request.issue_permissions {
        sqlx::query(
            r#"
            DELETE FROM issue_permissions
            WHERE subject_type = 'user'
              AND subject_id = $1
            "#,
        )
        .bind(user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;

        for entry in issue_permissions {
            sqlx::query(
                r#"
                INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
                VALUES ($1, 'user', $2, $3, 'allow')
                "#,
            )
            .bind(entry.issue_id)
            .bind(user_id.to_string())
            .bind(&entry.permission)
            .execute(&mut *tx)
            .await
            .map_err(internal_error)?;
        }
    }

    tx.commit().await.map_err(internal_error)?;

    get_user_access(Path(user_id), State(state), headers).await
}

async fn enqueue_invitation_email(
    state: &AppState,
    invitation_id: Uuid,
    email: &str,
    invite_token: &str,
    is_admin: bool,
    invited_by: &str,
) -> ApiResult<()> {
    let invite_url = format!(
        "{}/login?invite={invite_token}",
        state.config.public_frontend_url.trim_end_matches('/')
    );
    sqlx::query(
        r#"
        INSERT INTO jobs (topic, payload, dedupe_key)
        VALUES ($1, $2, $3)
        ON CONFLICT (dedupe_key)
        DO UPDATE SET
            payload = EXCLUDED.payload,
            status = 'pending',
            available_at = NOW(),
            last_error = NULL,
            updated_at = NOW()
        "#,
    )
    .bind("user.invitation.send_email")
    .bind(json!({
        "invitation_id": invitation_id,
        "email": email,
        "invite_url": invite_url,
        "is_admin": is_admin,
        "invited_by": invited_by
    }))
    .bind(format!("invite-email:{}", invitation_id))
    .execute(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    Ok(())
}
