import {
  Alert,
  Badge,
  Button,
  Card,
  Group,
  Paper,
  ScrollArea,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Title,
  Tooltip,
} from '@mantine/core';
import {
  IconActivityHeartbeat,
  IconAlertTriangle,
  IconDatabase,
  IconRefresh,
  IconServerCog,
} from '@tabler/icons-react';
import type { ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import { Navigate } from 'react-router-dom';

import { PageState } from '../components/PageState';
import { useAppContext } from '../context/AppContext';
import type { AdminHealth, HealthCheck, RecentJobFailure, WorkerHeartbeat } from '../types';

export function HealthPage() {
  const { currentUser, getAdminHealth, t } = useAppContext();
  const [health, setHealth] = useState<AdminHealth | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [pageError, setPageError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    void loadHealth(controller.signal);
    return () => controller.abort();
  }, []);

  async function loadHealth(signal?: AbortSignal) {
    try {
      if (health) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      setPageError(null);
      setHealth(await getAdminHealth(signal));
    } catch (error) {
      if (!(error instanceof DOMException && error.name === 'AbortError')) {
        setPageError(error instanceof Error ? error.message : 'Nepodarilo se nacist health data.');
      }
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }

  const healthyWorkers = useMemo(
    () => health?.workers.filter((worker) => worker.healthy).length ?? 0,
    [health],
  );

  if (!currentUser?.is_admin) {
    return <Navigate to="/overview" replace />;
  }

  return (
    <Stack gap="lg">
      <Paper radius="md" p="xl" withBorder shadow="sm">
        <Group justify="space-between" align="flex-start" gap="md">
          <Stack gap={4}>
            <Title order={2}>{t('health.title')}</Title>
            <Text c="dimmed">{t('health.subtitle')}</Text>
          </Stack>
          <Button
            leftSection={<IconRefresh size={16} />}
            variant="light"
            loading={refreshing}
            onClick={() => void loadHealth()}
          >
            Refresh
          </Button>
        </Group>
      </Paper>

      <PageState loading={loading} error={pageError} />

      {health ? (
        <Stack gap="lg">
          <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
            <StatusCard
              label="System"
              value={health.status}
              description={`Generated ${formatDateTime(health.generated_at)}`}
              ok={health.status === 'ok'}
              icon={<IconServerCog size={22} />}
            />
            <StatusCard
              label="Database"
              value={checkStatus(health.checks, 'db')}
              description={checkMessage(health.checks, 'db') ?? 'PostgreSQL check'}
              ok={checkStatus(health.checks, 'db') === 'ok'}
              icon={<IconDatabase size={22} />}
            />
            <StatusCard
              label="Queue"
              value={checkStatus(health.checks, 'queue')}
              description={checkMessage(health.checks, 'queue') ?? 'Queue check'}
              ok={checkStatus(health.checks, 'queue') === 'ok'}
              icon={<IconActivityHeartbeat size={22} />}
            />
            <StatusCard
              label="Workers"
              value={`${healthyWorkers}/${health.workers.length}`}
              description={checkMessage(health.checks, 'worker') ?? 'Worker heartbeat'}
              ok={healthyWorkers > 0}
              icon={<IconActivityHeartbeat size={22} />}
            />
          </SimpleGrid>

          <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
            <MetricCard
              label="Pending jobs"
              value={health.queue.pending_jobs}
              description="Waiting for processing"
              warn={health.queue.pending_jobs > 0}
            />
            <MetricCard
              label="Processing jobs"
              value={health.queue.processing_jobs}
              description="Currently locked by workers"
            />
            <MetricCard
              label="Dead jobs"
              value={health.queue.dead_jobs}
              description="Manual attention needed"
              danger={health.queue.dead_jobs > 0}
            />
            <MetricCard
              label="Stale jobs"
              value={health.queue.stale_processing_jobs}
              description="Processing beyond threshold"
              danger={health.queue.stale_processing_jobs > 0}
            />
            <MetricCard
              label="Oldest pending"
              value={formatSeconds(health.queue.oldest_pending_seconds)}
              description="Age of the oldest runnable job"
              warn={health.queue.oldest_pending_seconds > 60}
            />
            <MetricCard
              label="Done jobs"
              value={health.queue.done_jobs}
              description="Processed jobs retained in queue"
            />
            <MetricCard
              label="SMTP failures"
              value={health.queue.smtp_failed_jobs}
              description="Invitation and recovery email failures"
              danger={health.queue.smtp_failed_jobs > 0}
            />
            <MetricCard
              label="Webhook failures"
              value={health.queue.webhook_failed_jobs}
              description="GitLab webhook processing failures"
              danger={health.queue.webhook_failed_jobs > 0}
            />
          </SimpleGrid>

          <WorkersTable workers={health.workers} />
          <RecentFailuresTable jobs={health.recent_failed_jobs} />
        </Stack>
      ) : null}
    </Stack>
  );
}

function StatusCard({
  label,
  value,
  description,
  ok,
  icon,
}: {
  label: string;
  value: string;
  description: string;
  ok: boolean;
  icon: ReactNode;
}) {
  return (
    <Card withBorder radius="md" padding="lg">
      <Stack gap="sm">
        <Group justify="space-between" align="flex-start">
          <Text c="dimmed" size="sm" fw={700} tt="uppercase">
            {label}
          </Text>
          {icon}
        </Group>
        <Badge color={ok ? 'green' : 'red'} variant="light" size="lg" style={{ alignSelf: 'flex-start' }}>
          {value}
        </Badge>
        <Text c="dimmed" size="sm" lineClamp={2}>
          {description}
        </Text>
      </Stack>
    </Card>
  );
}

function MetricCard({
  label,
  value,
  description,
  warn,
  danger,
}: {
  label: string;
  value: number | string;
  description: string;
  warn?: boolean;
  danger?: boolean;
}) {
  const color = danger ? 'red' : warn ? 'yellow' : 'blue';

  return (
    <Card withBorder radius="md" padding="lg">
      <Stack gap={6}>
        <Group justify="space-between" align="flex-start">
          <Text c="dimmed" size="sm" fw={700} tt="uppercase">
            {label}
          </Text>
          <Badge color={color} variant="dot">
            {danger ? 'alert' : warn ? 'watch' : 'ok'}
          </Badge>
        </Group>
        <Title order={2}>{value}</Title>
        <Text c="dimmed" size="sm">
          {description}
        </Text>
      </Stack>
    </Card>
  );
}

function WorkersTable({ workers }: { workers: WorkerHeartbeat[] }) {
  return (
    <Paper radius="md" p="lg" withBorder>
      <Stack gap="md">
        <Group justify="space-between">
          <Title order={3}>Workers</Title>
          <Badge color="blue" variant="light">
            {workers.length} known
          </Badge>
        </Group>

        {workers.length === 0 ? (
          <Alert icon={<IconAlertTriangle size={18} />} color="red" variant="light">
            No worker heartbeat has been recorded.
          </Alert>
        ) : (
          <ScrollArea>
            <Table verticalSpacing="sm" highlightOnHover miw={920}>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Worker</Table.Th>
                  <Table.Th>Status</Table.Th>
                  <Table.Th>Heartbeat age</Table.Th>
                  <Table.Th>Last job</Table.Th>
                  <Table.Th>Processed</Table.Th>
                  <Table.Th>Failed</Table.Th>
                  <Table.Th>Last error</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {workers.map((worker) => (
                  <Table.Tr key={worker.worker_id}>
                    <Table.Td>
                      <Tooltip label={worker.worker_id}>
                        <Text size="sm" ff="monospace">
                          {shortId(worker.worker_id)}
                        </Text>
                      </Tooltip>
                    </Table.Td>
                    <Table.Td>
                      <Badge color={worker.healthy ? 'green' : 'red'} variant="light">
                        {worker.status}
                      </Badge>
                    </Table.Td>
                    <Table.Td>{formatSeconds(worker.heartbeat_age_seconds)}</Table.Td>
                    <Table.Td>
                      <Text size="sm" lineClamp={1}>
                        {worker.last_job_topic ?? '-'}
                      </Text>
                    </Table.Td>
                    <Table.Td>{worker.processed_jobs}</Table.Td>
                    <Table.Td>{worker.failed_jobs}</Table.Td>
                    <Table.Td>
                      <Text size="sm" c={worker.last_error ? 'red' : 'dimmed'} lineClamp={2}>
                        {worker.last_error ?? '-'}
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </ScrollArea>
        )}
      </Stack>
    </Paper>
  );
}

function RecentFailuresTable({ jobs }: { jobs: RecentJobFailure[] }) {
  return (
    <Paper radius="md" p="lg" withBorder>
      <Stack gap="md">
        <Group justify="space-between">
          <Title order={3}>Recent failed jobs</Title>
          <Badge color={jobs.length > 0 ? 'red' : 'green'} variant="light">
            {jobs.length} listed
          </Badge>
        </Group>

        {jobs.length === 0 ? (
          <Alert color="green" variant="light">
            No failed or dead jobs are currently listed.
          </Alert>
        ) : (
          <ScrollArea>
            <Table verticalSpacing="sm" highlightOnHover miw={980}>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Job</Table.Th>
                  <Table.Th>Topic</Table.Th>
                  <Table.Th>Status</Table.Th>
                  <Table.Th>Attempts</Table.Th>
                  <Table.Th>Updated</Table.Th>
                  <Table.Th>Error</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {jobs.map((job) => (
                  <Table.Tr key={job.id}>
                    <Table.Td>
                      <Tooltip label={job.id}>
                        <Text size="sm" ff="monospace">
                          {shortId(job.id)}
                        </Text>
                      </Tooltip>
                    </Table.Td>
                    <Table.Td>
                      <Text size="sm" lineClamp={1}>
                        {job.topic}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Badge color={job.status === 'dead' ? 'red' : 'yellow'} variant="light">
                        {job.status}
                      </Badge>
                    </Table.Td>
                    <Table.Td>{job.attempt_count}</Table.Td>
                    <Table.Td>{formatDateTime(job.updated_at)}</Table.Td>
                    <Table.Td>
                      <Text size="sm" c="red" lineClamp={2}>
                        {job.last_error ?? '-'}
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </ScrollArea>
        )}
      </Stack>
    </Paper>
  );
}

function checkStatus(checks: HealthCheck[], name: string) {
  return checks.find((check) => check.name === name)?.status ?? 'unknown';
}

function checkMessage(checks: HealthCheck[], name: string) {
  return checks.find((check) => check.name === name)?.message ?? null;
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(new Date(value));
}

function formatSeconds(value: number) {
  if (value < 60) {
    return `${value}s`;
  }

  const minutes = Math.floor(value / 60);
  const seconds = value % 60;
  if (minutes < 60) {
    return seconds ? `${minutes}m ${seconds}s` : `${minutes}m`;
  }

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}

function shortId(value: string) {
  if (value.length <= 18) {
    return value;
  }

  return `${value.slice(0, 12)}...${value.slice(-6)}`;
}
