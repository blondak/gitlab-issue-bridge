use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dto::{UserDto, UserRow},
    error::{internal_error, ApiError, ApiResult},
    repositories::auth as auth_repository,
};

pub const SESSION_COOKIE_NAME: &str = "issuehub_session";
const SESSION_MAX_AGE_SECONDS: i64 = 60 * 60 * 24 * 30;

pub struct AuthenticatedSession {
    pub user: UserDto,
    pub set_cookie: HeaderValue,
}

pub async fn login(
    pool: &PgPool,
    email: &str,
    password: &str,
    secure_cookie: bool,
) -> ApiResult<AuthenticatedSession> {
    let normalized_email = email.trim().to_lowercase();

    let user = auth_repository::find_active_user_by_credentials(pool, &normalized_email, password)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    create_session_for_user(pool, user, secure_cookie).await
}

pub async fn create_session_for_user(
    pool: &PgPool,
    user: UserRow,
    secure_cookie: bool,
) -> ApiResult<AuthenticatedSession> {
    let session_token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(SESSION_MAX_AGE_SECONDS);

    auth_repository::create_session(pool, user.id, &session_token, expires_at)
        .await
        .map_err(internal_error)?;

    Ok(AuthenticatedSession {
        user: UserDto::from(user),
        set_cookie: build_session_cookie(&session_token, secure_cookie)?,
    })
}

pub async fn current_user_from_headers(pool: &PgPool, headers: &HeaderMap) -> ApiResult<UserDto> {
    let session_token = get_session_cookie(headers)
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Missing session"))?;

    let user = auth_repository::find_active_user_by_session_token(pool, &session_token)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Invalid session"))?;

    Ok(UserDto::from(user))
}

pub async fn require_admin_from_headers(pool: &PgPool, headers: &HeaderMap) -> ApiResult<UserDto> {
    let user = current_user_from_headers(pool, headers).await?;
    if !user.is_admin {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Admin access required"));
    }
    Ok(user)
}

pub async fn logout(
    pool: &PgPool,
    headers: &HeaderMap,
    secure_cookie: bool,
) -> ApiResult<Option<HeaderValue>> {
    if let Some(token) = get_session_cookie(headers) {
        auth_repository::delete_session(pool, &token)
            .await
            .map_err(internal_error)?;
    }

    Ok(Some(clear_session_cookie(secure_cookie)))
}

fn build_session_cookie(session_token: &str, secure_cookie: bool) -> ApiResult<HeaderValue> {
    let secure_attribute = if secure_cookie { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE_NAME}={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_MAX_AGE_SECONDS}{secure_attribute}"
    ))
    .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

fn clear_session_cookie(secure_cookie: bool) -> HeaderValue {
    if secure_cookie {
        HeaderValue::from_static(
            "issuehub_session=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Secure",
        )
    } else {
        HeaderValue::from_static(
            "issuehub_session=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        )
    }
}

fn get_session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|cookie| {
        let mut parts = cookie.trim().splitn(2, '=');
        let name = parts.next()?;
        let value = parts.next()?;
        if name == SESSION_COOKIE_NAME {
            Some(value.to_string())
        } else {
            None
        }
    })
}
