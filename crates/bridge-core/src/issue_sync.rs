use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::gitlab::{GitLabIssueCommentSummary, GitLabIssueSummary};

pub async fn upsert_gitlab_issue_row(
    pool: &PgPool,
    project_id: Uuid,
    issue: &GitLabIssueSummary,
) -> Result<Uuid> {
    sqlx::query(
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'idle', 'gitlab', 1)
        ON CONFLICT (project_id, gitlab_issue_iid)
        DO UPDATE SET
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            state = EXCLUDED.state,
            sync_state = 'idle',
            last_source = 'gitlab',
            version = CASE
                WHEN issues.title IS DISTINCT FROM EXCLUDED.title
                  OR issues.description IS DISTINCT FROM EXCLUDED.description
                  OR issues.state IS DISTINCT FROM EXCLUDED.state
                THEN issues.version + 1
                ELSE issues.version
            END,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(project_id)
    .bind(issue.iid)
    .bind(&issue.title)
    .bind(&issue.description)
    .bind(&issue.state)
    .bind(issue.created_at)
    .bind(issue.updated_at)
    .execute(pool)
    .await
    .context("failed to upsert issue row")?;

    let issue_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM issues
        WHERE project_id = $1
          AND gitlab_issue_iid = $2
        "#,
    )
    .bind(project_id)
    .bind(issue.iid)
    .fetch_one(pool)
    .await
    .context("failed to resolve upserted issue id")?;

    upsert_issue_external_ref(
        pool,
        issue_id,
        project_id,
        "gitlab",
        &issue.iid.to_string(),
        Some(&format!("#{}", issue.iid)),
        None,
        "idle",
    )
    .await?;

    Ok(issue_id)
}

pub async fn upsert_issue_external_ref(
    pool: &PgPool,
    issue_id: Uuid,
    project_id: Uuid,
    provider: &str,
    external_issue_id: &str,
    external_issue_key: Option<&str>,
    external_url: Option<&str>,
    sync_state: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO issue_external_refs (
            issue_id,
            project_id,
            provider,
            external_issue_id,
            external_issue_key,
            external_url,
            sync_state
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (issue_id, provider)
        DO UPDATE SET
            project_id = EXCLUDED.project_id,
            external_issue_id = EXCLUDED.external_issue_id,
            external_issue_key = EXCLUDED.external_issue_key,
            external_url = EXCLUDED.external_url,
            sync_state = EXCLUDED.sync_state,
            updated_at = NOW()
        "#,
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(provider)
    .bind(external_issue_id)
    .bind(external_issue_key)
    .bind(external_url)
    .bind(sync_state)
    .execute(pool)
    .await
    .context("failed to upsert issue external ref")?;

    Ok(())
}

pub async fn persist_gitlab_issue_attachments(
    pool: &PgPool,
    issue_id: Uuid,
    gitlab_base_url: &str,
    body: &str,
) -> Result<()> {
    let existing = existing_gitlab_issue_attachments(pool, issue_id, None).await?;
    let attachments = parse_comment_attachments(body, gitlab_base_url);

    sqlx::query(
        r#"
        DELETE FROM issue_attachments
        WHERE issue_id = $1
          AND comment_id IS NULL
          AND storage_backend = 'gitlab'
        "#,
    )
    .bind(issue_id)
    .execute(pool)
    .await
    .context("failed to clear existing issue attachments")?;

    for attachment in attachments {
        let existing_attachment = existing.get(&attachment.external_url);
        let attachment_id = existing_attachment
            .map(|value| value.id)
            .unwrap_or_else(Uuid::new_v4);
        let proxy_path = existing_attachment
            .map(|value| value.proxy_path.clone())
            .unwrap_or_else(|| format!("/api/v1/attachments/{attachment_id}/download"));
        let byte_size = existing_attachment.map(|value| value.byte_size).unwrap_or(0);
        let storage_path = existing_attachment.and_then(|value| value.storage_path.clone());
        let cache_state = existing_attachment
            .map(|value| value.cache_state.clone())
            .unwrap_or_else(|| "not_cached".to_string());
        let cached_at = existing_attachment.and_then(|value| value.cached_at.clone());
        let last_cache_error = existing_attachment.and_then(|value| value.last_cache_error.clone());
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
            VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, 'gitlab', $8, $9, $10, $11, $12, 'gitlab:issue', 'idle')
            "#,
        )
        .bind(attachment_id)
        .bind(issue_id)
        .bind(&attachment.filename)
        .bind(&attachment.content_type)
        .bind(byte_size)
        .bind(&attachment.external_url)
        .bind(&proxy_path)
        .bind(storage_path)
        .bind(cache_state)
        .bind(cached_at)
        .bind(last_cache_error)
        .bind(attachment.inline)
        .execute(pool)
        .await
        .context("failed to persist issue attachment")?;
    }

    Ok(())
}

