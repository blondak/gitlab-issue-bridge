use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio::fs;

use crate::integrations::{
    CreateExternalComment, CreateExternalIssue, ExternalAttachmentUpload, ExternalComment,
    ExternalIssue, ExternalValidationResult, IntegrationProvider, IssueTrackerAdapter,
    ProjectIntegrationConfig, UpdateExternalIssue, UploadExternalAttachment,
};

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

#[derive(Debug, Clone, Copy, Default)]
pub struct GitLabAdapter;

impl IssueTrackerAdapter for GitLabAdapter {
    fn provider(&self) -> IntegrationProvider {
        IntegrationProvider::GitLab
    }

    async fn validate(
        &self,
        config: &ProjectIntegrationConfig,
    ) -> anyhow::Result<ExternalValidationResult> {
        let input = gitlab_validation_input(config)?;
        validate_integration(input).await.map(Into::into)
    }

    async fn import_issues(
        &self,
        config: &ProjectIntegrationConfig,
    ) -> anyhow::Result<Vec<ExternalIssue>> {
        let input = gitlab_issue_import_input(config)?;
        let issues = import_project_issues(input).await?;
        Ok(issues.into_iter().map(Into::into).collect())
    }

    async fn fetch_issue(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
    ) -> anyhow::Result<ExternalIssue> {
        let input = gitlab_issue_import_input(config)?;
        let issue = crate::gitlab::fetch_issue(input, parse_gitlab_issue_iid(external_issue_id)?).await?;
        Ok(issue.into_summary().into())
    }

    async fn create_issue(
        &self,
        config: &ProjectIntegrationConfig,
        input: CreateExternalIssue,
    ) -> anyhow::Result<ExternalIssue> {
        let project_id = gitlab_project_id(config)?;
        let issue = create_project_issue(GitLabCreateIssueInput {
            gitlab_api_base_url: config.api_base_url.clone(),
            gitlab_project_id: project_id,
            token: config.token.clone(),
            verify_tls: config.verify_tls,
            title: input.title,
            description: input.description,
        })
        .await?;

        Ok(issue.into())
    }

    async fn update_issue(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
        input: UpdateExternalIssue,
    ) -> anyhow::Result<ExternalIssue> {
        let project_id = gitlab_project_id(config)?;
        let issue = update_project_issue(GitLabUpdateIssueInput {
            gitlab_api_base_url: config.api_base_url.clone(),
            gitlab_project_id: project_id,
            gitlab_issue_iid: parse_gitlab_issue_iid(external_issue_id)?,
            token: config.token.clone(),
            verify_tls: config.verify_tls,
            title: input.title,
            description: input.description,
            state_event: input.state_event,
        })
        .await?;

        Ok(issue.into())
    }

    async fn import_comments(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
    ) -> anyhow::Result<Vec<ExternalComment>> {
        let input = gitlab_issue_import_input(config)?;
        let comments = import_issue_comments(input, parse_gitlab_issue_iid(external_issue_id)?).await?;
        Ok(comments.into_iter().map(Into::into).collect())
    }

    async fn create_comment(
        &self,
        config: &ProjectIntegrationConfig,
        input: CreateExternalComment,
    ) -> anyhow::Result<ExternalComment> {
        let project_id = gitlab_project_id(config)?;
        let comment = create_issue_comment(GitLabCreateIssueCommentInput {
            gitlab_api_base_url: config.api_base_url.clone(),
            gitlab_project_id: project_id,
            gitlab_issue_iid: parse_gitlab_issue_iid(&input.external_issue_id)?,
            token: config.token.clone(),
            verify_tls: config.verify_tls,
            body: input.body,
            discussion_id: input.discussion_id,
            reply_to_note_id: input
                .reply_to_external_id
                .as_deref()
                .map(parse_gitlab_note_id)
                .transpose()?,
        })
        .await?;

        Ok(comment.into())
    }

    async fn upload_attachment(
        &self,
        config: &ProjectIntegrationConfig,
        input: UploadExternalAttachment,
    ) -> anyhow::Result<ExternalAttachmentUpload> {
        let project_id = gitlab_project_id(config)?;
        let upload = upload_project_attachment(GitLabUploadAttachmentInput {
            gitlab_api_base_url: config.api_base_url.clone(),
            gitlab_project_id: project_id,
            token: config.token.clone(),
            verify_tls: config.verify_tls,
            file_path: input.file_path,
            filename: input.filename,
            content_type: input.content_type,
        })
        .await?;

        Ok(ExternalAttachmentUpload { url: upload.url })
    }
}

pub async fn validate_integration(input: GitLabValidationInput) -> anyhow::Result<GitLabValidationResult> {
    let project = fetch_project(input).await?;

    Ok(GitLabValidationResult {
        project_name: project.name,
        web_url: project.web_url,
        visibility: project.visibility,
    })
}

impl From<GitLabValidationResult> for ExternalValidationResult {
    fn from(value: GitLabValidationResult) -> Self {
        Self {
            project_name: value.project_name,
            web_url: value.web_url,
            visibility: value.visibility,
        }
    }
}

impl From<GitLabIssueSummary> for ExternalIssue {
    fn from(value: GitLabIssueSummary) -> Self {
        Self {
            provider: IntegrationProvider::GitLab,
            external_id: value.iid.to_string(),
            external_key: Some(format!("#{}", value.iid)),
            title: value.title,
            description: value.description,
            state: value.state,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<GitLabIssueCommentSummary> for ExternalComment {
    fn from(value: GitLabIssueCommentSummary) -> Self {
        Self {
            provider: IntegrationProvider::GitLab,
            external_id: value.note_id.to_string(),
            discussion_id: value.discussion_id,
            reply_to_external_id: value.reply_to_note_id.map(|note_id| note_id.to_string()),
            author_external_id: value.author_external_id,
            author_name: value.author_name,
            body_raw: value.body_raw,
            system_note: value.system_note,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

fn gitlab_validation_input(config: &ProjectIntegrationConfig) -> anyhow::Result<GitLabValidationInput> {
    Ok(GitLabValidationInput {
        gitlab_api_base_url: config.api_base_url.clone(),
        gitlab_project_id: gitlab_project_id(config)?,
        token: config.token.clone(),
        verify_tls: config.verify_tls,
    })
}

fn gitlab_issue_import_input(config: &ProjectIntegrationConfig) -> anyhow::Result<GitLabIssueImportInput> {
    Ok(GitLabIssueImportInput {
        gitlab_api_base_url: config.api_base_url.clone(),
        gitlab_project_id: gitlab_project_id(config)?,
        token: config.token.clone(),
        verify_tls: config.verify_tls,
    })
}

fn gitlab_project_id(config: &ProjectIntegrationConfig) -> anyhow::Result<i64> {
    if config.provider != IntegrationProvider::GitLab {
        return Err(anyhow!("GitLab adapter cannot handle {} integration", config.provider));
    }

    config.external_project_id_as_i64()
}

fn parse_gitlab_issue_iid(value: &str) -> anyhow::Result<i64> {
    value
        .trim_start_matches('#')
        .parse::<i64>()
        .map_err(|error| anyhow!("invalid GitLab issue iid {value}: {error}"))
}

fn parse_gitlab_note_id(value: &str) -> anyhow::Result<i64> {
    value
        .trim_start_matches('#')
        .parse::<i64>()
        .map_err(|error| anyhow!("invalid GitLab note id {value}: {error}"))
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
