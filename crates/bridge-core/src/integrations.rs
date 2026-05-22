use std::{fmt, str::FromStr};

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationProvider {
    GitLab,
    Redmine,
}

impl IntegrationProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitLab => "gitlab",
            Self::Redmine => "redmine",
        }
    }
}

impl fmt::Display for IntegrationProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for IntegrationProvider {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gitlab" => Ok(Self::GitLab),
            "redmine" => Ok(Self::Redmine),
            other => Err(anyhow!("unsupported integration provider {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectIntegrationConfig {
    pub provider: IntegrationProvider,
    pub base_url: String,
    pub api_base_url: String,
    pub external_project_id: String,
    pub token: String,
    pub verify_tls: bool,
    pub sync_enabled: bool,
}

impl ProjectIntegrationConfig {
    pub fn gitlab(
        base_url: impl Into<String>,
        api_base_url: impl Into<String>,
        project_id: i64,
        token: impl Into<String>,
        verify_tls: bool,
        sync_enabled: bool,
    ) -> Self {
        Self {
            provider: IntegrationProvider::GitLab,
            base_url: base_url.into(),
            api_base_url: api_base_url.into(),
            external_project_id: project_id.to_string(),
            token: token.into(),
            verify_tls,
            sync_enabled,
        }
    }

    pub fn external_project_id_as_i64(&self) -> anyhow::Result<i64> {
        self.external_project_id
            .parse::<i64>()
            .map_err(|error| anyhow!("invalid external project id {}: {error}", self.external_project_id))
    }
}

#[derive(Debug, Clone)]
pub struct ExternalValidationResult {
    pub project_name: String,
    pub web_url: String,
    pub visibility: String,
}

#[derive(Debug, Clone)]
pub struct ExternalIssue {
    pub provider: IntegrationProvider,
    pub external_id: String,
    pub external_key: Option<String>,
    pub title: String,
    pub description: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExternalComment {
    pub provider: IntegrationProvider,
    pub external_id: String,
    pub discussion_id: Option<String>,
    pub reply_to_external_id: Option<String>,
    pub author_external_id: String,
    pub author_name: String,
    pub body_raw: String,
    pub system_note: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateExternalIssue {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct UpdateExternalIssue {
    pub title: String,
    pub description: String,
    pub state_event: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateExternalComment {
    pub external_issue_id: String,
    pub body: String,
    pub discussion_id: Option<String>,
    pub reply_to_external_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadExternalAttachment {
    pub file_path: String,
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct ExternalAttachmentUpload {
    pub url: String,
}

#[allow(async_fn_in_trait)]
pub trait IssueTrackerAdapter {
    fn provider(&self) -> IntegrationProvider;

    async fn validate(
        &self,
        config: &ProjectIntegrationConfig,
    ) -> anyhow::Result<ExternalValidationResult>;

    async fn import_issues(
        &self,
        config: &ProjectIntegrationConfig,
    ) -> anyhow::Result<Vec<ExternalIssue>>;

    async fn fetch_issue(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
    ) -> anyhow::Result<ExternalIssue>;

    async fn create_issue(
        &self,
        config: &ProjectIntegrationConfig,
        input: CreateExternalIssue,
    ) -> anyhow::Result<ExternalIssue>;

    async fn update_issue(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
        input: UpdateExternalIssue,
    ) -> anyhow::Result<ExternalIssue>;

    async fn import_comments(
        &self,
        config: &ProjectIntegrationConfig,
        external_issue_id: &str,
    ) -> anyhow::Result<Vec<ExternalComment>>;

    async fn create_comment(
        &self,
        config: &ProjectIntegrationConfig,
        input: CreateExternalComment,
    ) -> anyhow::Result<ExternalComment>;

    async fn upload_attachment(
        &self,
        config: &ProjectIntegrationConfig,
        input: UploadExternalAttachment,
    ) -> anyhow::Result<ExternalAttachmentUpload>;
}
