import {
  Badge,
  Button,
  Center,
  Checkbox,
  Drawer,
  Group,
  ScrollArea,
  Stack,
  Switch,
  Table,
  Tabs,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { useEffect, useMemo, useState } from 'react';
import { Navigate } from 'react-router-dom';

import { PageState } from '../components/PageState';
import { UserAccessPanel } from '../components/UserAccessPanel';
import { useAppContext } from '../context/AppContext';
import classes from '../components/TableSort.module.css';
import headerTabsClasses from '../components/HeaderTabs.module.css';
import type {
  CreateInvitationValues,
  ManagedUser,
  UpdateUserValues,
  UserInvitation,
  UserManagementOverview,
} from '../types';

type UserDrafts = Record<string, UpdateUserValues>;

export function UsersPage() {
  const { currentUser, getUserManagementOverview, updateUser, inviteUser, resendInvitation, deleteInvitation, t } = useAppContext();
  const [loading, setLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [overview, setOverview] = useState<UserManagementOverview | null>(null);
  const [savingUserId, setSavingUserId] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<UserDrafts>({});
  const [inviteLoading, setInviteLoading] = useState(false);
  const [invitationActionId, setInvitationActionId] = useState<string | null>(null);
  const [accessUserId, setAccessUserId] = useState<string | null>(null);
  const [accessUserEmail, setAccessUserEmail] = useState<string>('');
  const [search, setSearch] = useState('');
  const [sortBy, setSortBy] = useState<'email' | 'full_name' | 'role' | 'active' | 'created_at' | null>('email');
  const [reverseSortDirection, setReverseSortDirection] = useState(false);
  const [activeTab, setActiveTab] = useState<string | null>('users');

  const inviteForm = useForm<CreateInvitationValues>({
    initialValues: {
      email: '',
      is_admin: false,
    },
    validate: {
      email: (value) => (/.+@.+\..+/.test(value) ? null : 'Zadej validni email.'),
    },
  });

  useEffect(() => {
    void loadOverview();
  }, []);

  async function loadOverview() {
    try {
      setLoading(true);
      setPageError(null);
      const data = await getUserManagementOverview();
      setOverview(data);
      setDrafts(buildDrafts(data.users));
    } catch (error) {
      setPageError(error instanceof Error ? error.message : t('users.loadError'));
    } finally {
      setLoading(false);
    }
  }

  async function handleUserSave(user: ManagedUser) {
    const draft = drafts[user.id];
    if (!draft) return;

    setSavingUserId(user.id);
    try {
      await updateUser(user.id, draft);
      await loadOverview();
    } catch (error) {
      setPageError(error instanceof Error ? error.message : t('users.updateError'));
    } finally {
      setSavingUserId(null);
    }
  }

  async function handleInvite(values: CreateInvitationValues) {
    setInviteLoading(true);
    try {
      await inviteUser(values);
      inviteForm.reset();
      await loadOverview();
    } catch (error) {
      setPageError(error instanceof Error ? error.message : t('users.inviteError'));
    } finally {
      setInviteLoading(false);
    }
  }

  async function handleResendInvitation(invitation: UserInvitation) {
    setInvitationActionId(invitation.id);
    try {
      await resendInvitation(invitation.id);
      await loadOverview();
    } catch (error) {
      setPageError(error instanceof Error ? error.message : t('users.resendError'));
    } finally {
      setInvitationActionId(null);
    }
  }

  async function handleDeleteInvitation(invitation: UserInvitation) {
    if (!window.confirm(`${t('users.deleteConfirm')} ${invitation.email}?`)) return;

    setInvitationActionId(invitation.id);
    try {
      await deleteInvitation(invitation.id);
      await loadOverview();
    } catch (error) {
      setPageError(error instanceof Error ? error.message : t('users.deleteError'));
    } finally {
      setInvitationActionId(null);
    }
  }

  const sortedUsers = useMemo(() => {
    if (!overview) {
      return [];
    }

    const query = search.trim().toLowerCase();
    const filtered = overview.users.filter((user) => {
      const role = user.is_admin ? 'admin' : 'member';
      const status = user.active ? 'active' : 'inactive';
      return [user.email, user.full_name, role, status].some((value) =>
        value.toLowerCase().includes(query),
      );
    });

    if (!sortBy) {
      return filtered;
    }

    const sorted = [...filtered].sort((a, b) => {
      const aValue =
        sortBy === 'role'
          ? a.is_admin
            ? 'admin'
            : 'member'
          : sortBy === 'active'
            ? a.active
              ? 'active'
              : 'inactive'
            : a[sortBy];
      const bValue =
        sortBy === 'role'
          ? b.is_admin
            ? 'admin'
            : 'member'
          : sortBy === 'active'
            ? b.active
              ? 'active'
              : 'inactive'
            : b[sortBy];

      if (sortBy === 'created_at') {
        return new Date(String(aValue)).getTime() - new Date(String(bValue)).getTime();
      }

      return String(aValue).localeCompare(String(bValue));
    });

    return reverseSortDirection ? sorted.reverse() : sorted;
  }, [overview, reverseSortDirection, search, sortBy]);

  function setSorting(field: 'email' | 'full_name' | 'role' | 'active' | 'created_at') {
    const reversed = field === sortBy ? !reverseSortDirection : false;
    setReverseSortDirection(reversed);
    setSortBy(field);
  }

  if (!currentUser?.is_admin) {
    return <Navigate to="/overview" replace />;
  }

  return (
    <Stack gap="lg">
      <div className={headerTabsClasses.header}>
        <div className={headerTabsClasses.mainSection}>
          <Stack gap={4}>
            <Title order={2}>{t('users.title')}</Title>
            <Text c="dimmed">
              {t('users.subtitle')}
            </Text>
          </Stack>
        </div>
        <Tabs
          value={activeTab}
          onChange={setActiveTab}
          variant="outline"
          classNames={{
            root: headerTabsClasses.tabs,
            list: headerTabsClasses.tabsList,
            tab: headerTabsClasses.tab,
          }}
        >
          <Tabs.List>
            <Tabs.Tab value="users">{t('users.tabUsers')}</Tabs.Tab>
            <Tabs.Tab value="pending">{t('users.tabPending')}</Tabs.Tab>
            <Tabs.Tab value="invite">{t('users.tabInvite')}</Tabs.Tab>
          </Tabs.List>
        </Tabs>
      </div>

      <PageState loading={loading} error={pageError} />

      {overview ? (
        <>
          {activeTab === 'users' ? (
            <Stack gap="md">
              <Group justify="space-between">
                <Title order={3}>{t('users.adminTitle')}</Title>
                <Badge color="blue" variant="light">
                  {overview.users.length} {t('common.users')}
                </Badge>
              </Group>

              <TextInput
                placeholder={t('users.search')}
                value={search}
                onChange={(event) => setSearch(event.currentTarget.value)}
              />

              <ScrollArea>
                <Table verticalSpacing="md" highlightOnHover miw={820}>
                  <Table.Thead>
                    <Table.Tr>
                      <SortableTh
                        sorted={sortBy === 'email'}
                        reversed={reverseSortDirection}
                        onSort={() => setSorting('email')}
                      >
                        {t('common.email')}
                      </SortableTh>
                      <SortableTh
                        sorted={sortBy === 'full_name'}
                        reversed={reverseSortDirection}
                        onSort={() => setSorting('full_name')}
                      >
                        {t('common.fullName')}
                      </SortableTh>
                      <Table.Th>{t('common.admin')}</Table.Th>
                      <SortableTh
                        sorted={sortBy === 'role'}
                        reversed={reverseSortDirection}
                        onSort={() => setSorting('role')}
                      >
                        {t('common.role')}
                      </SortableTh>
                      <SortableTh
                        sorted={sortBy === 'active'}
                        reversed={reverseSortDirection}
                        onSort={() => setSorting('active')}
                      >
                        {t('common.active')}
                      </SortableTh>
                      <SortableTh
                        sorted={sortBy === 'created_at'}
                        reversed={reverseSortDirection}
                        onSort={() => setSorting('created_at')}
                      >
                        {t('common.created')}
                      </SortableTh>
                      <Table.Th />
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {sortedUsers.length > 0 ? sortedUsers.map((user) => {
                      const draft = drafts[user.id];
                      const isCurrentAdmin = currentUser.id === user.id;
                      const adminSwitchDisabled = isCurrentAdmin && (draft?.is_admin ?? user.is_admin);
                      const activeSwitchDisabled = isCurrentAdmin && (draft?.active ?? user.active);

                      return (
                        <Table.Tr key={user.id}>
                          <Table.Td>
                            <Stack gap={2}>
                              <Text fw={600}>{user.email}</Text>
                              <Text size="xs" c="dimmed">
                                {t('common.created')} {formatDate(user.created_at)}
                              </Text>
                            </Stack>
                          </Table.Td>
                          <Table.Td>
                            <TextInput
                              value={draft?.full_name ?? ''}
                              onChange={(event) => {
                                const fullName = event.currentTarget.value;
                                setDrafts((current) => ({
                                  ...current,
                                  [user.id]: {
                                    ...(current[user.id] ?? userToDraft(user)),
                                    full_name: fullName,
                                  },
                                }));
                              }}
                            />
                          </Table.Td>
                          <Table.Td>
                            <Switch
                              checked={draft?.is_admin ?? false}
                              disabled={adminSwitchDisabled}
                              onChange={(event) => {
                                const isAdmin = event.currentTarget.checked;
                                setDrafts((current) => ({
                                  ...current,
                                  [user.id]: {
                                    ...(current[user.id] ?? userToDraft(user)),
                                    is_admin: isAdmin,
                                  },
                                }));
                              }}
                            />
                          </Table.Td>
                          <Table.Td>
                            <Badge color={draft?.is_admin ? 'teal' : 'gray'} variant="light">
                              {draft?.is_admin ? t('common.admin') : t('common.member')}
                            </Badge>
                          </Table.Td>
                          <Table.Td>
                            <Switch
                              checked={draft?.active ?? false}
                              disabled={activeSwitchDisabled}
                              onChange={(event) => {
                                const active = event.currentTarget.checked;
                                setDrafts((current) => ({
                                  ...current,
                                  [user.id]: {
                                    ...(current[user.id] ?? userToDraft(user)),
                                    active,
                                  },
                                }));
                              }}
                            />
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm" c="dimmed">
                              {formatDate(user.created_at)}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Stack gap={4} align="flex-end">
                              <Button
                                size="xs"
                                loading={savingUserId === user.id}
                                onClick={() => void handleUserSave(user)}
                              >
                                {t('users.saveUser')}
                              </Button>
                              <Button
                                size="xs"
                                variant="light"
                                onClick={() => {
                                  setAccessUserId(user.id);
                                  setAccessUserEmail(user.email);
                                }}
                              >
                                {t('users.access')}
                              </Button>
                              {isCurrentAdmin ? (
                                <Text size="xs" c="dimmed" ta="right">
                                  {t('users.selfAdminWarning')}
                                </Text>
                              ) : null}
                            </Stack>
                          </Table.Td>
                        </Table.Tr>
                      );
                    }) : (
                      <Table.Tr>
                        <Table.Td colSpan={7}>
                          <Text fw={500} ta="center">
                            {t('users.emptyFilteredMessage')}
                          </Text>
                        </Table.Td>
                      </Table.Tr>
                    )}
                  </Table.Tbody>
                </Table>
              </ScrollArea>
            </Stack>
          ) : null}

          {activeTab === 'invite' ? (
            <Stack gap="md">
              <Title order={3}>{t('users.inviteTitle')}</Title>
              <Text c="dimmed" size="sm">
                {t('users.inviteHelp')}
              </Text>
              <form onSubmit={inviteForm.onSubmit((values) => void handleInvite(values))}>
                <Stack gap="md">
                  <TextInput
                    label={t('common.email')}
                    placeholder="new.user@example.com"
                    required
                    {...inviteForm.getInputProps('email')}
                  />
                  <Checkbox
                    label={t('users.inviteAsAdmin')}
                    {...inviteForm.getInputProps('is_admin', { type: 'checkbox' })}
                  />
                  <Button type="submit" loading={inviteLoading}>
                    {t('users.sendInvitation')}
                  </Button>
                </Stack>
              </form>
            </Stack>
          ) : null}

          {activeTab === 'pending' ? (
            <Stack gap="md">
              <Group justify="space-between">
                <Title order={3}>{t('users.pendingTitle')}</Title>
                <Badge color="blue" variant="light">
                  {overview.invitations.length} {t('common.invitations')}
                </Badge>
              </Group>
              {overview.invitations.length > 0 ? (
                <ScrollArea>
                  <Table verticalSpacing="md" highlightOnHover miw={720}>
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>{t('common.email')}</Table.Th>
                        <Table.Th>{t('common.role')}</Table.Th>
                        <Table.Th>{t('common.status')}</Table.Th>
                        <Table.Th>{t('users.lastSent')}</Table.Th>
                        <Table.Th>{t('users.expires')}</Table.Th>
                        <Table.Th />
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {overview.invitations.map((invitation) => (
                        <Table.Tr key={invitation.id}>
                          <Table.Td>
                            <Text fw={600}>{invitation.email}</Text>
                          </Table.Td>
                          <Table.Td>
                            <Badge color={invitation.is_admin ? 'teal' : 'gray'} variant="light">
                              {invitation.is_admin ? t('common.admin') : t('common.member')}
                            </Badge>
                          </Table.Td>
                          <Table.Td>
                            <Badge color="blue" variant="light">
                              {invitation.status}
                            </Badge>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm" c="dimmed">
                              {formatDate(invitation.last_sent_at)}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm" c="dimmed">
                              {formatDate(invitation.expires_at)}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Group gap="xs" justify="flex-end" wrap="nowrap">
                              <Button
                                size="xs"
                                variant="light"
                                loading={invitationActionId === invitation.id}
                                onClick={() => void handleResendInvitation(invitation)}
                              >
                                {t('users.resend')}
                              </Button>
                              <Button
                                size="xs"
                                color="red"
                                variant="subtle"
                                loading={invitationActionId === invitation.id}
                                onClick={() => void handleDeleteInvitation(invitation)}
                              >
                                {t('users.deleteInvitation')}
                              </Button>
                            </Group>
                          </Table.Td>
                        </Table.Tr>
                      ))}
                    </Table.Tbody>
                  </Table>
                </ScrollArea>
              ) : (
                <Text c="dimmed" size="sm">
                  {t('users.noInvitations')}
                </Text>
              )}
            </Stack>
          ) : null}

          {activeTab === 'users' ? null : null}
        </>
      ) : null}

      <Drawer
        opened={accessUserId !== null}
        onClose={() => setAccessUserId(null)}
        title={`${t('users.drawerTitle')}: ${accessUserEmail}`}
        position="right"
        size="lg"
        padding="md"
      >
        {accessUserId ? <UserAccessPanel userId={accessUserId} /> : null}
      </Drawer>
    </Stack>
  );
}

function buildDrafts(users: ManagedUser[]): UserDrafts {
  return Object.fromEntries(users.map((user) => [user.id, userToDraft(user)]));
}

function userToDraft(user: ManagedUser): UpdateUserValues {
  return {
    full_name: user.full_name,
    is_admin: user.is_admin,
    active: user.active,
  };
}

function formatDate(value: string) {
  return new Date(value).toLocaleString('cs-CZ');
}

type SortableThProps = {
  children: React.ReactNode;
  reversed: boolean;
  sorted: boolean;
  onSort: () => void;
};

function SortableTh({ children, reversed, sorted, onSort }: SortableThProps) {
  const icon = sorted ? (reversed ? '↑' : '↓') : '↕';
  return (
    <Table.Th className={classes.th}>
      <UnstyledButton onClick={onSort} className={classes.control}>
        <Group justify="space-between">
          <Text fw={500} fz="sm">
            {children}
          </Text>
          <Center className={classes.icon}>
            <Text size="sm">{icon}</Text>
          </Center>
        </Group>
      </UnstyledButton>
    </Table.Th>
  );
}
