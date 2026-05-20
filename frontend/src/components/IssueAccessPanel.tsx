import { Button, Group, Select, Stack, Table, Text, Title } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';

import { useAppContext } from '../context/AppContext';
import { getIssuePermissionInfo, getIssuePermissionOptions, PermissionSummary } from '../lib/permission-ui';
import type {
  IssueAccessAssignment,
  IssueAccessOverview,
  IssueAccessUserOption,
  UpdateIssueAccessValues,
} from '../types';

type IssueAccessPanelProps = {
  accessOverview: IssueAccessOverview | null;
  loading?: boolean;
  saving?: boolean;
  onSave: (values: UpdateIssueAccessValues) => Promise<void> | void;
};

export function IssueAccessPanel({
  accessOverview,
  loading,
  saving,
	onSave,
}: IssueAccessPanelProps) {
  const { t } = useAppContext();
  const [assignments, setAssignments] = useState<IssueAccessAssignment[]>(accessOverview?.assignments ?? []);
  const [newUserId, setNewUserId] = useState<string | null>(null);
  const [newPermission, setNewPermission] = useState<string>('comment');

  useEffect(() => {
    setAssignments(accessOverview?.assignments ?? []);
  }, [accessOverview?.assignments]);

	  const availableOptions = useMemo(
    () =>
      accessOverview?.available_users.map((user) => ({
        value: user.id,
        label: `${user.full_name} (${user.email})`,
      })) ?? [],
    [accessOverview],
	  );
  const permissionOptions = useMemo(() => getIssuePermissionOptions(t), [t]);

	  if (loading) {
	    return (
	      <div>
	        <Text c="dimmed">{t('access.loadingIssue')}</Text>
	      </div>
	    );
	  }

  if (!accessOverview) {
	    return (
	      <div>
	        <Text c="dimmed">{t('access.issueUnavailable')}</Text>
	      </div>
	    );
	  }

  const allAvailableUsers: IssueAccessUserOption[] = [
    ...accessOverview.available_users,
    ...assignments.map((assignment) => ({
      id: assignment.user_id,
      email: assignment.email,
      full_name: assignment.full_name,
    })),
  ];

  function updateAssignment(userId: string, permission: string) {
    setAssignments((current) =>
      current.map((assignment) =>
        assignment.user_id === userId ? { ...assignment, permission } : assignment,
      ),
    );
  }

  function removeAssignment(userId: string) {
    setAssignments((current) => current.filter((assignment) => assignment.user_id !== userId));
  }

  function addAssignment() {
    if (!newUserId || assignments.some((assignment) => assignment.user_id === newUserId)) {
      return;
    }

    const user = allAvailableUsers.find((option) => option.id === newUserId);
    if (!user) {
      return;
    }

    setAssignments((current) => [
      ...current,
      {
        user_id: user.id,
        email: user.email,
        full_name: user.full_name,
        permission: newPermission,
      },
    ]);
    setNewUserId(null);
    setNewPermission('comment');
  }

  return (
    <Stack gap="md">
	      <Title order={3}>{t('access.issueTitle')}</Title>
	      <Text c="dimmed" size="sm">
	        {t('access.issueDescription')}
	      </Text>

      <Group align="flex-end">
	        <Select
	          label={t('access.user')}
	          placeholder={t('access.userPlaceholder')}
          data={availableOptions}
          value={newUserId}
          onChange={setNewUserId}
          searchable
          flex={1}
        />
	        <Select
	          label={t('common.permission')}
          data={permissionOptions}
          value={newPermission}
          onChange={(value) => setNewPermission(value ?? 'comment')}
          w={160}
        />
	        <Button variant="light" onClick={addAssignment} disabled={!newUserId}>
	          {t('access.add')}
        </Button>
      </Group>

      {assignments.length > 0 ? (
        <Table verticalSpacing="sm" highlightOnHover>
          <Table.Thead>
            <Table.Tr>
	              <Table.Th>{t('common.user')}</Table.Th>
	              <Table.Th>{t('common.permission')}</Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {assignments.map((assignment) => (
              <Table.Tr key={assignment.user_id}>
                <Table.Td>
                  <Stack gap={2}>
                    <Text fw={600}>{assignment.full_name}</Text>
                    <Text size="xs" c="dimmed">
                      {assignment.email}
                    </Text>
                  </Stack>
                </Table.Td>
                <Table.Td>
	                  <Stack gap="xs">
	                    <Select
	                      data={permissionOptions}
	                      value={assignment.permission}
	                      onChange={(value) => updateAssignment(assignment.user_id, value ?? 'read')}
	                      w={260}
	                    />
	                    <PermissionSummary info={getIssuePermissionInfo(assignment.permission, t)} />
	                  </Stack>
                </Table.Td>
                <Table.Td>
                  <Button
                    color="red"
                    variant="subtle"
                    onClick={() => removeAssignment(assignment.user_id)}
                  >
	                    {t('common.remove')}
                  </Button>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      ) : (
	        <Text c="dimmed">{t('access.noIssueAssignments')}</Text>
      )}

      <Group justify="flex-end">
        <Button
          loading={saving}
          onClick={() =>
            void onSave({
              assignments: assignments.map((assignment) => ({
                user_id: assignment.user_id,
                permission: assignment.permission,
              })),
            })
          }
        >
	          {t('access.save')}
        </Button>
      </Group>
    </Stack>
  );
}
