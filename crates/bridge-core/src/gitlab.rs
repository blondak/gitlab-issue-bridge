use anyhow::Context;
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio::fs;

#[derive(Debug, Clone)]
pub struct GitLabValidationInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: String,
    pub verify_tls: bool,
}

#[derive(Debug)]
pub struct GitLabValidationResult {
    pub project_name: String,
    pub web_url: String,
    pub visibility: String,
}

#[derive(Debug, Clone)]
pub struct GitLabIssueImportInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: String,
    pub verify_tls: bool,
}

#[derive(Debug, Clone)]
pub struct GitLabIssueSummary {
    pub iid: i64,
    pub title: String,
    pub description: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GitLabIssueCommentSummary {
    pub note_id: i64,
    pub discussion_id: Option<String>,
    pub individual_note: bool,
    pub reply_to_note_id: Option<i64>,
    pub author_external_id: String,
    pub author_name: String,
    pub body_raw: String,
    pub system_note: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GitLabCreateIssueInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: String,
    pub verify_tls: bool,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct GitLabCreateIssueCommentInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub gitlab_issue_iid: i64,
    pub token: String,
    pub verify_tls: bool,
    pub body: String,
    pub discussion_id: Option<String>,
    pub reply_to_note_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct GitLabUpdateIssueInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub gitlab_issue_iid: i64,
    pub token: String,
    pub verify_tls: bool,
    pub title: String,
    pub description: String,
    pub state_event: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitLabUploadAttachmentInput {
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: String,
    pub verify_tls: bool,
    pub file_path: String,
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct GitLabUploadAttachmentResult {
    pub url: String,
}

pub async fn validate_integration(input: GitLabValidationInput) -> anyhow::Result<GitLabValidationResult> {
    let project = fetch_project(input).await?;

    Ok(GitLabValidationResult {
        project_name: project.name,
        web_url: project.web_url,
        visibility: project.visibility,
    })
}

pub async fn fetch_issue_web_url(
    input: GitLabIssueImportInput,
    gitlab_issue_iid: i64,
) -> anyhow::Result<String> {
    let issue = fetch_issue(input, gitlab_issue_iid).await?;

    issue
        .web_url
        .ok_or_else(|| anyhow::anyhow!("GitLab issue response did not include web_url"))
}

pub async fn fetch_issue(
    input: GitLabIssueImportInput,
    gitlab_issue_iid: i64,
) -> anyhow::Result<GitLabIssueResponse> {
    let client = build_client(&input.token, input.verify_tls)?;

    client
        .get(format!(
            "{}/projects/{}/issues/{}",
            input.gitlab_api_base_url.trim_end_matches('/'),
            input.gitlab_project_id,
            gitlab_issue_iid
        ))
        .send()
        .await
        .context("failed to call GitLab issue API")?
        .error_for_status()
        .context("GitLab returned non-success status for issue lookup")?
        .json::<GitLabIssueResponse>()
        .await
        .context("failed to parse GitLab issue response")
}

async fn fetch_project(input: GitLabValidationInput) -> anyhow::Result<GitLabProjectResponse> {
    let client = build_client(&input.token, input.verify_tls)?;

    client
        .get(format!(
            "{}/projects/{}",
            input.gitlab_api_base_url.trim_end_matches('/'),
            input.gitlab_project_id
        ))
        .send()
        .await
        .context("failed to call GitLab API")?
        .error_for_status()
        .context("GitLab returned non-success status")?
        .json::<GitLabProjectResponse>()
        .await
        .context("failed to parse GitLab project response")
}

pub async fn update_project_issue(input: GitLabUpdateIssueInput) -> anyhow::Result<GitLabIssueSummary> {
    let client = build_client(&input.token, input.verify_tls)?;

    let mut params = vec![("title", input.title), ("description", input.description)];
    if let Some(state_event) = input.state_event {
        params.push(("state_event", state_event));
    }

    let issue = client
        .put(format!(
            "{}/projects/{}/issues/{}",
            input.gitlab_api_base_url.trim_end_matches('/'),
            input.gitlab_project_id,
            input.gitlab_issue_iid
        ))
        .form(&params)
        .send()
        .await
        .context("failed to call GitLab update issue API")?
        .error_for_status()
        .context("GitLab returned non-success status for issue update")?
        .json::<GitLabIssueResponse>()
        .await
        .context("failed to parse GitLab update issue response")?;

    Ok(issue.into_summary())
}

pub async fn import_project_issues(input: GitLabIssueImportInput) -> anyhow::Result<Vec<GitLabIssueSummary>> {
    let client = build_client(&input.token, input.verify_tls)?;
    let mut page = 1;
    let mut issues = Vec::new();

    loop {
        let response = client
            .get(format!(
                "{}/projects/{}/issues?per_page=100&page={page}&state=all",
                input.gitlab_api_base_url.trim_end_matches('/'),
                input.gitlab_project_id
            ))
            .send()
            .await
            .context("failed to call GitLab issues API")?
            .error_for_status()
            .context("GitLab returned non-success status for issues import")?;

        let next_page = response
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let page_items = response
            .json::<Vec<GitLabIssueResponse>>()
            .await
            .context("failed to parse GitLab issues response")?;

        issues.extend(page_items.into_iter().map(|issue| issue.into_summary()));

        if next_page.is_empty() {
            break;
        }

        page = next_page
            .parse::<i32>()
            .context("GitLab returned invalid x-next-page header")?;
    }

    Ok(issues)
}

pub async fn create_project_issue(input: GitLabCreateIssueInput) -> anyhow::Result<GitLabIssueSummary> {
    let client = build_client(&input.token, input.verify_tls)?;

    let issue = client
        .post(format!(
            "{}/projects/{}/issues",
            input.gitlab_api_base_url.trim_end_matches('/'),
            input.gitlab_project_id
        ))
        .form(&[("title", input.title), ("description", input.description)])
        .send()
        .await
        .context("failed to call GitLab create issue API")?
        .error_for_status()
        .context("GitLab returned non-success status for issue creation")?
        .json::<GitLabIssueResponse>()
        .await
        .context("failed to parse GitLab create issue response")?;

    Ok(issue.into_summary())
}

pub async fn import_issue_comments(
    input: GitLabIssueImportInput,
    gitlab_issue_iid: i64,
) -> anyhow::Result<Vec<GitLabIssueCommentSummary>> {
    let client = build_client(&input.token, input.verify_tls)?;
    let mut page = 1;
    let mut comments = Vec::new();

    loop {
        let response = client
            .get(format!(
                "{}/projects/{}/issues/{}/discussions?per_page=100&page={page}",
                input.gitlab_api_base_url.trim_end_matches('/'),
                input.gitlab_project_id,
                gitlab_issue_iid
            ))
            .send()
            .await
            .context("failed to call GitLab issue discussions API")?
            .error_for_status()
            .context("GitLab returned non-success status for issue comment import")?;

        let next_page = response
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let page_items = response
            .json::<Vec<GitLabDiscussionResponse>>()
            .await
            .context("failed to parse GitLab issue discussions response")?;

        for discussion in page_items {
            let notes = discussion.notes;
            let parent_note_id = notes.first().map(|note| note.id);
            let notes_len = notes.len();

            comments.extend(notes.into_iter().enumerate().map(|(index, note)| GitLabIssueCommentSummary {
                note_id: note.id,
                discussion_id: Some(discussion.id.clone()),
                individual_note: discussion.individual_note,
                reply_to_note_id: if notes_len > 1 && index > 0 { parent_note_id } else { None },
                author_external_id: note
                    .author
                    .as_ref()
                    .map(|author| format!("gitlab:user:{}", author.id))
                    .unwrap_or_else(|| "gitlab:user:unknown".to_string()),
                author_name: note
                    .author
                    .map(|author| author.name)
                    .unwrap_or_else(|| "GitLab user".to_string()),
                body_raw: note.body,
                system_note: note.system,
                created_at: note.created_at,
                updated_at: note.updated_at,
            }));
        }

        if next_page.is_empty() {
            break;
        }

        page = next_page
            .parse::<i32>()
            .context("GitLab returned invalid x-next-page header for notes")?;
    }

    Ok(comments)
}

pub async fn create_issue_comment(
    input: GitLabCreateIssueCommentInput,
) -> anyhow::Result<GitLabIssueCommentSummary> {
    let client = build_client(&input.token, input.verify_tls)?;

    if let Some(discussion_id) = input.discussion_id {
        let note = client
            .post(format!(
                "{}/projects/{}/issues/{}/discussions/{}/notes",
                input.gitlab_api_base_url.trim_end_matches('/'),
                input.gitlab_project_id,
                input.gitlab_issue_iid,
                discussion_id
            ))
            .form(&[("body", input.body)])
            .send()
            .await
            .context("failed to call GitLab discussion reply API")?
            .error_for_status()
            .context("GitLab returned non-success status for discussion reply")?
            .json::<GitLabDiscussionNoteResponse>()
            .await
            .context("failed to parse GitLab discussion reply response")?;

        Ok(GitLabIssueCommentSummary {
            note_id: note.id,
            discussion_id: Some(discussion_id),
            individual_note: false,
            reply_to_note_id: input.reply_to_note_id,
            author_external_id: note
                .author
                .as_ref()
                .map(|author| format!("gitlab:user:{}", author.id))
                .unwrap_or_else(|| "gitlab:user:unknown".to_string()),
            author_name: note
                .author
                .map(|author| author.name)
                .unwrap_or_else(|| "GitLab user".to_string()),
            body_raw: note.body,
            system_note: note.system,
            created_at: note.created_at,
            updated_at: note.updated_at,
        })
    } else {
        let discussion = client
            .post(format!(
                "{}/projects/{}/issues/{}/discussions",
                input.gitlab_api_base_url.trim_end_matches('/'),
                input.gitlab_project_id,
                input.gitlab_issue_iid
            ))
            .form(&[("body", input.body)])
            .send()
            .await
            .context("failed to call GitLab discussion creation API")?
            .error_for_status()
            .context("GitLab returned non-success status for discussion creation")?
            .json::<GitLabDiscussionResponse>()
            .await
            .context("failed to parse GitLab discussion creation response")?;

        let note = discussion
            .notes
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GitLab discussion response did not include note"))?;

        Ok(GitLabIssueCommentSummary {
            note_id: note.id,
            discussion_id: Some(discussion.id),
            individual_note: discussion.individual_note,
            reply_to_note_id: None,
            author_external_id: note
                .author
                .as_ref()
                .map(|author| format!("gitlab:user:{}", author.id))
                .unwrap_or_else(|| "gitlab:user:unknown".to_string()),
            author_name: note
                .author
                .map(|author| author.name)
                .unwrap_or_else(|| "GitLab user".to_string()),
            body_raw: note.body,
            system_note: note.system,
            created_at: note.created_at,
            updated_at: note.updated_at,
        })
    }
}

pub async fn upload_project_attachment(
    input: GitLabUploadAttachmentInput,
) -> anyhow::Result<GitLabUploadAttachmentResult> {
    let client = build_client(&input.token, input.verify_tls)?;
    let file_bytes = fs::read(&input.file_path)
        .await
        .with_context(|| format!("failed to read attachment {}", input.file_path))?;

    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(input.filename.clone())
        .mime_str(&input.content_type)
        .context("invalid attachment content type")?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let upload = client
        .post(format!(
            "{}/projects/{}/uploads",
            input.gitlab_api_base_url.trim_end_matches('/'),
            input.gitlab_project_id
        ))
        .multipart(form)
        .send()
        .await
        .context("failed to call GitLab attachment upload API")?
        .error_for_status()
        .context("GitLab returned non-success status for attachment upload")?
        .json::<GitLabUploadResponse>()
        .await
        .context("failed to parse GitLab attachment upload response")?;

    Ok(GitLabUploadAttachmentResult {
        url: upload.url,
    })
}

pub fn build_client(token: &str, verify_tls: bool) -> anyhow::Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "PRIVATE-TOKEN",
        HeaderValue::from_str(token).context("invalid GitLab private token")?,
    );

    reqwest::Client::builder()
        .danger_accept_invalid_certs(!verify_tls)
        .default_headers(headers)
        .build()
        .context("failed to build GitLab HTTP client")
}

#[derive(Debug, Deserialize, Clone)]
pub struct GitLabIssueResponse {
    pub iid: i64,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub web_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GitLabIssueResponse {
    pub fn into_summary(self) -> GitLabIssueSummary {
        GitLabIssueSummary {
            iid: self.iid,
            title: self.title,
            description: self.description.unwrap_or_default(),
            state: self.state,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitLabProjectResponse {
    name: String,
    web_url: String,
    visibility: String,
}

#[derive(Debug, Deserialize)]
struct GitLabDiscussionResponse {
    id: String,
    individual_note: bool,
    notes: Vec<GitLabDiscussionNoteResponse>,
}

#[derive(Debug, Deserialize)]
struct GitLabDiscussionNoteResponse {
    id: i64,
    body: String,
    system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    author: Option<GitLabAuthorResponse>,
}

#[derive(Debug, Deserialize)]
struct GitLabAuthorResponse {
    id: i64,
    name: String,
}

#[derive(Debug, Deserialize)]
struct GitLabUploadResponse {
    url: String,
}
