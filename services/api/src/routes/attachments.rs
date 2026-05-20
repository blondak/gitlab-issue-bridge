use std::{
    io::ErrorKind,
    path::{Path as FsPath, PathBuf},
};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use bridge_core::secrets::decrypt_secret;
use reqwest::header::HeaderMap as ReqwestHeaderMap;
use tokio::fs;
use tracing::warn;
use uuid::Uuid;

use crate::{
    error::{internal_error, ApiError, ApiResult},
    services::auth as auth_service,
    state::AppState,
};

pub async fn download_attachment(
    Path(attachment_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let current_user = auth_service::current_user_from_headers(state.pool.as_ref(), &headers).await?;

    let attachment = sqlx::query_as::<_, AttachmentDownloadRow>(
        r#"
        SELECT
            issue_attachments.id,
            issue_attachments.filename,
            issue_attachments.content_type,
            issue_attachments.external_url,
            issue_attachments.storage_backend,
            issue_attachments.storage_path,
            issue_attachments.issue_id,
            project_gitlab_integrations.gitlab_api_base_url,
            project_gitlab_integrations.gitlab_project_id,
            project_gitlab_integrations.gitlab_base_url,
            project_gitlab_integrations.token_encrypted,
            project_gitlab_integrations.verify_tls
        FROM issue_attachments
        JOIN issues ON issues.id = issue_attachments.issue_id
        LEFT JOIN project_gitlab_integrations ON project_gitlab_integrations.project_id = issues.project_id
        WHERE (issue_attachments.proxy_path = $1
           OR issue_attachments.id::text = $2)
        LIMIT 1
        "#,
    )
    .bind(format!("/api/v1/attachments/{attachment_id}/download"))
    .bind(&attachment_id)
    .fetch_optional(state.pool.as_ref())
    .await
    .map_err(internal_error)?;

    let attachment = attachment.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Attachment not found"))?;

    if !current_user.is_admin {
        let allowed = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1::BIGINT
            FROM issues
            LEFT JOIN project_permissions
              ON project_permissions.project_id = issues.project_id
             AND project_permissions.effect = 'allow'
             AND (
               (project_permissions.subject_type = 'user' AND project_permissions.subject_id = $2)
               OR (project_permissions.subject_type = 'email' AND project_permissions.subject_id = $3)
             )
             AND project_permissions.permission = ANY($4)
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
            LIMIT 1
            "#,
        )
        .bind(attachment.issue_id)
        .bind(current_user.id.to_string())
        .bind(current_user.email.clone())
        .bind(["view", "admin"].as_slice())
        .bind(["read", "comment", "edit", "admin"].as_slice())
        .fetch_optional(state.pool.as_ref())
        .await
        .map_err(internal_error)?;

        if allowed.is_none() {
            return Err(ApiError::new(StatusCode::NOT_FOUND, "Attachment not found"));
        }
    }

    match attachment.storage_backend.as_str() {
        "local" => download_local_attachment(&attachment).await,
        "gitlab" => download_gitlab_attachment(&state, attachment).await,
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Unsupported attachment storage backend",
        )),
    }
}

async fn download_local_attachment(attachment: &AttachmentDownloadRow) -> ApiResult<Response> {
    let storage_path = attachment
        .storage_path
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Attachment file is missing"))?;
    let bytes = fs::read(storage_path)
        .await
        .map_err(|error| ApiError::new(StatusCode::NOT_FOUND, error.to_string()))?;

    build_attachment_response(&attachment.filename, &attachment.content_type, bytes)
}

async fn download_gitlab_attachment(
    state: &AppState,
    attachment: AttachmentDownloadRow,
) -> ApiResult<Response> {
    if let Some(bytes) = read_cached_gitlab_attachment(state, &attachment).await {
        return build_attachment_response(&attachment.filename, &attachment.content_type, bytes);
    }

    let encrypted_token = attachment
        .token_encrypted
        .as_ref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab token is not configured"))?;

    let token = decrypt_secret(&state.config.secret_encryption_key, encrypted_token)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let mut reqwest_headers = ReqwestHeaderMap::new();
    reqwest_headers.insert(
        "PRIVATE-TOKEN",
        HeaderValue::from_str(&token)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?,
    );

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!attachment.verify_tls.unwrap_or(true))
        .default_headers(reqwest_headers)
        .build()
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let upstream_url = build_gitlab_upload_api_url(&attachment)?;

    let upstream = match client.get(upstream_url).send().await {
        Ok(response) => response,
        Err(error) => {
            record_gitlab_cache_error(state, attachment.id, "remote_fetch_failed", &error.to_string()).await;
            warn!(attachment_id = %attachment.id, error = %error, "failed to fetch GitLab attachment");
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "Failed to download GitLab attachment",
            ));
        }
    };

    let upstream = match upstream.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            record_gitlab_cache_error(state, attachment.id, "remote_fetch_failed", &error.to_string()).await;
            warn!(attachment_id = %attachment.id, error = %error, "GitLab attachment request failed");
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "Failed to download GitLab attachment",
            ));
        }
    };

    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(&attachment.content_type)
        .to_string();

    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => {
            record_gitlab_cache_error(state, attachment.id, "remote_fetch_failed", &error.to_string()).await;
            warn!(attachment_id = %attachment.id, error = %error, "failed to read GitLab attachment body");
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "Failed to download GitLab attachment",
            ));
        }
    };

    persist_gitlab_attachment_cache(state, &attachment, &content_type, &bytes).await;

    build_attachment_response(&attachment.filename, &content_type, bytes)
}

