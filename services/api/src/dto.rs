use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub password_hash: String,
    pub preferred_language: Option<String>,
    pub is_admin: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub preferred_language: Option<String>,
    pub is_admin: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PasswordResetTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PasswordRecoveryPreviewDto {
    pub email: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ManagedUserDto {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub preferred_language: Option<String>,
    pub is_admin: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserInvitationRow {
    pub id: Uuid,
    pub email: String,
    pub invited_by_user_id: Uuid,
    pub is_admin: bool,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub last_sent_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserInvitationDto {
    pub id: Uuid,
    pub email: String,
    pub is_admin: bool,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub last_sent_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserManagementOverviewDto {
    pub users: Vec<ManagedUserDto>,
    pub invitations: Vec<UserInvitationDto>,
}

#[derive(Debug, Serialize)]
pub struct InvitationPreviewDto {
    pub email: String,
    pub is_admin: bool,
    pub status: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct IssueRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_slug: String,
    pub project_name: String,
    pub gitlab_issue_iid: i64,
    pub title: String,
    pub description: String,
    pub state: String,
    pub sync_state: String,
    pub last_source: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct IssueDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_slug: String,
    pub project_name: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub sync_state: String,
    pub gitlab_issue_iid: i64,
    pub version: i64,
    pub last_activity_at: DateTime<Utc>,
    pub capabilities: IssueCapabilitiesDto,
}

#[derive(Debug, Serialize, Clone)]
pub struct IssueCapabilitiesDto {
    pub can_view: bool,
    pub can_comment: bool,
    pub can_edit: bool,
    pub can_change_state: bool,
    pub can_manage_access: bool,
    pub can_sync_comments: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct ProjectIntegrationRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub gitlab_base_url: String,
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token_encrypted: Option<String>,
    pub webhook_secret_encrypted: Option<String>,
    pub verify_tls: bool,
    pub sync_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub gitlab_integration: Option<ProjectIntegrationDto>,
    pub capabilities: ProjectCapabilitiesDto,
}

#[derive(Debug, Serialize)]
pub struct ProjectIntegrationDto {
    pub id: Uuid,
    pub gitlab_base_url: String,
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub verify_tls: bool,
    pub sync_enabled: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProjectCapabilitiesDto {
    pub can_view: bool,
    pub can_create_issue: bool,
    pub can_manage: bool,
}

#[derive(Debug, Serialize)]
pub struct GitLabIntegrationValidationResponse {
    pub valid: bool,
    pub project_name: String,
    pub web_url: String,
    pub visibility: String,
}

#[derive(Debug, Serialize)]
pub struct GitLabIssueImportResponse {
    pub imported_count: usize,
    pub created_count: usize,
    pub updated_count: usize,
}

#[derive(Debug, Serialize)]
pub struct GitLabCommentImportResponse {
    pub imported_count: usize,
    pub created_count: usize,
    pub updated_count: usize,
}

#[derive(Debug, Serialize)]
pub struct GitLabWebhookResponse {
    pub status: String,
    pub event_type: String,
    pub handled: bool,
    pub job_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub issue_iid: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueCommentRequest {
    pub body: String,
    pub reply_to_note_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct IssueUploadDto {
    pub upload_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub proxy_path: String,
    pub markdown: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordRecoveryRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub password: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CommentRow {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub gitlab_note_id: i64,
    pub discussion_id: Option<String>,
    pub individual_note: bool,
    pub reply_to_gitlab_note_id: Option<i64>,
    pub author_external_id: String,
    pub author_name: String,
    pub body_raw: String,
    pub system_note: bool,
    pub sync_state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AttachmentRow {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub comment_id: Option<Uuid>,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub external_url: String,
    pub proxy_path: String,
    pub storage_backend: String,
    pub storage_path: Option<String>,
    pub inline: bool,
    pub created_by_external_id: String,
    pub sync_state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentDto {
    pub id: Uuid,
    pub gitlab_note_id: i64,
    pub discussion_id: Option<String>,
    pub individual_note: bool,
    pub reply_to_gitlab_note_id: Option<i64>,
    pub author_external_id: String,
    pub author_name: String,
    pub body_raw: String,
    pub system_note: bool,
    pub sync_state: String,
    pub attachments: Vec<AttachmentDto>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AttachmentDto {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub external_url: String,
    pub proxy_path: String,
    pub inline: bool,
    pub sync_state: String,
}

#[derive(Debug, Serialize)]
pub struct IssueDetailDto {
    pub issue: IssueDto,
    pub comments: Vec<CommentDto>,
    pub issue_attachments: Vec<AttachmentDto>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct IssuePermissionRow {
    pub issue_id: Uuid,
    pub subject_type: String,
    pub subject_id: String,
    pub permission: String,
    pub effect: String,
}

#[derive(Debug, Serialize)]
pub struct IssueAccessAssignmentDto {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub permission: String,
}

#[derive(Debug, Serialize)]
pub struct IssueAccessUserOptionDto {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
}

#[derive(Debug, Serialize)]
pub struct IssueAccessOverviewDto {
    pub assignments: Vec<IssueAccessAssignmentDto>,
    pub available_users: Vec<IssueAccessUserOptionDto>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProjectPermissionRow {
    pub project_id: Uuid,
    pub subject_type: String,
    pub subject_id: String,
    pub permission: String,
    pub effect: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectAccessAssignmentDto {
    pub subject_type: String,
    pub subject_id: String,
    pub display_name: String,
    pub email: String,
    pub permission: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectAccessUserOptionDto {
    pub subject_type: String,
    pub subject_id: String,
    pub display_name: String,
    pub email: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProjectIssuePermissionDto {
    pub issue_id: Uuid,
    pub issue_title: String,
    pub subject_type: String,
    pub subject_id: String,
    pub subject_display_name: String,
    pub subject_email: String,
    pub permission: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectAccessOverviewDto {
    pub assignments: Vec<ProjectAccessAssignmentDto>,
    pub available_subjects: Vec<ProjectAccessUserOptionDto>,
    pub issue_permissions: Vec<ProjectIssuePermissionDto>,
}

#[derive(Debug, Serialize)]
pub struct OverviewResponse {
    pub project_count: i64,
    pub integrated_project_count: i64,
    pub issue_count: i64,
    pub pending_jobs: i64,
    pub processing_jobs: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertProjectGitLabIntegrationRequest {
    pub gitlab_base_url: String,
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: String,
    pub webhook_secret: String,
    pub verify_tls: bool,
    pub sync_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ValidateProjectGitLabIntegrationRequest {
    pub gitlab_base_url: String,
    pub gitlab_api_base_url: String,
    pub gitlab_project_id: i64,
    pub token: Option<String>,
    pub verify_tls: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectIssueRequest {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueAccessAssignmentRequest {
    pub user_id: Uuid,
    pub permission: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIssueAccessRequest {
    pub assignments: Vec<UpdateIssueAccessAssignmentRequest>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectAccessAssignmentRequest {
    pub subject_type: String,
    pub subject_id: String,
    pub permission: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectAccessRequest {
    pub assignments: Vec<UpdateProjectAccessAssignmentRequest>,
}

#[derive(Debug, Deserialize)]
pub struct EnqueueJobRequest {
    pub topic: String,
    pub payload: Value,
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct JobListItemDto {
    pub id: Uuid,
    pub topic: String,
    pub status: String,
    pub attempt_count: i32,
    pub locked_by: Option<String>,
    pub dedupe_key: Option<String>,
    pub last_error: Option<String>,
    pub available_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Serialize)]
pub struct HealthCheckDto {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub status: String,
    pub service: String,
    pub checks: Vec<HealthCheckDto>,
}

#[derive(Debug, Serialize)]
pub struct QueueHealthDto {
    pub pending_jobs: i64,
    pub processing_jobs: i64,
    pub done_jobs: i64,
    pub dead_jobs: i64,
    pub stale_processing_jobs: i64,
    pub oldest_pending_seconds: i64,
    pub smtp_failed_jobs: i64,
    pub webhook_failed_jobs: i64,
}

#[derive(Debug, Serialize)]
pub struct WorkerHeartbeatDto {
    pub worker_id: String,
    pub status: String,
    pub healthy: bool,
    pub heartbeat_age_seconds: i64,
    pub heartbeat_at: DateTime<Utc>,
    pub last_job_id: Option<Uuid>,
    pub last_job_topic: Option<String>,
    pub last_error: Option<String>,
    pub processed_jobs: i64,
    pub failed_jobs: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct RecentJobFailureDto {
    pub id: Uuid,
    pub topic: String,
    pub status: String,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminHealthResponse {
    pub status: String,
    pub service: String,
    pub generated_at: DateTime<Utc>,
    pub checks: Vec<HealthCheckDto>,
    pub queue: QueueHealthDto,
    pub workers: Vec<WorkerHeartbeatDto>,
    pub recent_failed_jobs: Vec<RecentJobFailureDto>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub full_name: String,
    pub preferred_language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserDto,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub full_name: String,
    pub is_admin: bool,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserAccessProjectPermissionRow {
    pub project_id: Uuid,
    pub project_name: String,
    pub permission: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserAccessIssuePermissionRow {
    pub issue_id: Uuid,
    pub issue_title: String,
    pub project_name: String,
    pub permission: String,
}

#[derive(Debug, Serialize)]
pub struct UserAccessProjectOptionDto {
    pub project_id: Uuid,
    pub project_name: String,
}

#[derive(Debug, Serialize)]
pub struct UserAccessOverviewDto {
    pub project_permissions: Vec<UserAccessProjectPermissionRow>,
    pub issue_permissions: Vec<UserAccessIssuePermissionRow>,
    pub available_projects: Vec<UserAccessProjectOptionDto>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserProjectAccessEntry {
    pub project_id: Uuid,
    pub permission: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserAccessRequest {
    pub project_permissions: Vec<UpdateUserProjectAccessEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInvitationRequest {
    pub full_name: String,
    pub password: String,
}

impl From<UserRow> for UserDto {
    fn from(user: UserRow) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            preferred_language: user.preferred_language,
            is_admin: user.is_admin,
        }
    }
}

impl From<UserRow> for ManagedUserDto {
    fn from(user: UserRow) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            preferred_language: user.preferred_language,
            is_admin: user.is_admin,
            active: user.active,
            created_at: user.created_at,
        }
    }
}

impl From<UserInvitationRow> for UserInvitationDto {
    fn from(invitation: UserInvitationRow) -> Self {
        Self {
            id: invitation.id,
            email: invitation.email,
            is_admin: invitation.is_admin,
            status: invitation.status,
            expires_at: invitation.expires_at,
            last_sent_at: invitation.last_sent_at,
            accepted_at: invitation.accepted_at,
            created_at: invitation.created_at,
        }
    }
}

impl IssueDto {
    pub fn from_parts(issue: IssueRow, capabilities: IssueCapabilitiesDto) -> Self {
        Self {
            id: issue.id,
            project_id: issue.project_id,
            project_slug: issue.project_slug,
            project_name: issue.project_name,
            title: issue.title,
            description: issue.description,
            state: issue.state,
            sync_state: issue.sync_state,
            gitlab_issue_iid: issue.gitlab_issue_iid,
            version: issue.version,
            last_activity_at: issue.updated_at,
            capabilities,
        }
    }
}

impl ProjectDto {
    pub fn from_parts(
        project: ProjectRow,
        integration: Option<ProjectIntegrationRow>,
        capabilities: ProjectCapabilitiesDto,
    ) -> Self {
        Self {
            id: project.id,
            slug: project.slug,
            name: project.name,
            description: project.description,
            active: project.active,
            gitlab_integration: integration.map(|integration| ProjectIntegrationDto {
                id: integration.id,
                gitlab_base_url: integration.gitlab_base_url,
                gitlab_api_base_url: integration.gitlab_api_base_url,
                gitlab_project_id: integration.gitlab_project_id,
                verify_tls: integration.verify_tls,
                sync_enabled: integration.sync_enabled,
            }),
            capabilities,
        }
    }
}

pub fn attachment_to_dto(attachment: &AttachmentRow) -> AttachmentDto {
    AttachmentDto {
        id: attachment.id,
        filename: attachment.filename.clone(),
        content_type: attachment.content_type.clone(),
        byte_size: attachment.byte_size,
        external_url: attachment.external_url.clone(),
        proxy_path: attachment.proxy_path.clone(),
        inline: attachment.inline,
        sync_state: attachment.sync_state.clone(),
    }
}
