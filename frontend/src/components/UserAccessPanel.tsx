import { Button, Divider, Group, ScrollArea, Select, Stack, Table, Text, Title } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';

import { useAppContext } from '../context/AppContext';
import {
  getIssuePermissionInfo,
  getIssuePermissionOptions,
  getProjectPermissionInfo,
  getProjectPermissionOptions,
  PermissionSummary,
} from '../lib/permission-ui';
import type { UserAccessIssueOption, UserAccessIssuePermission, UserAccessOverview, UserAccessProjectPermission } from '../types';

type UserAccessPanelProps = {
  userId: string;
};

export function UserAccessPanel({ userId }: UserAccessPanelProps) {
  const { getUserAccess, updateUserAccess, t } = useAppContext();

  const [overview, setOverview] = useState<UserAccessOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [assignments, setAssignments] = useState<UserAccessProjectPermission[]>([]);
  const [newProjectId, setNewProjectId] = useState<string | null>(null);
  const [newPermission, setNewPermission] = useState<string>('view');
  const [issueAssignments, setIssueAssignments] = useState<UserAccessIssuePermission[]>([]);
  const [newIssueId, setNewIssueId] = useState<string | null>(null);
  const [newIssuePermission, setNewIssuePermission] = useState<string>('comment');

  useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [userId]);

  async function load(signal?: AbortSignal) {
    try {
      setLoading(true);
      setError(null);
      const data = await getUserAccess(userId, signal);
      setOverview(data);
      setAssignments(data.project_permissions);
      setIssueAssignments(data.issue_permissions);
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return;
      setError(err instanceof Error ? err.message : 'Nepodařilo se načíst přístupy.');
    } finally {
      setLoading(false);
    }
  }

  const allProjectOptions = useMemo(
    () => {
      const options = [
        ...(overview?.available_projects ?? []),
        ...(overview?.project_permissions.map((p) => ({
          project_id: p.project_id,
          project_name: p.project_name,
        })) ?? []),
      ];
      const seen = new Set<string>();
      return options.filter((option) => {
        if (seen.has(option.project_id)) return false;
        seen.add(option.project_id);
        return true;
      });
    },
    [overview],
  );

  const availableOptions = useMemo(
    () =>
      allProjectOptions
        .filter((p) => !assignments.some((a) => a.project_id === p.project_id))
        .map((p) => ({
          value: p.project_id,
          label: p.project_name,
        })),
    [allProjectOptions, assignments],
  );

  const allIssueOptions = useMemo<UserAccessIssueOption[]>(() => {
    const options = [
      ...(overview?.available_issues ?? []),
      ...(overview?.issue_permissions.map((issue) => ({
        issue_id: issue.issue_id,
        issue_title: issue.issue_title,
        gitlab_issue_iid: issue.gitlab_issue_iid,
        project_id: issue.project_id,
        project_name: issue.project_name,
      })) ?? []),
    ];
    const seen = new Set<string>();
    return options.filter((option) => {
      if (seen.has(option.issue_id)) return false;
      seen.add(option.issue_id);
      return true;
    });
  }, [overview]);

  const availableIssueOptions = useMemo(
    () =>
      allIssueOptions
        .filter((issue) => !issueAssignments.some((assignment) => assignment.issue_id === issue.issue_id))
        .map((issue) => ({
          value: issue.issue_id,
          label: `${formatIssueReference(issue)} - ${issue.project_name} - ${issue.issue_title}`,
        })),
    [allIssueOptions, issueAssignments, t],
  );

  const permissionOptions = useMemo(() => getProjectPermissionOptions(t), [t]);
  const issuePermissionOptions = useMemo(() => getIssuePermissionOptions(t), [t]);

  function addAssignment() {
    if (!newProjectId || !overview) return;
    if (assignments.some((a) => a.project_id === newProjectId)) return;
    const project = allProjectOptions.find((p) => p.project_id === newProjectId);
    if (!project) return;
    setAssignments((current) => [
      ...current,
      { project_id: project.project_id, project_name: project.project_name, permission: newPermission },
    ]);
    setNewProjectId(null);
    setNewPermission('view');
  }

  function updateAssignment(projectId: string, permission: string) {
    setAssignments((current) =>
      current.map((a) => (a.project_id === projectId ? { ...a, permission } : a)),
    );
  }

  function removeAssignment(projectId: string) {
    setAssignments((current) => current.filter((a) => a.project_id !== projectId));
  }

  function addIssueAssignment() {
    if (!newIssueId) return;
    if (issueAssignments.some((assignment) => assignment.issue_id === newIssueId)) return;
    const issue = allIssueOptions.find((item) => item.issue_id === newIssueId);
    if (!issue) return;

    setIssueAssignments((current) => [
      ...current,
      {
        issue_id: issue.issue_id,
        issue_title: issue.issue_title,
        gitlab_issue_iid: issue.gitlab_issue_iid,
        project_id: issue.project_id,
        project_name: issue.project_name,
        permission: newIssuePermission,
      },
    ]);
    setNewIssueId(null);
    setNewIssuePermission('comment');
  }

  function updateIssueAssignment(issueId: string, permission: string) {
    setIssueAssignments((current) =>
      current.map((assignment) => (assignment.issue_id === issueId ? { ...assignment, permission } : assignment)),
    );
  }

  function removeIssueAssignment(issueId: string) {
    setIssueAssignments((current) => current.filter((assignment) => assignment.issue_id !== issueId));
  }

  function formatIssueReference(issue: { gitlab_issue_iid: number; issue_id: string }) {
    return issue.gitlab_issue_iid > 0 ? `#${issue.gitlab_issue_iid}` : `${t('issues.localNumber')} ${issue.issue_id.slice(0, 8)}`;
  }

  async function handleSave() {
    setSaving(true);
    try {
      const updated = await updateUserAccess(userId, {
        project_permissions: assignments.map((a) => ({
          project_id: a.project_id,
          permission: a.permission,
        })),
        issue_permissions: issueAssignments.map((assignment) => ({
          issue_id: assignment.issue_id,
          permission: assignment.permission,
        })),
      });
      setError(null);
      setOverview(updated);
      setAssignments(updated.project_permissions);
      setIssueAssignments(updated.issue_permissions);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Nepodařilo se uložit přístupy.');
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return <Text c="dimmed" size="sm">Načítám přístupy...</Text>;
  }

  if (error) {
    return <Text c="red" size="sm">{error}</Text>;
  }

  return (
    <Stack gap="md">
      <Title order={4}>{t('access.projectTitle')}</Title>
      <Text c="dimmed" size="sm">
        {t('access.projectDescription')}
      </Text>

      <Group align="flex-end">
        <Select
          label={t('access.project')}
          placeholder={t('access.projectPlaceholder')}
          data={availableOptions}
          value={newProjectId}
          onChange={setNewProjectId}
          searchable
          flex={1}
        />
        <Select
          label={t('common.permission')}
          data={permissionOptions}
          value={newPermission}
          onChange={(value) => setNewPermission(value ?? 'view')}
          w={220}
        />
        <Button variant="light" onClick={addAssignment} disabled={!newProjectId}>
          {t('access.add')}
        </Button>
      </Group>

      {assignments.length > 0 ? (
        <ScrollArea>
          <Table verticalSpacing="sm" highlightOnHover miw={640}>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t('access.project')}</Table.Th>
                <Table.Th>{t('common.permission')}</Table.Th>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {assignments.map((assignment) => (
                <Table.Tr key={assignment.project_id}>
                  <Table.Td>
                    <Text fw={500}>{assignment.project_name}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Stack gap="xs">
                      <Select
                        data={permissionOptions}
                        value={assignment.permission}
                        onChange={(value) => updateAssignment(assignment.project_id, value ?? 'view')}
                        w={260}
                      />
                      <PermissionSummary info={getProjectPermissionInfo(assignment.permission, t)} />
                    </Stack>
                  </Table.Td>
                  <Table.Td>
                    <Button
                      color="red"
                      variant="subtle"
                      onClick={() => removeAssignment(assignment.project_id)}
                    >
                      {t('common.remove')}
                    </Button>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </ScrollArea>
      ) : (
        <Text c="dimmed" size="sm">{t('access.noUserProjectAssignments')}</Text>
      )}

      <Divider />

      <Title order={4}>{t('access.issuePermissionsTitle')}</Title>
      <Text c="dimmed" size="sm">
        {t('access.issuePermissionsDescription')}
      </Text>

      <Group align="flex-end">
        <Select
          label={t('access.issue')}
          placeholder={t('access.issuePlaceholder')}
          data={availableIssueOptions}
          value={newIssueId}
          onChange={setNewIssueId}
          searchable
          flex={1}
          limit={50}
        />
        <Select
          label={t('common.permission')}
          data={issuePermissionOptions}
          value={newIssuePermission}
          onChange={(value) => setNewIssuePermission(value ?? 'comment')}
          w={220}
        />
        <Button variant="light" onClick={addIssueAssignment} disabled={!newIssueId}>
          {t('access.addIssueAccess')}
        </Button>
      </Group>

      {issueAssignments.length > 0 ? (
        <ScrollArea>
          <Table verticalSpacing="sm" highlightOnHover miw={760}>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t('access.issue')}</Table.Th>
                <Table.Th>{t('access.project')}</Table.Th>
                <Table.Th>{t('common.permission')}</Table.Th>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {issueAssignments.map((ip) => (
                <Table.Tr key={ip.issue_id}>
                  <Table.Td>
                    <Stack gap={2}>
                      <Text size="sm" fw={500}>{formatIssueReference(ip)}</Text>
                      <Text size="sm">{ip.issue_title}</Text>
                    </Stack>
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm" c="dimmed">{ip.project_name}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Stack gap="xs">
                      <Select
                        data={issuePermissionOptions}
                        value={ip.permission}
                        onChange={(value) => updateIssueAssignment(ip.issue_id, value ?? 'comment')}
                        w={220}
                      />
                      <PermissionSummary info={getIssuePermissionInfo(ip.permission, t)} />
                    </Stack>
                  </Table.Td>
                  <Table.Td>
                    <Button
                      color="red"
                      variant="subtle"
                      size="xs"
                      onClick={() => removeIssueAssignment(ip.issue_id)}
                    >
                      {t('common.remove')}
                    </Button>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </ScrollArea>
      ) : (
        <Text c="dimmed" size="sm">{t('access.noUserIssueAssignments')}</Text>
      )}

      <Group justify="flex-end">
        <Button loading={saving} onClick={() => void handleSave()}>
          {t('access.save')}
        </Button>
      </Group>
    </Stack>
  );
}
