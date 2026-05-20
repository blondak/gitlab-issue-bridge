use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{PasswordResetTokenRow, UserRow};

pub async fn find_active_user_by_credentials(
    pool: &PgPool,
    email: &str,
    password: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        FROM users
        WHERE email = $1
          AND active = TRUE
          AND password_hash = crypt($2, password_hash)
        LIMIT 1
        "#,
    )
    .bind(email)
    .bind(password)
    .fetch_optional(pool)
    .await
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    session_token: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO user_sessions (user_id, session_token, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(session_token)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_session(pool: &PgPool, session_token: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM user_sessions
        WHERE session_token = $1
        "#,
    )
    .bind(session_token)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn find_active_user_by_session_token(
    pool: &PgPool,
    session_token: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        SELECT users.id, users.email, users.full_name, users.password_hash, users.preferred_language, users.is_admin, users.active, users.created_at
        FROM user_sessions
        JOIN users ON users.id = user_sessions.user_id
        WHERE user_sessions.session_token = $1
          AND user_sessions.expires_at > NOW()
          AND users.active = TRUE
        LIMIT 1
        "#,
    )
    .bind(session_token)
    .fetch_optional(pool)
    .await
}

pub async fn update_current_user_profile(
    pool: &PgPool,
    user_id: Uuid,
    full_name: &str,
    preferred_language: Option<&str>,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET full_name = $2,
            preferred_language = $3,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        "#,
    )
    .bind(user_id)
    .bind(full_name)
    .bind(preferred_language)
    .fetch_optional(pool)
    .await
}

pub async fn verify_user_password(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
) -> Result<bool, sqlx::Error> {
    let matched = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM users
        WHERE id = $1
          AND active = TRUE
          AND password_hash = crypt($2, password_hash)
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(password)
    .fetch_optional(pool)
    .await?;

    Ok(matched.is_some())
}

pub async fn update_user_password(
    pool: &PgPool,
    user_id: Uuid,
    new_password: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = crypt($2, gen_salt('bf')),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(new_password)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn find_active_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        r#"
        SELECT id, email, full_name, password_hash, preferred_language, is_admin, active, created_at
        FROM users
        WHERE email = $1
          AND active = TRUE
        LIMIT 1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn create_password_reset_token(
    pool: &PgPool,
    user_id: Uuid,
    token: &str,
    expires_at: DateTime<Utc>,
) -> Result<PasswordResetTokenRow, sqlx::Error> {
    sqlx::query_as::<_, PasswordResetTokenRow>(
        r#"
        INSERT INTO password_reset_tokens (user_id, token_hash, expires_at)
        VALUES ($1, crypt($2, gen_salt('bf')), $3)
        RETURNING id, user_id, token_hash, expires_at, used_at, created_at
        "#,
    )
    .bind(user_id)
    .bind(token)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

pub async fn find_valid_password_reset_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<PasswordResetTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, PasswordResetTokenRow>(
        r#"
        SELECT id, user_id, token_hash, expires_at, used_at, created_at
        FROM password_reset_tokens
        WHERE used_at IS NULL
          AND expires_at > NOW()
          AND token_hash = crypt($1, token_hash)
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn mark_password_reset_token_used(
    pool: &PgPool,
    token_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(token_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_sessions_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM user_sessions
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
