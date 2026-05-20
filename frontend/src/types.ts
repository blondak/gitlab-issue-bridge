export type Overview = {
  project_count: number;
  integrated_project_count: number;
  issue_count: number;
  pending_jobs: number;
  processing_jobs: number;
};

export type HealthCheck = {
  name: string;
  status: string;
  message: string | null;
};

export type QueueHealth = {
  pending_jobs: number;
  processing_jobs: number;
  done_jobs: number;
  dead_jobs: number;
  stale_processing_jobs: number;
  oldest_pending_seconds: number;
  smtp_failed_jobs: number;
  webhook_failed_jobs: number;
};

export type WorkerHeartbeat = {
  worker_id: string;
  status: string;
  healthy: boolean;
  heartbeat_age_seconds: number;
  heartbeat_at: string;
  last_job_id: string | null;
  last_job_topic: string | null;
  last_error: string | null;
  processed_jobs: number;
  failed_jobs: number;
};

export type RecentJobFailure = {
  id: string;
  topic: string;
  status: string;
  attempt_count: number;
  last_error: string | null;
  updated_at: string;
};

export type AdminHealth = {
  status: string;
  service: string;
  generated_at: string;
  checks: HealthCheck[];
  queue: QueueHealth;
  workers: WorkerHeartbeat[];
  recent_failed_jobs: RecentJobFailure[];
};

export type User = {
  id: string;
  email: string;
  full_name: string;
  preferred_language: 'cs' | 'en' | null;
  is_admin: boolean;
};

export type ManagedUser = User & {
  active: boolean;
  created_at: string;
};

export type UserInvitation = {
  id: string;
  email: string;
  is_admin: boolean;
  status: string;
  expires_at: string;
  last_sent_at: string;
  accepted_at: string | null;
  created_at: string;
};

export type InvitationPreview = {
  email: string;
  is_admin: boolean;
  status: string;
  expires_at: string;
};

export type UserManagementOverview = {
  users: ManagedUser[];
  invitations: UserInvitation[];
};

export type ProjectIntegration = {
  id: string;
  gitlab_base_url: string;
  gitlab_api_base_url: string;
  gitlab_project_id: number;
  verify_tls: boolean;
  sync_enabled: boolean;
};

export type ProjectCapabilities = {
  can_view: boolean;
  can_create_issue: boolean;
  can_manage: boolean;
};

export type Project = {
  id: string;
  slug: string;
  name: string;
  description: string;
  active: boolean;
  gitlab_integration: ProjectIntegration | null;
  capabilities: ProjectCapabilities;
};

export type ProjectAccessAssignment = {
  subject_type: 'user' | 'email';
  subject_id: string;
  display_name: string;
  email: string;
  permission: string;
};

export type ProjectAccessSubjectOption = {
  subject_type: 'user' | 'email';
  subject_id: string;
  display_name: string;
  email: string;
};

export type ProjectIssuePermission = {
  issue_id: string;
  issue_title: string;
  subject_type: string;
  subject_id: string;
  subject_display_name: string;
  subject_email: string;
  permission: string;
};

export type ProjectAccessOverview = {
  assignments: ProjectAccessAssignment[];
  available_subjects: ProjectAccessSubjectOption[];
  issue_permissions: ProjectIssuePermission[];
};

export type Issue = {
  id: string;
  project_id: string;
  project_slug: string;
  project_name: string;
  title: string;
  description: string;
  state: string;
  sync_state: string;
  gitlab_issue_iid: number;
  version: number;
  last_activity_at?: string | null;
  capabilities: IssueCapabilities;
};

export type IssueCapabilities = {
  can_view: boolean;
  can_comment: boolean;
  can_edit: boolean;
  can_change_state: boolean;
  can_manage_access: boolean;
  can_sync_comments: boolean;
};

export type Attachment = {
  id: string;
  filename: string;
  content_type: string;
  byte_size: number;
  external_url: string;
  proxy_path: string;
  inline: boolean;
  sync_state: string;
};

