import { Badge, Box, Group, Paper, Stack, Text, Title } from '@mantine/core';

import { useAppContext } from '../context/AppContext';
import type { Project } from '../types';

type ProjectListProps = {
  projects: Project[];
  selectedProjectId: string | null;
  onSelect: (projectId: string) => void;
};

export function ProjectList({ projects, selectedProjectId, onSelect }: ProjectListProps) {
  const { t } = useAppContext();

  return (
    <Stack gap="md">
      <Title order={3}>{t('projects.title')}</Title>
      {projects.length > 0 ? (
        <Stack gap="sm">
          {projects.map((project) => (
            <Paper
              key={project.id}
              withBorder
              radius="md"
              p="md"
              style={{
                cursor: 'pointer',
                borderColor: selectedProjectId === project.id ? 'var(--mantine-color-blue-5)' : undefined,
                background:
                  selectedProjectId === project.id
                    ? 'light-dark(var(--mantine-color-blue-0), rgba(25, 113, 194, 0.16))'
                    : undefined,
              }}
              onClick={() => onSelect(project.id)}
            >
              <Group justify="space-between" align="flex-start">
                <Box>
                  <Text fw={700}>{project.name}</Text>
                  <Text c="dimmed" size="sm">
                    {project.slug}
                  </Text>
                </Box>
                <Group gap={6}>
                  <Badge color={project.active ? 'teal' : 'gray'} variant="light">
                    {project.active ? t('common.active') : t('common.inactive')}
                  </Badge>
                  <Badge color={project.gitlab_integration ? 'blue' : 'gray'} variant="light">
                    {project.gitlab_integration ? t('common.connected') : t('common.optionalOff')}
                  </Badge>
                </Group>
              </Group>
            </Paper>
          ))}
        </Stack>
      ) : (
        <Text c="dimmed" size="sm">
          {t('projects.emptyMessage')}
        </Text>
      )}
    </Stack>
  );
}
