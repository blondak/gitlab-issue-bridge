import { Anchor, Badge, Box, Button, Card, Group, Paper, SimpleGrid, Stack, Text, Title } from '@mantine/core';
import { Link } from 'react-router-dom';

import { PageState } from '../components/PageState';
import { StatsGroup } from '../components/StatsGroup';
import { useAppContext } from '../context/AppContext';

export function OverviewPage() {
	  const { currentUser, overview, dataLoading, issues, projects, t } = useAppContext();

	  const openIssueCount = issues.filter((i) => i.state === 'open').length;
	  const recentIssues = [...issues]
	    .sort((left, right) => {
	      const leftTime = left.last_activity_at ? new Date(left.last_activity_at).getTime() : 0;
	      const rightTime = right.last_activity_at ? new Date(right.last_activity_at).getTime() : 0;
	      return rightTime - leftTime;
	    })
	    .slice(0, 5);

  return (
    <Stack gap="xl">
      <Paper radius="md" p="xl" withBorder shadow="sm">
        <Stack gap="md">
          <Title order={1}>
            {t('overview.greeting')}, {currentUser?.full_name}
          </Title>
          <Text c="dimmed" size="lg">
            {t('overview.subtitle')}
          </Text>
          <Group gap="sm">
            {currentUser?.is_admin ? (
              <Button component={Link} to="/projects">
                {t('overview.openProjects')}
              </Button>
            ) : null}
            <Button component={Link} to="/issues" variant="light">
              {t('overview.openIssues')}
            </Button>
          </Group>
        </Stack>
      </Paper>

      <PageState loading={dataLoading && !overview} />

      {overview ? (
        <>
          <StatsGroup
            data={[
              {
                title: t('overview.myIssues'),
                value: issues.length,
                description: t('overview.myIssuesDesc'),
              },
              {
                title: t('overview.openCount'),
                value: openIssueCount,
                description: t('overview.openCountDesc'),
              },
              {
                title: t('overview.myProjects'),
                value: projects.length,
                description: t('overview.myProjectsDesc'),
              },
            ]}
          />

          <SimpleGrid cols={{ base: 1, md: 2 }}>
            <Card withBorder radius="md" padding="lg">
              <Stack gap="md">
                <Text c="dimmed" size="sm" fw={700} tt="uppercase">
                  {t('overview.recentActivity')}
                </Text>
                {recentIssues.length === 0 ? (
                  <Text c="dimmed" size="sm">
                    {t('overview.noIssues')}
                  </Text>
                ) : (
                  recentIssues.map((issue) => (
                    <Group key={issue.id} justify="space-between" wrap="nowrap" gap="sm">
                      <Box style={{ minWidth: 0, flex: 1 }}>
                        <Anchor
                          component={Link}
                          to={`/issues/${issue.id}`}
                          size="sm"
                          style={{ display: 'block' }}
                          truncate
                        >
                          {issue.title}
                        </Anchor>
                        <Text c="dimmed" size="xs" truncate>
                          {issue.project_name}
                        </Text>
                      </Box>
	                      <Badge
	                        color={issue.state.toLowerCase() === 'closed' ? 'gray' : 'green'}
	                        variant="light"
	                        size="sm"
	                        style={{ flexShrink: 0 }}
	                      >
	                        {issue.state.toLowerCase() === 'closed' ? t('common.closed') : t('common.open')}
	                      </Badge>
                    </Group>
                  ))
                )}
                {issues.length > 5 ? (
                  <Anchor component={Link} to="/issues" size="sm" c="dimmed">
                    {t('overview.allIssuesLink')} ({issues.length})
                  </Anchor>
                ) : null}
              </Stack>
            </Card>

            <Card withBorder radius="md" padding="lg">
              <Stack gap="md">
                <Group justify="space-between">
                  <Box>
                    <Text c="dimmed" size="sm" fw={700} tt="uppercase">
                      {t('overview.currentUser')}
                    </Text>
                    <Title order={3}>{currentUser?.full_name}</Title>
                  </Box>
	                  <Badge color={currentUser?.is_admin ? 'teal' : 'gray'} variant="light">
	                    {currentUser?.is_admin ? t('common.admin') : t('common.member')}
	                  </Badge>
                </Group>
                <Text c="dimmed">{currentUser?.email}</Text>
              </Stack>
            </Card>
          </SimpleGrid>

          {currentUser?.is_admin ? (
            <Card withBorder radius="md" padding="lg">
              <Stack gap="md">
                <Text c="dimmed" size="sm" fw={700} tt="uppercase">
                  {t('overview.systemStats')}
                </Text>
                <StatsGroup
                  data={[
                    {
                      title: t('overview.allProjects'),
                      value: overview.project_count,
                      description: t('overview.allProjectsDesc'),
                    },
                    {
                      title: t('overview.integratedProjects'),
                      value: overview.integrated_project_count,
                      description: t('overview.integratedProjectsDesc'),
                    },
                    {
                      title: t('overview.allIssuesCount'),
                      value: overview.issue_count,
                      description: t('overview.allIssuesCountDesc'),
                    },
                    {
                      title: t('overview.pendingJobs'),
                      value: overview.pending_jobs,
                      description: t('overview.pendingJobsDesc'),
                    },
                  ]}
                />
              </Stack>
            </Card>
          ) : null}
        </>
      ) : null}
    </Stack>
  );
}
