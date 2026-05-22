import { Badge, Stack, Text } from '@mantine/core';

type T = (key: string) => string;

type PermissionInfo = {
  label: string;
  summary: string;
  color: string;
};

const projectPermissionKeys = ['view', 'create_issue', 'admin'] as const;
const issuePermissionKeys = ['read', 'comment', 'edit', 'admin'] as const;

const projectPermissionColors: Record<string, string> = {
  view: 'blue',
  create_issue: 'grape',
  admin: 'teal',
};

const issuePermissionColors: Record<string, string> = {
  read: 'blue',
  comment: 'grape',
  edit: 'orange',
  admin: 'teal',
};

export function getProjectPermissionOptions(t: T) {
  return projectPermissionKeys.map((permission) => ({
    value: permission,
    label: t(`permissions.project.${permission}.label`),
  }));
}

export function getIssuePermissionOptions(t: T) {
  return issuePermissionKeys.map((permission) => ({
    value: permission,
    label: t(`permissions.issue.${permission}.label`),
  }));
}

export function getProjectPermissionInfo(permission: string, t: T): PermissionInfo {
  const known = projectPermissionKeys.includes(permission as (typeof projectPermissionKeys)[number])
    ? (permission as (typeof projectPermissionKeys)[number])
    : 'view';

  return {
    label: t(`permissions.project.${known}.label`),
    summary: t(`permissions.project.${known}.summary`),
    color: projectPermissionColors[known] ?? 'gray',
  };
}

export function getIssuePermissionInfo(permission: string, t: T): PermissionInfo {
  const known = issuePermissionKeys.includes(permission as (typeof issuePermissionKeys)[number])
    ? (permission as (typeof issuePermissionKeys)[number])
    : 'read';

  return {
    label: t(`permissions.issue.${known}.label`),
    summary: t(`permissions.issue.${known}.summary`),
    color: issuePermissionColors[known] ?? 'gray',
  };
}

export function PermissionSummary({ info }: { info: PermissionInfo }) {
  return (
    <Stack gap={2}>
      <Badge color={info.color} variant="light" w="fit-content">
        {info.label}
      </Badge>
      <Text size="xs" c="dimmed">
        {info.summary}
      </Text>
    </Stack>
  );
}