fn build_attachment_response(filename: &str, content_type: &str, bytes: Vec<u8>) -> ApiResult<Response> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", header_safe_filename(filename)),
        )
        .body(Body::from(bytes))
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn read_cached_gitlab_attachment(
    state: &AppState,
    attachment: &AttachmentDownloadRow,
) -> Option<Vec<u8>> {
    let storage_path = attachment.storage_path.as_deref()?;

    match fs::read(storage_path).await {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            record_gitlab_cache_missing(state, attachment.id).await;
            None
        }
        Err(error) => {
            record_gitlab_cache_error(state, attachment.id, "cache_read_failed", &error.to_string()).await;
            warn!(
                attachment_id = %attachment.id,
                storage_path = storage_path,
                error = %error,
                "failed to read GitLab attachment cache"
            );
            None
        }
    }
}

async fn persist_gitlab_attachment_cache(
    state: &AppState,
    attachment: &AttachmentDownloadRow,
    content_type: &str,
    bytes: &[u8],
) {
    let cache_path = gitlab_attachment_cache_path(&state.config, attachment);

    if let Some(parent) = cache_path.parent() {
        if let Err(error) = fs::create_dir_all(parent).await {
            record_gitlab_cache_error(state, attachment.id, "cache_write_failed", &error.to_string()).await;
            warn!(attachment_id = %attachment.id, error = %error, "failed to create GitLab attachment cache directory");
            return;
        }
    }

    let temp_path = temporary_cache_path(&cache_path);
    if let Err(error) = fs::write(&temp_path, bytes).await {
        record_gitlab_cache_error(state, attachment.id, "cache_write_failed", &error.to_string()).await;
        warn!(attachment_id = %attachment.id, error = %error, "failed to write GitLab attachment cache");
        return;
    }
    if let Err(error) = fs::rename(&temp_path, &cache_path).await {
        let _ = fs::remove_file(&temp_path).await;
        record_gitlab_cache_error(state, attachment.id, "cache_write_failed", &error.to_string()).await;
        warn!(attachment_id = %attachment.id, error = %error, "failed to commit GitLab attachment cache");
        return;
    }

    let cache_path = path_to_string(&cache_path);
    if let Err(error) = sqlx::query(
        r#"
        UPDATE issue_attachments
        SET storage_path = $2,
            byte_size = $3,
            content_type = $4,
            cache_state = 'cached',
            cached_at = NOW(),
            last_cache_error = NULL
        WHERE id = $1
          AND storage_backend = 'gitlab'
        "#,
    )
    .bind(attachment.id)
    .bind(cache_path)
    .bind(bytes.len() as i64)
    .bind(content_type)
    .execute(state.pool.as_ref())
    .await
    {
        warn!(attachment_id = %attachment.id, error = %error, "failed to persist GitLab attachment cache metadata");
    }
}

async fn record_gitlab_cache_missing(state: &AppState, attachment_id: Uuid) {
    if let Err(error) = sqlx::query(
        r#"
        UPDATE issue_attachments
        SET storage_path = NULL,
            cached_at = NULL,
            cache_state = 'not_cached',
            last_cache_error = NULL
        WHERE id = $1
          AND storage_backend = 'gitlab'
        "#,
    )
    .bind(attachment_id)
    .execute(state.pool.as_ref())
    .await
    {
        warn!(%attachment_id, error = %error, "failed to mark GitLab attachment cache as missing");
    }
}

async fn record_gitlab_cache_error(
    state: &AppState,
    attachment_id: Uuid,
    cache_state: &str,
    error_message: &str,
) {
    if let Err(error) = sqlx::query(
        r#"
        UPDATE issue_attachments
        SET storage_path = NULL,
            cached_at = NULL,
            cache_state = $2,
            last_cache_error = $3
        WHERE id = $1
          AND storage_backend = 'gitlab'
        "#,
    )
    .bind(attachment_id)
    .bind(cache_state)
    .bind(truncate_cache_error(error_message))
    .execute(state.pool.as_ref())
    .await
    {
        warn!(%attachment_id, error = %error, "failed to persist GitLab attachment cache error");
    }
}

#[derive(sqlx::FromRow)]
struct AttachmentDownloadRow {
    id: Uuid,
    filename: String,
    content_type: String,
    external_url: String,
    storage_backend: String,
    storage_path: Option<String>,
    issue_id: uuid::Uuid,
    gitlab_api_base_url: Option<String>,
    gitlab_project_id: Option<i64>,
    gitlab_base_url: Option<String>,
    token_encrypted: Option<String>,
    verify_tls: Option<bool>,
}