export type Comment = {
  id: string;
  gitlab_note_id: number;
  discussion_id?: string | null;
  individual_note?: boolean;
  reply_to_gitlab_note_id?: number | null;
  author_external_id: string;
  author_name: string;
  body_raw: string;
  system_note: boolean;
  sync_state: string;
  attachments: Attachment[];
  created_at: string;
};

export type IssueDetailData = {
  issue: Issue;
  comments: Comment[];
  issue_attachments: Attachment[];
};

export type IssueAccessAssignment = {
  user_id: string;
  email: string;
  full_name: string;
  permission: string;
};

export type IssueAccessUserOption = {
  id: string;
  email: string;
  full_name: string;
};

export type IssueAccessOverview = {
  assignments: IssueAccessAssignment[];
  available_users: IssueAccessUserOption[];
};

export type ProjectFormValues = {
  slug: string;
  name: string;
  description: string;
  active: boolean;
};

export type IntegrationFormValues = {
  gitlab_base_url: string;
  gitlab_api_base_url: string;
  gitlab_project_id: string;
  token: string;
  webhook_secret: string;
  verify_tls: boolean;
  sync_enabled: boolean;
};

export type GitLabIntegrationValidationResult = {
  valid: boolean;
  project_name: string;
  web_url: string;
  visibility: string;
};

export type GitLabIssueImportResult = {
  imported_count: number;
  created_count: number;
  updated_count: number;
};

export type GitLabCommentImportResult = {
  imported_count: number;
  created_count: number;
  updated_count: number;
};

export type UpdateUserValues = {
  full_name: string;
  is_admin: boolean;
  active: boolean;
};

export type CreateInvitationValues = {
  email: string;
  is_admin: boolean;
};

export type AcceptInvitationValues = {
  full_name: string;
  password: string;
};

export type CreateIssueValues = {
  project_id: string;
  title: string;
  description: string;
};

export type UpdateIssueValues = {
  title?: string;
  description?: string;
  state?: string;
};

export type UpdateIssueAccessValues = {
  assignments: Array<{
    user_id: string;
    permission: string;
  }>;
};

export type UpdateProfileValues = {
  full_name: string;
  preferred_language: 'cs' | 'en' | null;
};

export type ChangePasswordValues = {
  current_password: string;
  new_password: string;
};

export type PasswordRecoveryRequestValues = {
  email: string;
};

export type PasswordRecoveryPreview = {
  email: string;
  expires_at: string;
};

export type PasswordResetValues = {
  password: string;
};

export type IssueUpload = {
  upload_id: string;
  filename: string;
  content_type: string;
  byte_size: number;
  proxy_path: string;
  markdown: string;
};

export type CreateCommentValues = {
  body: string;
  reply_to_note_id?: number | null;
};

export type ProjectEditorValues = {
  slug: string;
  name: string;
  description: string;
  active: boolean;
  enable_gitlab_integration: boolean;
  gitlab_base_url: string;
  gitlab_api_base_url: string;
  gitlab_project_id: string;
  token: string;
  webhook_secret: string;
  verify_tls: boolean;
  sync_enabled: boolean;
};

export type UpdateProjectAccessValues = {
  assignments: Array<{
    subject_type: 'user' | 'email';
    subject_id: string;
    permission: string;
  }>;
};

export type UserAccessProjectPermission = {
  project_id: string;
  project_name: string;
  permission: string;
};

export type UserAccessIssuePermission = {
  issue_id: string;
  issue_title: string;
  project_name: string;
  permission: string;
};

export type UserAccessProjectOption = {
  project_id: string;
  project_name: string;
};

export type UserAccessOverview = {
  project_permissions: UserAccessProjectPermission[];
  issue_permissions: UserAccessIssuePermission[];
  available_projects: UserAccessProjectOption[];
};

export type UpdateUserAccessValues = {
  project_permissions: Array<{
    project_id: string;
    permission: string;
  }>;
};