pub async fn persist_gitlab_comment_and_attachments(
    pool: &PgPool,
    issue_id: Uuid,
    gitlab_base_url: &str,
    comment: &GitLabIssueCommentSummary,
) -> Result<Uuid> {
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'idle', $10, $11)
        ON CONFLICT (issue_id, gitlab_note_id)
        DO UPDATE SET
            discussion_id = EXCLUDED.discussion_id,
            individual_note = EXCLUDED.individual_note,
            reply_to_gitlab_note_id = EXCLUDED.reply_to_gitlab_note_id,
            author_external_id = EXCLUDED.author_external_id,
            author_name = EXCLUDED.author_name,
            body_raw = EXCLUDED.body_raw,
            system_note = EXCLUDED.system_note,
            sync_state = 'idle',
            updated_at = EXCLUDED.updated_at
        RETURNING id
        "#,
    )
    .bind(issue_id)
    .bind(comment.note_id)
    .bind(&comment.discussion_id)
    .bind(comment.individual_note)
    .bind(comment.reply_to_note_id)
    .bind(&comment.author_external_id)
    .bind(&comment.author_name)
    .bind(&comment.body_raw)
    .bind(comment.system_note)
    .bind(comment.created_at)
    .bind(comment.updated_at)
    .fetch_one(pool)
    .await
    .context("failed to upsert issue comment")?;

    let existing = existing_gitlab_issue_attachments(pool, issue_id, Some(comment_id)).await?;

    sqlx::query(
        r#"
        DELETE FROM issue_attachments
        WHERE comment_id = $1
          AND storage_backend = 'gitlab'
        "#,
    )
    .bind(comment_id)
    .execute(pool)
    .await
    .context("failed to clear existing comment attachments")?;

    for attachment in parse_comment_attachments(&comment.body_raw, gitlab_base_url) {
        let existing_attachment = existing.get(&attachment.external_url);
        let attachment_id = existing_attachment
            .map(|value| value.id)
            .unwrap_or_else(Uuid::new_v4);
        let proxy_path = existing_attachment
            .map(|value| value.proxy_path.clone())
            .unwrap_or_else(|| format!("/api/v1/attachments/{attachment_id}/download"));
        let byte_size = existing_attachment.map(|value| value.byte_size).unwrap_or(0);
        let storage_path = existing_attachment.and_then(|value| value.storage_path.clone());
        let cache_state = existing_attachment
            .map(|value| value.cache_state.clone())
            .unwrap_or_else(|| "not_cached".to_string());
        let cached_at = existing_attachment.and_then(|value| value.cached_at.clone());
        let last_cache_error = existing_attachment.and_then(|value| value.last_cache_error.clone());
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'gitlab', $9, $10, $11, $12, $13, $14, 'idle')
            "#,
        )
        .bind(attachment_id)
        .bind(issue_id)
        .bind(comment_id)
        .bind(&attachment.filename)
        .bind(&attachment.content_type)
        .bind(byte_size)
        .bind(&attachment.external_url)
        .bind(&proxy_path)
        .bind(storage_path)
        .bind(cache_state)
        .bind(cached_at)
        .bind(last_cache_error)
        .bind(attachment.inline)
        .bind(&comment.author_external_id)
        .execute(pool)
        .await
        .context("failed to persist comment attachment")?;
    }

    Ok(comment_id)
}