fn gitlab_attachment_cache_path(
    config: &bridge_core::config::AppConfig,
    attachment: &AttachmentDownloadRow,
) -> PathBuf {
    FsPath::new(&config.attachment_cache_dir).join(format!(
        "{}_{}",
        attachment.id,
        sanitize_cache_filename(&attachment.filename)
    ))
}

fn path_to_string(path: &FsPath) -> String {
    path.to_string_lossy().to_string()
}

fn temporary_cache_path(cache_path: &FsPath) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "attachment-cache".into());
    cache_path.with_file_name(format!("{file_name}.{}.tmp", Uuid::new_v4()))
}

fn sanitize_cache_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.trim_matches(&['.', '_', '-'][..]).is_empty() {
        "attachment".to_string()
    } else {
        sanitized
    }
}

fn header_safe_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|character| {
            if character.is_ascii() && !character.is_ascii_control() && !matches!(character, '"' | '\\') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.trim().is_empty() {
        "attachment".to_string()
    } else {
        sanitized
    }
}

fn truncate_cache_error(message: &str) -> String {
    message.chars().take(1000).collect()
}

fn build_gitlab_upload_api_url(attachment: &AttachmentDownloadRow) -> ApiResult<reqwest::Url> {
    let gitlab_base_url = attachment
        .gitlab_base_url
        .as_deref()
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab integration not found"))?;
    let gitlab_api_base_url = attachment
        .gitlab_api_base_url
        .as_deref()
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab integration not found"))?;
    let gitlab_project_id = attachment
        .gitlab_project_id
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "GitLab integration not found"))?;

    let upload_path = normalize_upload_path(&attachment.external_url, gitlab_base_url)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Attachment URL is invalid"))?;

    let upload_tail = upload_path
        .strip_prefix("/uploads/")
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Attachment is not a GitLab upload"))?;

    let (secret, filename) = upload_tail
        .split_once('/')
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Attachment upload path is invalid"))?;

    let mut url = reqwest::Url::parse(gitlab_api_base_url.trim_end_matches('/'))
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "GitLab API base URL is invalid"))?;
        segments.extend([
            "projects",
            &gitlab_project_id.to_string(),
            "uploads",
            secret,
            filename,
        ]);
    }

    Ok(url)
}

fn normalize_upload_path(external_url: &str, gitlab_base_url: &str) -> Option<String> {
    if external_url.starts_with("/uploads/") {
        return Some(external_url.to_string());
    }

    let normalized_base = gitlab_base_url.trim_end_matches('/');
    if external_url.starts_with(normalized_base) {
        let suffix = &external_url[normalized_base.len()..];
        if suffix.starts_with("/uploads/") {
            return Some(suffix.to_string());
        }
    }

    if external_url.starts_with("http://") || external_url.starts_with("https://") {
        let url = reqwest::Url::parse(external_url).ok()?;
        let path = url.path();
        if path.starts_with("/uploads/") {
            return Some(path.to_string());
        }
        if let Some(upload_index) = path.find("/uploads/") {
            return Some(path[upload_index..].to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use bridge_core::config::AppConfig;

    use super::*;

    #[test]
    fn normalizes_project_scoped_gitlab_upload_urls() {
        assert_eq!(
            normalize_upload_path(
                "https://gitlab.example.test/group/project/uploads/secret/file.png",
                "https://gitlab.example.test/group/project",
            ),
            Some("/uploads/secret/file.png".to_string())
        );
        assert_eq!(
            normalize_upload_path("/uploads/secret/file.png", "https://gitlab.example.test"),
            Some("/uploads/secret/file.png".to_string())
        );
    }

    #[test]
    fn cache_path_uses_separate_configured_directory_and_safe_filename() {
        let mut config = AppConfig::default();
        config.attachment_cache_dir = "/tmp/issuehub-cache".to_string();
        let attachment_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let attachment = AttachmentDownloadRow {
            id: attachment_id,
            filename: "../../report final.png".to_string(),
            content_type: "image/png".to_string(),
            external_url: "https://gitlab.example.test/uploads/secret/report.png".to_string(),
            storage_backend: "gitlab".to_string(),
            storage_path: None,
            issue_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            gitlab_api_base_url: Some("https://gitlab.example.test/api/v4".to_string()),
            gitlab_project_id: Some(123),
            gitlab_base_url: Some("https://gitlab.example.test".to_string()),
            token_encrypted: None,
            verify_tls: Some(true),
        };

        let cache_path = gitlab_attachment_cache_path(&config, &attachment);
        assert_eq!(cache_path.parent().unwrap(), FsPath::new("/tmp/issuehub-cache"));
        assert_eq!(
            cache_path.file_name().unwrap().to_string_lossy(),
            "11111111-1111-1111-1111-111111111111_.._.._report_final.png"
        );
    }
}
