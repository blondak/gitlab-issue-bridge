import { Button, Divider, Group, Select, Stack, Table, Text, Title } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';

import { useAppContext } from '../context/AppContext';
import {
  getIssuePermissionInfo,
  getProjectPermissionInfo,
  getProjectPermissionOptions,
  PermissionSummary,
} from '../lib/permission-ui';
import type {
  ProjectAccessAssignment,
  ProjectAccessOverview,
  ProjectAccessSubjectOption,
  UpdateProjectAccessValues,
} from '../types';

type ProjectAccessPanelProps = {
  accessOverview: ProjectAccessOverview | null;
  loading?: boolean;
  saving?: boolean;
  onSave: (values: UpdateProjectAccessValues) => Promise<void> | void;
  onIssuePermissionRemoved?: () => void;
};

export function ProjectAccessPanel({ accessOverview, loading, saving, onSave, onIssuePermissionRemoved }: ProjectAccessPanelProps) {
  const { removeIssuePermission, t } = useAppContext();
  const [assignments, setAssignments] = useState<ProjectAccessAssignment[]>(accessOverview?.assignments ?? []);
  const [newSubjectKey, setNewSubjectKey] = useState<string | null>(null);
  const [newPermission, setNewPermission] = useState<string>('view');
  const [removingIssueKey, setRemovingIssueKey] = useState<string | null>(null);

  useEffect(() => {
    setAssignments(accessOverview?.assignments ?? []);
  }, [accessOverview?.assignments]);

  const availableOptions = useMemo(
    () =>
      accessOverview?.available_subjects.map((subject) => ({
        value: `${subject.subject_type}:${subject.subject_id}`,
        label: `${subject.display_name} (${subject.email})`,
      })) ?? [],
    [accessOverview],
  );
  const permissionOptions = useMemo(() => getProjectPermissionOptions(t), [t]);

  if (loading) {
    return <Text c="dimmed">{t('access.loadingProject')}</Text>;
  }

  if (!accessOverview) {
    return <Text c="dimmed">{t('access.projectUnavailable')}</Text>;
  }

  const allSubjects: ProjectAccessSubjectOption[] = [
    ...accessOverview.available_subjects,
    ...assignments.map((assignment) => ({
      subject_type: assignment.subject_type,
      subject_id: assignment.subject_id,
      display_name: assignment.display_name,
      email: assignment.email,
    })),
  ];

  function addAssignment() {
    if (!newSubjectKey) return;
    const [subject_type, ...rest] = newSubjectKey.split(':');
    const subject_id = rest.join(':');
    if (!subject_type || !subject_id) return;
    if (assignments.some((assignment) => assignment.subject_type === subject_type && assignment.subject_id === subject_id)) {
      return;
    }
    const subject = allSubjects.find((item) => item.subject_type === subject_type && item.subject_id === subject_id);
    if (!subject) return;

    setAssignments((current) => [
      ...current,
      {
        subject_type: subject.subject_type,
        subject_id: subject.subject_id,
        display_name: subject.display_name,
        email: subject.email,
        permission: newPermission,
      },
    ]);
    setNewSubjectKey(null);
    setNewPermission('view');
  }

  function updateAssignment(subject_type: string, subject_id: string, permission: string) {
    setAssignments((current) =>
      current.map((assignment) =>
        assignment.subject_type === subject_type && assignment.subject_id === subject_id
          ? { ...assignment, permission }
          : assignment,
      ),
    );
  }

  function removeAssignment(subject_type: string, subject_id: string) {
    setAssignments((current) =>
      current.filter((assignment) => !(assignment.subject_type === subject_type && assignment.subject_id === subject_id)),
    );
  }

  async function handleRemoveIssuePermission(issueId: string, subjectType: string, subjectId: string) {
    const key = `${issueId}:${subjectType}:${subjectId}`;
    setRemovingIssueKey(key);
    try {
      await removeIssuePermission(issueId, subjectType, subjectId);
      onIssuePermissionRemoved?.();
    } finally {
      setRemovingIssueKey(null);
    }
  }

  return (
    <Stack gap="md">
      <Title order={3}>{t('access.projectTitle')}</Title>
      <Text c="dimmed" size="sm">
        {t('access.projectDescription')}
      </Text>
      <Text c="dimmed" size="sm">
        {t('access.issueSpecificHelp')}
      </Text>

      <Group align="flex-end">
        <Select
          label={t('access.subject')}
          placeholder={t('access.subjectPlaceholder')}
          data={availableOptions}
          value={newSubjectKey}
          onChange={setNewSubjectKey}
          searchable
          flex={1}
        />
        <Select
          label={t('common.permission')}
          data={permissionOptions}
          value={newPermission}
          onChange={(value) => setNewPermission(value ?? 'view')}
          w={180}
        />
        <Button variant="light" onClick={addAssignment} disabled={!newSubjectKey}>
          {t('access.add')}
        </Button>
      </Group>

      {assignments.length > 0 ? (
        <Table verticalSpacing="sm" highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>{t('common.user')}</Table.Th>
              <Table.Th>{t('common.type')}</Table.Th>
              <Table.Th>{t('common.permission')}</Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {assignments.map((assignment) => (
              <Table.Tr key={`${assignment.subject_type}:${assignment.subject_id}`}>
                <Table.Td>
                  <Stack gap={2}>
                    <Text fw={600}>{assignment.display_name}</Text>
                    <Text size="xs" c="dimmed">
                      {assignment.email}
                    </Text>
                  </Stack>
                </Table.Td>
                <Table.Td>{assignment.subject_type === 'user' ? t('access.activeUser') : t('access.invitedUser')}</Table.Td>
                <Table.Td>
                  <Stack gap="xs">
                    <Select
                      data={permissionOptions}
                      value={assignment.permission}
                      onChange={(value) =>
                        updateAssignment(assignment.subject_type, assignment.subject_id, value ?? 'view')
                      }
                      w={260}
                    />
                    <PermissionSummary info={getProjectPermissionInfo(assignment.permission, t)} />
                  </Stack>
                </Table.Td>
                <Table.Td>
                  <Button
                    color="red"
                    variant="subtle"
                    onClick={() => removeAssignment(assignment.subject_type, assignment.subject_id)}
                  >
                    {t('common.remove')}
                  </Button>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      ) : (
        <Text c="dimmed">{t('access.noProjectAssignments')}</Text>
      )}

      <Group justify="flex-end">
        <Button
          loading={saving}
          onClick={() =>
            void onSave({
              assignments: assignments.map((assignment) => ({
                subject_type: assignment.subject_type,
                subject_id: assignment.subject_id,
                permission: assignment.permission,
              })),
            })
          }
        >
          {t('access.save')}
        </Button>
      </Group>

      {accessOverview.issue_permissions.length > 0 ? (
        <>
          <Divider />
          <Title order={4}>{t('access.issuePermissionsTitle')}</Title>
          <Text c="dimmed" size="sm">
            {t('access.issuePermissionsDescription')}
          </Text>
          <Table verticalSpacing="sm" highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Issue</Table.Th>
                <Table.Th>{t('common.user')}</Table.Th>
                <Table.Th>{t('common.permission')}</Table.Th>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {accessOverview.issue_permissions.map((ip) => {
                const key = `${ip.issue_id}:${ip.subject_type}:${ip.subject_id}`;
                return (
                  <Table.Tr key={key}>
                    <Table.Td>
                      <Text size="sm" fw={500}>{ip.issue_title}</Text>
                    </Table.Td>
                    <Table.Td>
                      <Stack gap={2}>
                        <Text size="sm">{ip.subject_display_name}</Text>
                        <Text size="xs" c="dimmed">{ip.subject_email}</Text>
                      </Stack>
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
                        onClick={() => void handleRemoveIssuePermission(ip.issue_id, ip.subject_type, ip.subject_id)}
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
