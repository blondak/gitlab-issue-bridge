use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::{
    dto::{
        AuthResponse, ChangePasswordRequest, LoginRequest, PasswordRecoveryPreviewDto,
        PasswordRecoveryRequest, PasswordResetRequest, UpdateProfileRequest, UserDto,
    },
    error::{ApiError, ApiResult},
    repositories::auth as auth_repository,
    services::{auth as auth_service, rate_limit},
    state::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
    let normalized_email = request.email.trim().to_lowercase();
    let client = rate_limit::client_identifier(&headers);
    let limits = &state.config.rate_limits;
    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.login_per_email,
        format!("login:email:{}", rate_limit::key_part(&normalized_email)),
    )?;
    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.login_per_ip,
        format!("login:ip:{}", rate_limit::key_part(client)),
    )?;

    let session = auth_service::login(
        state.pool.as_ref(),
        &normalized_email,
        &request.password,
        state.config.session_cookie_secure,
    )
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, session.set_cookie);

    Ok((headers, Json(AuthResponse { user: session.user })))
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<AuthResponse>> {
    let user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;
    Ok(Json(AuthResponse { user }))
}

pub async fn update_me(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let preferred_language = match request.preferred_language.as_deref() {
        Some("cs") => Some("cs"),
        Some("en") => Some("en"),
        Some(_) => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Preferred language must be either cs or en",
            ))
        }
        None => None,
    };

    let user = auth_repository::update_current_user_profile(
        state.pool.as_ref(),
        current_user.id,
        request.full_name.trim(),
        preferred_language,
    )
    .await
    .map_err(crate::error::internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "User not found"))?;

    Ok(Json(AuthResponse {
        user: UserDto::from(user),
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    let cleared_cookie = auth_service::logout(
        state.pool.as_ref(),
        &headers,
        state.config.session_cookie_secure,
    )
    .await?;

    let mut response_headers = HeaderMap::new();
    if let Some(cleared_cookie) = cleared_cookie {
        response_headers.insert(header::SET_COOKIE, cleared_cookie);
    }

    Ok((response_headers, StatusCode::NO_CONTENT))
}

pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> ApiResult<StatusCode> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    if request.new_password.trim().len() < 8 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "New password must have at least 8 characters",
        ));
    }

    let valid_current = auth_repository::verify_user_password(
        state.pool.as_ref(),
        current_user.id,
        request.current_password.trim(),
    )
    .await
    .map_err(crate::error::internal_error)?;

    if !valid_current {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Current password is incorrect"));
    }

    auth_repository::update_user_password(
        state.pool.as_ref(),
        current_user.id,
        request.new_password.trim(),
    )
    .await
    .map_err(crate::error::internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn request_password_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PasswordRecoveryRequest>,
) -> ApiResult<StatusCode> {
    let normalized_email = request.email.trim().to_lowercase();
    let client = rate_limit::client_identifier(&headers);
    let limits = &state.config.rate_limits;
    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.password_recovery_per_ip,
        format!("password-recovery:ip:{}", rate_limit::key_part(client)),
    )?;

    if normalized_email.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }

    state.rate_limiter.check(
        limits.enabled,
        limits.window_seconds,
        limits.password_recovery_per_email,
        format!("password-recovery:email:{}", rate_limit::key_part(&normalized_email)),
    )?;

    if let Some(user) = auth_repository::find_active_user_by_email(state.pool.as_ref(), &normalized_email)
        .await
        .map_err(crate::error::internal_error)?
    {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(2);
        let token_row = auth_repository::create_password_reset_token(
            state.pool.as_ref(),
            user.id,
            &token,
            expires_at,
        )
        .await
        .map_err(crate::error::internal_error)?;

        let reset_url = format!(
            "{}/login?recovery={}",
            state.config.public_frontend_url.trim_end_matches('/'),
            token
        );

        sqlx::query(
            r#"
            INSERT INTO jobs (topic, payload, status, available_at)
            VALUES ($1, $2, 'pending', NOW())
            "#,
        )
        .bind("user.password_reset.send_email")
        .bind(json!({
            "email": user.email,
            "full_name": user.full_name,
            "reset_url": reset_url,
            "expires_at": token_row.expires_at,
        }))
        .execute(state.pool.as_ref())
        .await
        .map_err(crate::error::internal_error)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_password_recovery_preview(
    Path(token): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<PasswordRecoveryPreviewDto>> {
    let token_row = auth_repository::find_valid_password_reset_token(state.pool.as_ref(), token.trim())
        .await
        .map_err(crate::error::internal_error)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Recovery token is invalid or expired"))?;

    let user = sqlx::query_as::<_, crate::dto::UserRow>(
        r#"
        SELECT id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(token_row.user_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(crate::error::internal_error)?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "User not found"))?;

    Ok(Json(PasswordRecoveryPreviewDto {
        email: user.email,
        expires_at: token_row.expires_at,
    }))
}

pub async fn reset_password(
    Path(token): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> ApiResult<StatusCode> {
    if request.password.trim().len() < 8 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Password must have at least 8 characters",
        ));
    }

    let token_row = auth_repository::find_valid_password_reset_token(state.pool.as_ref(), token.trim())
        .await
        .map_err(crate::error::internal_error)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Recovery token is invalid or expired"))?;

    auth_repository::update_user_password(
        state.pool.as_ref(),
        token_row.user_id,
        request.password.trim(),
    )
    .await
    .map_err(crate::error::internal_error)?;

    auth_repository::mark_password_reset_token_used(state.pool.as_ref(), token_row.id)
        .await
        .map_err(crate::error::internal_error)?;

    auth_repository::delete_sessions_for_user(state.pool.as_ref(), token_row.user_id)
        .await
        .map_err(crate::error::internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn require_authenticated(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<UserDto> {
    auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await
}