#[derive(sqlx::FromRow)]
struct ExistingGitlabAttachment {
    id: Uuid,
    external_url: String,
    proxy_path: String,
    byte_size: i64,
    storage_path: Option<String>,
    cache_state: String,
    cached_at: Option<DateTime<Utc>>,
    last_cache_error: Option<String>,
}

async fn existing_gitlab_issue_attachments(
    pool: &PgPool,
    issue_id: Uuid,
    comment_id: Option<Uuid>,
) -> Result<HashMap<String, ExistingGitlabAttachment>> {
    let rows = if let Some(comment_id) = comment_id {
        sqlx::query_as::<_, ExistingGitlabAttachment>(
            r#"
            SELECT id, external_url, proxy_path, byte_size, storage_path, cache_state, cached_at, last_cache_error
            FROM issue_attachments
            WHERE issue_id = $1
              AND comment_id = $2
              AND storage_backend = 'gitlab'
            "#,
        )
        .bind(issue_id)
        .bind(comment_id)
        .fetch_all(pool)
        .await
        .context("failed to load existing GitLab comment attachments")?
    } else {
        sqlx::query_as::<_, ExistingGitlabAttachment>(
            r#"
            SELECT id, external_url, proxy_path, byte_size, storage_path, cache_state, cached_at, last_cache_error
            FROM issue_attachments
            WHERE issue_id = $1
              AND comment_id IS NULL
              AND storage_backend = 'gitlab'
            "#,
        )
        .bind(issue_id)
        .fetch_all(pool)
        .await
        .context("failed to load existing GitLab issue attachments")?
    };

    Ok(rows
        .into_iter()
        .map(|row| (row.external_url.clone(), row))
        .collect())
}

struct ParsedAttachment {
    external_url: String,
    filename: String,
    content_type: String,
    inline: bool,
}

fn parse_comment_attachments(body: &str, gitlab_base_url: &str) -> Vec<ParsedAttachment> {
    let mut urls = Vec::new();
    let trimmed_base = gitlab_base_url.trim_end_matches('/');

    for target in extract_markdown_targets(body) {
        if target.contains("/uploads/") {
            urls.push(target);
        }
    }

    for token in body.split_whitespace() {
        let candidate = token
            .trim_matches(|char: char| matches!(char, ')' | '(' | ']' | '[' | '>' | '<' | '"' | '\'' | ','));
        if candidate.contains("/uploads/") {
            urls.push(candidate.to_string());
        }
    }

    let mut seen = HashSet::new();
    urls.into_iter()
        .filter_map(|url| {
            let absolute_url = if url.starts_with("http://") || url.starts_with("https://") {
                url
            } else if url.starts_with('/') {
                format!("{trimmed_base}{url}")
            } else {
                return None;
            };

            if !seen.insert(absolute_url.clone()) {
                return None;
            }

            let filename = absolute_url
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .unwrap_or("attachment")
                .to_string();
            let content_type = content_type_from_filename(&filename).to_string();
            let inline = matches!(
                filename.rsplit('.').next().map(|value| value.to_ascii_lowercase()),
                Some(ext) if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")
            );

            Some(ParsedAttachment {
                external_url: absolute_url,
                filename,
                content_type,
                inline,
            })
        })
        .collect()
}

fn extract_markdown_targets(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut index = 0usize;
    let mut targets = Vec::new();

    while index + 1 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(relative_end) = body[start..].find(')') {
                let target = body[start..start + relative_end].trim();
                if !target.is_empty() {
                    targets.push(target.to_string());
                }
                index = start + relative_end + 1;
                continue;
            }
        }
        index += 1;
    }

    targets
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
