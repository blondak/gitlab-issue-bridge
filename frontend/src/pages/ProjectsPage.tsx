import {
  Badge,
  Button,
  Center,
  Group,
  ScrollArea,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';
import { Link, Navigate, useLocation, useNavigate, useParams } from 'react-router-dom';

import classes from '../components/TableSort.module.css';
import { PageState } from '../components/PageState';
import { ProjectAccessPanel } from '../components/ProjectAccessPanel';
import { ProjectEditorForm } from '../components/ProjectEditorForm';
import { useAppContext } from '../context/AppContext';
import type {
  GitLabIssueImportResult,
  GitLabIntegrationValidationResult,
  IntegrationFormValues,
  ProjectAccessOverview,
  ProjectEditorValues,
  ProjectFormValues,
} from '../types';

function projectToEditorValues(project?: {
  slug: string;
  name: string;
  description: string;
  active: boolean;
  gitlab_integration: {
    gitlab_base_url: string;
    gitlab_api_base_url: string;
    gitlab_project_id: number;
    verify_tls: boolean;
    sync_enabled: boolean;
  } | null;
}): ProjectEditorValues {
  return {
    slug: project?.slug ?? '',
    name: project?.name ?? '',
    description: project?.description ?? '',
    active: project?.active ?? true,
    enable_gitlab_integration: Boolean(project?.gitlab_integration),
    gitlab_base_url: project?.gitlab_integration?.gitlab_base_url ?? '',
    gitlab_api_base_url: project?.gitlab_integration?.gitlab_api_base_url ?? '',
    gitlab_project_id: project?.gitlab_integration
      ? String(project.gitlab_integration.gitlab_project_id)
      : '',
    token: '',
    webhook_secret: '',
    verify_tls: project?.gitlab_integration?.verify_tls ?? true,
    sync_enabled: project?.gitlab_integration?.sync_enabled ?? true,
  };
}

export function ProjectsPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { projectId } = useParams();
  const {
    currentUser,
    projects,
    dataLoading,
    createProject,
    updateProject,
    deleteProject,
    saveIntegration,
    deleteIntegration,
    getProjectAccess,
    updateProjectAccess,
    validateIntegration,
    importProjectIssues,
    t,
  } = useAppContext();
  const [submitting, setSubmitting] = useState(false);
  const [validationLoading, setValidationLoading] = useState(false);
  const [importLoading, setImportLoading] = useState(false);
  const [validationResult, setValidationResult] = useState<GitLabIntegrationValidationResult | null>(null);
  const [importResult, setImportResult] = useState<GitLabIssueImportResult | null>(null);
  const [projectAccess, setProjectAccess] = useState<ProjectAccessOverview | null>(null);
  const [projectAccessLoading, setProjectAccessLoading] = useState(false);
  const [projectAccessSaving, setProjectAccessSaving] = useState(false);
  const [search, setSearch] = useState('');
  const [sortBy, setSortBy] = useState<'name' | 'slug' | 'active' | 'gitlab' | null>('name');
  const [reverseSortDirection, setReverseSortDirection] = useState(false);
  const manageableProjects = useMemo(
    () => projects.filter((project) => project.capabilities.can_manage),
    [projects],
  );

  const isCreateRoute = location.pathname === '/projects/new';
  const isDetailRoute = Boolean(projectId) && !isCreateRoute;
  const selectedProject = manageableProjects.find((project) => project.id === projectId) ?? null;
  const sortedProjects = useMemo(() => {
    const query = search.trim().toLowerCase();
    const filtered = manageableProjects.filter((project) =>
      [project.name, project.slug, project.description, project.active ? 'active' : 'inactive']
        .some((value) => value.toLowerCase().includes(query)),
    );

    if (!sortBy) {
      return filtered;
    }

    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'active') {
        return Number(a.active) - Number(b.active);
      }

      if (sortBy === 'gitlab') {
        return Number(Boolean(a.gitlab_integration)) - Number(Boolean(b.gitlab_integration));
      }

      return String(a[sortBy]).localeCompare(String(b[sortBy]));
    });

    return reverseSortDirection ? sorted.reverse() : sorted;
  }, [manageableProjects, reverseSortDirection, search, sortBy]);

  if (isCreateRoute && !currentUser?.is_admin) {
    return <Navigate to="/overview" replace />;
  }

  if (!currentUser?.is_admin && manageableProjects.length === 0) {
    return <Navigate to="/overview" replace />;
  }

  if (projectId && !selectedProject && !dataLoading) {
    return <Navigate to="/projects" replace />;
  }

  async function loadProjectAccess(currentProjectId: string) {
    setProjectAccessLoading(true);
    try {
      const overview = await getProjectAccess(currentProjectId);
      setProjectAccess(overview);
    } finally {
      setProjectAccessLoading(false);
    }
  }

  async function persistProjectAndIntegration(
    projectValues: ProjectFormValues,
    integrationValues: ProjectEditorValues,
    existingProjectId?: string,
  ) {
    const project = existingProjectId
      ? await updateProject(existingProjectId, projectValues)
      : await createProject(projectValues);

    if (integrationValues.enable_gitlab_integration) {
      const payload: IntegrationFormValues = {
        gitlab_base_url: integrationValues.gitlab_base_url,
        gitlab_api_base_url: integrationValues.gitlab_api_base_url,
        gitlab_project_id: integrationValues.gitlab_project_id,
        token: integrationValues.token,
        webhook_secret: integrationValues.webhook_secret,
        verify_tls: integrationValues.verify_tls,
        sync_enabled: integrationValues.sync_enabled,
      };

      await saveIntegration(project.id, payload);
    } else if (existingProjectId && selectedProject?.gitlab_integration) {
      await deleteIntegration(existingProjectId);
    }

    return project;
  }

  async function handleCreate(values: ProjectEditorValues) {
    setSubmitting(true);
    try {
      const project = await persistProjectAndIntegration(
        {
          slug: values.slug,
          name: values.name,
          description: values.description,
          active: values.active,
        },
        values,
      );
      navigate(`/projects/${project.id}`, { replace: true });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleUpdate(values: ProjectEditorValues) {
    if (!selectedProject) return;
    setSubmitting(true);
    try {
      const project = await persistProjectAndIntegration(
        {
          slug: values.slug,
          name: values.name,
          description: values.description,
          active: values.active,
        },
        values,
        selectedProject.id,
      );
      navigate(`/projects/${project.id}`, { replace: true });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDeleteProject() {
    if (!selectedProject || !window.confirm(`Opravdu smazat projekt ${selectedProject.name}?`)) return;

    setSubmitting(true);
    try {
      await deleteProject(selectedProject.id);
      navigate('/projects', { replace: true });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDeleteIntegration() {
    if (!selectedProject?.gitlab_integration || !window.confirm('Opravdu smazat GitLab integraci?')) return;

    setSubmitting(true);
    try {
      await deleteIntegration(selectedProject.id);
      setValidationResult(null);
      setImportResult(null);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleSaveProjectAccess(values: {
    assignments: Array<{ subject_type: 'user' | 'email'; subject_id: string; permission: string }>;
  }) {
    if (!selectedProject) return;
    setProjectAccessSaving(true);
    try {
      const overview = await updateProjectAccess(selectedProject.id, values);
      setProjectAccess(overview);
    } finally {
      setProjectAccessSaving(false);
    }
  }

  async function handleValidate(values: ProjectEditorValues) {
    if (!selectedProject) return;
    setValidationLoading(true);
    try {
      const result = await validateIntegration(selectedProject.id, {
        gitlab_base_url: values.gitlab_base_url,
        gitlab_api_base_url: values.gitlab_api_base_url,
        gitlab_project_id: values.gitlab_project_id,
        token: values.token,
        webhook_secret: values.webhook_secret,
        verify_tls: values.verify_tls,
        sync_enabled: values.sync_enabled,
      });
      setValidationResult(result);
    } finally {
      setValidationLoading(false);
    }
  }

  async function handleImport() {
    if (!selectedProject) return;
    setImportLoading(true);
    try {
      const result = await importProjectIssues(selectedProject.id);
      setImportResult(result);
    } finally {
      setImportLoading(false);
    }
  }

  useEffect(() => {
    if (selectedProject) {
      void loadProjectAccess(selectedProject.id);
    } else {
      setProjectAccess(null);
    }
  }, [selectedProject?.id]);

  function setSorting(field: 'name' | 'slug' | 'active' | 'gitlab') {
    const reversed = field === sortBy ? !reverseSortDirection : false;
    setReverseSortDirection(reversed);
    setSortBy(field);
  }

  if (!isCreateRoute && !isDetailRoute) {
    return (
      <Stack gap="lg">
        <Group justify="space-between" align="flex-end">
          <Stack gap={4}>
            <Title order={2}>{t('projects.title')}</Title>
            <Text c="dimmed">{t('projects.subtitle')}</Text>
          </Stack>
          {currentUser?.is_admin ? (
            <Button component={Link} to="/projects/new">
              {t('projects.create')}
            </Button>
          ) : null}
        </Group>

	        <PageState loading={dataLoading && projects.length === 0} />
	        <TextInput
	          placeholder={t('projects.search')}
	          value={search}
	          onChange={(event) => setSearch(event.currentTarget.value)}
	        />
        <ScrollArea>
          <Table verticalSpacing="md" highlightOnHover miw={760}>
            <Table.Thead>
              <Table.Tr>
                <SortableTh
                  sorted={sortBy === 'name'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('name')}
                >
	                  {t('projects.table.name')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'slug'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('slug')}
                >
	                  {t('projects.table.slug')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'active'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('active')}
                >
	                  {t('projects.table.status')}
                </SortableTh>
                <SortableTh
                  sorted={sortBy === 'gitlab'}
                  reversed={reverseSortDirection}
                  onSort={() => setSorting('gitlab')}
                >
	                  {t('projects.table.gitlab')}
                </SortableTh>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {sortedProjects.length > 0 ? (
                sortedProjects.map((project) => (
                  <Table.Tr key={project.id}>
                    <Table.Td>
                      <Stack gap={2}>
                        <Text fw={600}>{project.name}</Text>
	                        <Text size="sm" c="dimmed">
	                          {project.description || t('common.noDescription')}
	                        </Text>
                      </Stack>
                    </Table.Td>
                    <Table.Td>{project.slug}</Table.Td>
                    <Table.Td>
	                      <Badge color={project.active ? 'teal' : 'gray'} variant="light">
	                        {project.active ? t('common.active') : t('common.inactive')}
	                      </Badge>
                    </Table.Td>
                    <Table.Td>
	                      <Badge color={project.gitlab_integration ? 'blue' : 'gray'} variant="light">
	                        {project.gitlab_integration ? t('common.connected') : t('common.optionalOff')}
	                      </Badge>
                    </Table.Td>
                    <Table.Td>
                      <Button
                        size="xs"
                        variant="light"
	                        component={Link}
	                        to={`/projects/${project.id}`}
	                      >
	                        {t('projects.manage')}
	                      </Button>
                    </Table.Td>
                  </Table.Tr>
                ))
              ) : (
	                <Table.Tr>
	                  <Table.Td colSpan={5}>
	                    <Stack gap={4} align="center" py="xl">
	                      <Text fw={600} ta="center">
	                        {manageableProjects.length === 0 ? t('projects.emptyTitle') : t('projects.emptyFilteredTitle')}
	                      </Text>
	                      <Text ta="center" c="dimmed" size="sm">
	                        {manageableProjects.length === 0 ? t('projects.emptyMessage') : t('projects.emptyFilteredMessage')}
	                      </Text>
	                      {manageableProjects.length === 0 && currentUser?.is_admin ? (
	                        <Button component={Link} to="/projects/new" size="xs" variant="light">
	                          {t('projects.create')}
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
    );
  }

  return (
	    <Stack gap="lg">
	      <Group justify="space-between" align="flex-end">
	        <Stack gap={4}>
	          <Title order={2}>
	            {isCreateRoute ? t('projects.createTitle') : `${t('projects.editTitle')}: ${selectedProject?.name}`}
	          </Title>
	          <Text c="dimmed">
	            {t('projects.editorSubtitle')}
	          </Text>
	        </Stack>
	        <Button component={Link} to="/projects" variant="light">
	          {t('projects.back')}
	        </Button>
      </Group>

      {isCreateRoute ? (
	        <ProjectEditorForm
	          title={t('projects.createTitle')}
	          submitLabel={t('projects.create')}
          initialValues={projectToEditorValues()}
          loading={submitting}
          onSubmit={handleCreate}
        />
      ) : selectedProject ? (
        <Stack gap="xl">
	          <ProjectEditorForm
	            title={`${t('projects.editTitle')}: ${selectedProject.name}`}
	            submitLabel={t('projects.save')}
            initialValues={projectToEditorValues(selectedProject)}
            loading={submitting}
            validationLoading={validationLoading}
            importLoading={importLoading}
            validationResult={validationResult}
            importResult={importResult}
            canManageExistingIntegration={Boolean(selectedProject.gitlab_integration)}
            onSubmit={handleUpdate}
            onValidate={handleValidate}
            onImport={handleImport}
            onDelete={currentUser?.is_admin ? handleDeleteProject : undefined}
            onDeleteIntegration={handleDeleteIntegration}
          />
          <ProjectAccessPanel
            accessOverview={projectAccess}
            loading={projectAccessLoading}
            saving={projectAccessSaving}
            onSave={handleSaveProjectAccess}
            onIssuePermissionRemoved={() => void loadProjectAccess(selectedProject.id)}
          />
        </Stack>
      ) : null}
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
