import {
  Badge,
  Button,
  Center,
  Group,
  ScrollArea,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from '@mantine/core';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { CreateIssueModal } from '../components/CreateIssueModal';
import { PageState } from '../components/PageState';
import classes from '../components/TableSort.module.css';
import { useAppContext } from '../context/AppContext';
import { getCreatableProjects } from '../lib/capability-ui';
import type { CreateIssueValues } from '../types';

export function IssuesPage() {
  const navigate = useNavigate();
  const {
    projects,
    issues,
    dataLoading,
    createIssue,
    t,
  } = useAppContext();
  const stateFilterOptions = [
    { value: 'open', label: t('issues.state.open') },
    { value: 'closed', label: t('issues.state.closed') },
    { value: 'all', label: t('issues.state.all') },
  ];
  const [projectFilter, setProjectFilter] = useState<string | null>(null);
  const [stateFilter, setStateFilter] = useState<string>('open');
  const [search, setSearch] = useState('');
  const [sortBy, setSortBy] = useState<'title' | 'project_name' | 'state' | 'sync_state' | 'gitlab_issue_iid' | 'last_activity_at' | null>('last_activity_at');
  const [reverseSortDirection, setReverseSortDirection] = useState(true);
  const [createModalOpened, setCreateModalOpened] = useState(false);
  const [createLoading, setCreateLoading] = useState(false);
  const creatableProjects = useMemo(
    () => getCreatableProjects(projects),
    [projects],
  );

  function normalizeIssueState(state: string) {
    const normalized = state.trim().toLowerCase();
    if (normalized === 'opened') {
      return 'open';
    }

    return normalized;
  }

	  function formatLastActivity(value?: string | null) {
	    if (!value) {
	      return '—';
	    }

	    const date = new Date(value);
	    if (Number.isNaN(date.getTime())) {
      return '—';
    }

    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
	    }).format(date);
	  }

	  function formatIssueStateLabel(state: string) {
	    return normalizeIssueState(state) === 'closed' ? t('common.closed') : t('common.open');
	  }

	  function formatIssueNumber(issue: { gitlab_issue_iid: number }) {
	    return issue.gitlab_issue_iid > 0 ? `GitLab #${issue.gitlab_issue_iid}` : t('issues.localNumber');
	  }

  const filteredIssues = useMemo(() => {
    const query = search.trim().toLowerCase();
    const filtered = issues.filter((issue) => {
      const matchesProject = projectFilter ? issue.project_id === projectFilter : true;
      const normalizedState = normalizeIssueState(issue.state);
      const matchesState = stateFilter === 'all' ? true : normalizedState === stateFilter.toLowerCase();
      const matchesSearch =
        !query ||
        [
          issue.title,
          issue.project_name,
          normalizedState,
	          issue.state,
	          issue.sync_state,
	          formatIssueNumber(issue),
	          formatLastActivity(issue.last_activity_at),
	        ]
          .some((value) => value.toLowerCase().includes(query));

      return matchesProject && matchesState && matchesSearch;
    });

    if (!sortBy) {
      return filtered;
    }

	    const sorted = [...filtered].sort((a, b) => {
	      if (sortBy === 'gitlab_issue_iid') {
	        const left = a.gitlab_issue_iid > 0 ? a.gitlab_issue_iid : Number.MAX_SAFE_INTEGER;
	        const right = b.gitlab_issue_iid > 0 ? b.gitlab_issue_iid : Number.MAX_SAFE_INTEGER;
	        return left - right;
	      }
      if (sortBy === 'last_activity_at') {
        const left = a.last_activity_at ? new Date(a.last_activity_at).getTime() : 0;
        const right = b.last_activity_at ? new Date(b.last_activity_at).getTime() : 0;
        return left - right;
      }

      return String(a[sortBy]).localeCompare(String(b[sortBy]));
    });

    return reverseSortDirection ? sorted.reverse() : sorted;
	  }, [issues, projectFilter, reverseSortDirection, search, sortBy, stateFilter, t]);

  function setSorting(field: 'title' | 'project_name' | 'state' | 'sync_state' | 'gitlab_issue_iid' | 'last_activity_at') {
    const reversed = field === sortBy ? !reverseSortDirection : false;
    setReverseSortDirection(reversed);
    setSortBy(field);
  }

  async function handleCreateIssue(values: CreateIssueValues) {
    setCreateLoading(true);
    try {
      const issue = await createIssue(values);
      navigate(`/issues/${issue.id}`);
    } finally {
      setCreateLoading(false);
    }
  }

  return (
    <Stack gap="lg">
      <Group justify="space-between" align="flex-end">
        <Stack gap={4}>
          <Title order={2}>{t('issues.title')}</Title>
          <Text c="dimmed">
            {t('issues.subtitle')}
          </Text>
        </Stack>
        <Group>
          <Select
            placeholder={t('issues.filterProject')}
            clearable
            value={projectFilter}
            onChange={setProjectFilter}
            data={projects.map((project) => ({ value: project.id, label: project.name }))}
            w={280}
          />
          <Select
            value={stateFilter}
            onChange={(value) => setStateFilter(value ?? 'open')}
            data={stateFilterOptions}
            w={180}
          />
          {creatableProjects.length > 0 ? (
            <Button onClick={() => setCreateModalOpened(true)}>{t('issues.create')}</Button>
          ) : null}
        </Group>
      </Group>

      <PageState loading={dataLoading && issues.length === 0} />

      <Stack gap="md">
	        <Title order={3}>{t('issues.listTitle')}</Title>
	        <TextInput
	          placeholder={t('issues.search')}
	          value={search}
	          onChange={(event) => setSearch(event.currentTarget.value)}
	        />
        <ScrollArea>
          <Table verticalSpacing="md" highlightOnHover miw={920}>
            <Table.Thead>
              <Table.Tr>
                <SortableTh
                  sorted={sortBy === 'title'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('title')}
                >
	                  {t('issues.table.title')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'project_name'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('project_name')}
                >
	                  {t('issues.table.project')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'gitlab_issue_iid'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('gitlab_issue_iid')}
                >
	                  {t('issues.table.number')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'state'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('state')}
                >
	                  {t('issues.table.state')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'last_activity_at'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('last_activity_at')}
                >
	                  {t('issues.table.lastActivity')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'sync_state'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('sync_state')}
                >
	                  {t('issues.table.sync')}
                </SortableTh>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {filteredIssues.length > 0 ? (
                filteredIssues.map((issue) => (
                  <Table.Tr
                    key={issue.id}
                    style={{ cursor: 'pointer' }}
                    onClick={() => navigate(`/issues/${issue.id}`)}
                  >
                    <Table.Td>
                      <Text fw={600}>{issue.title}</Text>
                    </Table.Td>
	                    <Table.Td>{issue.project_name}</Table.Td>
	                    <Table.Td>
	                      <Badge variant="light" color={issue.gitlab_issue_iid > 0 ? 'blue' : 'gray'}>
	                        {formatIssueNumber(issue)}
	                      </Badge>
	                    </Table.Td>
	                    <Table.Td>
	                      <Badge color={normalizeIssueState(issue.state) === 'open' ? 'teal' : 'gray'} variant="light">
	                        {formatIssueStateLabel(issue.state)}
	                      </Badge>
                    </Table.Td>
                    <Table.Td>
                      <Text size="sm">{formatLastActivity(issue.last_activity_at)}</Text>
                    </Table.Td>
                    <Table.Td>
                      <Badge color="blue" variant="light">
                        {issue.sync_state}
                      </Badge>
                    </Table.Td>
                  </Table.Tr>
                ))
              ) : (
	                <Table.Tr>
	                  <Table.Td colSpan={6}>
	                    <Stack gap={4} align="center" py="xl">
	                      <Text fw={600} ta="center">
	                        {issues.length === 0 ? t('issues.emptyTitle') : t('issues.emptyFilteredTitle')}
	                      </Text>
	                      <Text size="sm" c="dimmed" ta="center">
	                        {issues.length === 0 ? t('issues.emptyMessage') : t('issues.emptyFilteredMessage')}
	                      </Text>
	                      {issues.length === 0 && creatableProjects.length > 0 ? (
	                        <Button size="xs" variant="light" onClick={() => setCreateModalOpened(true)}>
	                          {t('issues.createFirst')}
	                        </Button>
	                      ) : null}
	                    </Stack>
	                  </Table.Td>
	                </Table.Tr>
              )}
            </Table.Tbody>
          </Table>
        </ScrollArea>
      </Stack>

      <CreateIssueModal
        opened={createModalOpened}
        onClose={() => setCreateModalOpened(false)}
        projects={creatableProjects}
        initialProjectId={projectFilter}
        loading={createLoading}
        onSubmit={handleCreateIssue}
      />
    </Stack>
  );
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
