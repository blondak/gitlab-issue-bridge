import { Button, Group, Select, Stack, Table, Text, Title } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';

import { useAppContext } from '../context/AppContext';
import {
  getIssuePermissionInfo,
  getProjectPermissionInfo,
  getProjectPermissionOptions,
  PermissionSummary,
} from '../lib/permission-ui';
import type { UserAccessOverview, UserAccessProjectPermission } from '../types';

type UserAccessPanelProps = {
  userId: string;
};

export function UserAccessPanel({ userId }: UserAccessPanelProps) {
  const { getUserAccess, updateUserAccess, removeIssuePermission, t } = useAppContext();

  const [overview, setOverview] = useState<UserAccessOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removingIssueKey, setRemovingIssueKey] = useState<string | null>(null);
  const [assignments, setAssignments] = useState<UserAccessProjectPermission[]>([]);
  const [newProjectId, setNewProjectId] = useState<string | null>(null);
  const [newPermission, setNewPermission] = useState<string>('view');

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
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') return;
      setError(err instanceof Error ? err.message : 'Nepodařilo se načíst přístupy.');
    } finally {
      setLoading(false);
    }
  }

	  const availableOptions = useMemo(
    () =>
      overview?.available_projects.map((p) => ({
        value: p.project_id,
        label: p.project_name,
      })) ?? [],
    [overview],
	  );
  const permissionOptions = useMemo(() => getProjectPermissionOptions(t), [t]);

  function addAssignment() {
    if (!newProjectId || !overview) return;
    if (assignments.some((a) => a.project_id === newProjectId)) return;
    const project = overview.available_projects.find((p) => p.project_id === newProjectId);
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

  async function handleSave() {
    setSaving(true);
    try {
      const updated = await updateUserAccess(userId, {
        project_permissions: assignments.map((a) => ({
          project_id: a.project_id,
          permission: a.permission,
        })),
      });
      setOverview(updated);
      setAssignments(updated.project_permissions);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Nepodařilo se uložit přístupy.');
    } finally {
      setSaving(false);
    }
  }

  async function handleRemoveIssuePermission(issueId: string, subjectType: string, subjectId: string) {
    const key = `${issueId}:${subjectType}:${subjectId}`;
    setRemovingIssueKey(key);
    try {
      await removeIssuePermission(issueId, subjectType, subjectId);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Nepodařilo se odebrat přístup k issue.');
    } finally {
      setRemovingIssueKey(null);
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
        <Table verticalSpacing="sm" highlightOnHover>
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
      ) : (
	        <Text c="dimmed" size="sm">{t('access.noUserProjectAssignments')}</Text>
      )}

      <Group justify="flex-end">
        <Button loading={saving} onClick={() => void handleSave()}>
	          {t('access.save')}
        </Button>
      </Group>

      {overview && overview.issue_permissions.length > 0 ? (
        <>
	          <Title order={4}>{t('access.issuePermissionsTitle')}</Title>
	          <Text c="dimmed" size="sm">
	            {t('access.issuePermissionsDescription')}
          </Text>
          <Table verticalSpacing="sm" highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Issue</Table.Th>
	                <Table.Th>{t('access.project')}</Table.Th>
	                <Table.Th>{t('common.permission')}</Table.Th>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {overview.issue_permissions.map((ip) => {
                const key = `${ip.issue_id}:user:${userId}`;
                return (
                <Table.Tr key={ip.issue_id}>
                  <Table.Td>
                    <Text size="sm">{ip.issue_title}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm" c="dimmed">{ip.project_name}</Text>
                  </Table.Td>
                  <Table.Td>
	                    <PermissionSummary info={getIssuePermissionInfo(ip.permission, t)} />
                  </Table.Td>
                  <Table.Td>
                    <Button
                      color="red"
                      variant="subtle"
                      size="xs"
                      loading={removingIssueKey === key}
                      onClick={() => void handleRemoveIssuePermission(ip.issue_id, 'user', userId)}
                    >
	                      {t('common.remove')}
                    </Button>
                  </Table.Td>
                </Table.Tr>
                );
              })}
            </Table.Tbody>
          </Table>
        </>
      ) : null}
    </Stack>
  );
}
