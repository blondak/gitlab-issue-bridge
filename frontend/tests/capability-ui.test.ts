import assert from 'node:assert/strict';
import test from 'node:test';

import {
  getCreatableProjects,
  getIssueDetailActionVisibility,
} from '../src/lib/capability-ui';
import type { IssueCapabilities, Project } from '../src/types';

test('getCreatableProjects only returns projects with explicit create capability', () => {
  const projects = [
    project('view-only', false),
    project('create-only', true),
    project('admin', true),
  ];

  assert.deepEqual(
    getCreatableProjects(projects).map((item) => item.slug),
    ['create-only', 'admin'],
  );
});

test('issue detail actions stay hidden when handlers are not wired', () => {
  const visibility = getIssueDetailActionVisibility(issueCapabilities('admin'), {
    canUpdateIssue: false,
    canCreateComment: false,
    canSyncComments: false,
  });

  assert.deepEqual(visibility, {
    showEditIssue: false,
    showChangeIssueState: false,
    showCommentEditor: false,
    showSyncComments: false,
    showAccessPanel: true,
  });
});

test('issue read permission only shows read-only issue detail actions', () => {
  const visibility = getIssueDetailActionVisibility(issueCapabilities('read'), {
    canUpdateIssue: true,
    canCreateComment: true,
    canSyncComments: true,
  });

  assert.deepEqual(visibility, {
    showEditIssue: false,
    showChangeIssueState: false,
    showCommentEditor: false,
    showSyncComments: false,
    showAccessPanel: false,
  });
});

test('issue comment permission can comment but cannot edit or close', () => {
  const visibility = getIssueDetailActionVisibility(issueCapabilities('comment'), {
    canUpdateIssue: true,
    canCreateComment: true,
    canSyncComments: true,
  });

  assert.deepEqual(visibility, {
    showEditIssue: false,
    showChangeIssueState: false,
    showCommentEditor: true,
    showSyncComments: false,
    showAccessPanel: false,
  });
});

test('issue edit permission can edit and change state without access management', () => {
  const visibility = getIssueDetailActionVisibility(issueCapabilities('edit'), {
    canUpdateIssue: true,
    canCreateComment: true,
    canSyncComments: true,
  });

  assert.deepEqual(visibility, {
    showEditIssue: true,
    showChangeIssueState: true,
    showCommentEditor: true,
    showSyncComments: false,
    showAccessPanel: false,
  });
});

test('issue admin permission exposes access management and sync only when enabled', () => {
  const withoutSync = getIssueDetailActionVisibility(issueCapabilities('admin', false), {
    canUpdateIssue: true,
    canCreateComment: true,
    canSyncComments: true,
  });
  const withSync = getIssueDetailActionVisibility(issueCapabilities('admin', true), {
    canUpdateIssue: true,
    canCreateComment: true,
    canSyncComments: true,
  });

  assert.equal(withoutSync.showAccessPanel, true);
  assert.equal(withoutSync.showSyncComments, false);
  assert.equal(withSync.showAccessPanel, true);
  assert.equal(withSync.showSyncComments, true);
});

function project(slug: string, canCreateIssue: boolean): Project {
  return {
    id: slug,
    slug,
    name: slug,
    description: '',
    active: true,
    gitlab_integration: null,
    capabilities: {
      can_view: true,
      can_create_issue: canCreateIssue,
      can_manage: slug === 'admin',
    },
  };
}

function issueCapabilities(permission: 'read' | 'comment' | 'edit' | 'admin', sync = false): IssueCapabilities {
  return {
    can_view: true,
    can_comment: permission === 'comment' || permission === 'edit' || permission === 'admin',
    can_edit: permission === 'edit' || permission === 'admin',
    can_change_state: permission === 'edit' || permission === 'admin',
    can_manage_access: permission === 'admin',
    can_sync_comments: permission === 'admin' && sync,
  };
}
