use axum::{
    extract::{Path, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    Json,
};
use sqlx::Row;

use crate::{
    dto::{AcceptInvitationRequest, AuthResponse, InvitationPreviewDto, UserRow},
    error::{internal_error, ApiError, ApiResult},
    services::auth as auth_service,
    state::AppState,
};

pub async fn get_invitation_preview(
    Path(invite_token): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<InvitationPreviewDto>> {
    let row = sqlx::query(
        r#"
        SELECT email, is_admin, status, expires_at
        FROM user_invitations
        WHERE invite_token = $1
        LIMIT 1
        "#,
    )
    .bind(invite_token)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(axum::http::StatusCode::NOT_FOUND, "Invitation not found"))?;

    let status: String = row.try_get("status").map_err(internal_error)?;
    let expires_at = row.try_get("expires_at").map_err(internal_error)?;

    Ok(Json(InvitationPreviewDto {
        email: row.try_get("email").map_err(internal_error)?,
        is_admin: row.try_get("is_admin").map_err(internal_error)?,
        status,
        expires_at,
    }))
}

pub async fn accept_invitation(
    Path(invite_token): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AcceptInvitationRequest>,
) -> ApiResult<impl IntoResponse> {
    let mut tx = state.pool.begin().await.map_err(internal_error)?;

    let invitation = sqlx::query(
        r#"
        SELECT id, email, is_admin, status, expires_at
        FROM user_invitations
        WHERE invite_token = $1
        LIMIT 1
        "#,
    )
    .bind(&invite_token)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| ApiError::new(axum::http::StatusCode::NOT_FOUND, "Invitation not found"))?;

    let invitation_id: uuid::Uuid = invitation.try_get("id").map_err(internal_error)?;
    let email: String = invitation.try_get("email").map_err(internal_error)?;
    let is_admin: bool = invitation.try_get("is_admin").map_err(internal_error)?;
    let status: String = invitation.try_get("status").map_err(internal_error)?;
    let expires_at: chrono::DateTime<chrono::Utc> =
        invitation.try_get("expires_at").map_err(internal_error)?;

    if status != "pending" {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "Invitation is no longer pending",
        ));
    }

    if expires_at <= chrono::Utc::now() {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "Invitation has expired",
        ));
    }

    let existing_user = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT 1
        FROM users
        WHERE email = $1
        LIMIT 1
        "#,
    )
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal_error)?;

    if existing_user.is_some() {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "User with this email already exists",
        ));
    }

    let user = sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (email, full_name, password_hash, preferred_language, is_admin, active)
        VALUES ($1, $2, crypt($3, gen_salt('bf')), NULL, $4, TRUE)
        RETURNING id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        "#,
    )
    .bind(&email)
    .bind(request.full_name)
    .bind(request.password)
    .bind(is_admin)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        INSERT INTO project_permissions (project_id, subject_type, subject_id, permission, effect)
        SELECT
            project_id,
            'user',
            $2,
            permission,
            effect
        FROM project_permissions
        WHERE subject_type = 'email'
          AND subject_id = $1
        ON CONFLICT (project_id, subject_type, subject_id)
        DO UPDATE SET
            permission = CASE
                WHEN project_permissions.permission = 'admin' OR EXCLUDED.permission = 'admin' THEN 'admin'
                WHEN project_permissions.permission = 'create_issue' OR EXCLUDED.permission = 'create_issue' THEN 'create_issue'
                ELSE 'view'
            END,
            effect = 'allow'
        "#,
    )
    .bind(&email)
    .bind(user.id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        DELETE FROM project_permissions
        WHERE subject_type = 'email'
          AND subject_id = $1
        "#,
    )
    .bind(&email)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
        SELECT
            issue_id,
            'user',
            $2,
            permission,
            effect
        FROM issue_permissions
        WHERE subject_type = 'email'
          AND subject_id = $1
          AND NOT EXISTS (
            SELECT 1
            FROM issue_permissions existing
            WHERE existing.issue_id = issue_permissions.issue_id
              AND existing.subject_type = 'user'
              AND existing.subject_id = $2
              AND existing.permission = issue_permissions.permission
              AND existing.effect = issue_permissions.effect
          )
        "#,
    )
    .bind(&email)
    .bind(user.id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        DELETE FROM issue_permissions
        WHERE subject_type = 'email'
          AND subject_id = $1
        "#,
    )
    .bind(&email)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        r#"
        UPDATE user_invitations
        SET status = 'accepted',
            accepted_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(invitation_id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    tx.commit().await.map_err(internal_error)?;

    let session = auth_service::create_session_for_user(
        state.pool.as_ref(),
        user,
        state.config.session_cookie_secure,
    )
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, session.set_cookie);

    Ok((headers, Json(AuthResponse { user: session.user })))
}
