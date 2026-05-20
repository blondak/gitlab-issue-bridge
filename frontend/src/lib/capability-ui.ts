import type { IssueCapabilities, Project } from '../types';

export type IssueDetailHandlerAvailability = {
  canUpdateIssue: boolean;
  canCreateComment: boolean;
  canSyncComments: boolean;
};

export type IssueDetailActionVisibility = {
  showEditIssue: boolean;
  showChangeIssueState: boolean;
  showCommentEditor: boolean;
  showSyncComments: boolean;
  showAccessPanel: boolean;
};

export function getCreatableProjects(projects: Project[]) {
  return projects.filter((project) => project.capabilities.can_create_issue);
}

export function getIssueDetailActionVisibility(
  capabilities: IssueCapabilities | null | undefined,
  handlers: IssueDetailHandlerAvailability,
): IssueDetailActionVisibility {
  return {
    showEditIssue: Boolean(capabilities?.can_edit && handlers.canUpdateIssue),
    showChangeIssueState: Boolean(capabilities?.can_change_state && handlers.canUpdateIssue),
    showCommentEditor: Boolean(capabilities?.can_comment && handlers.canCreateComment),
    showSyncComments: Boolean(capabilities?.can_sync_comments && handlers.canSyncComments),
    showAccessPanel: Boolean(capabilities?.can_manage_access),
  };
}
