import {
  Alert,
  Button,
  Card,
  Checkbox,
  Divider,
  Grid,
  Group,
  PasswordInput,
  Stack,
  Text,
  TextInput,
  Textarea,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';

import { useAppContext } from '../context/AppContext';
import type {
  GitLabIssueImportResult,
  GitLabIntegrationValidationResult,
  ProjectEditorValues,
} from '../types';

type ProjectEditorFormProps = {
  title: string;
  submitLabel: string;
  initialValues: ProjectEditorValues;
  loading?: boolean;
  validationLoading?: boolean;
  importLoading?: boolean;
  validationResult?: GitLabIntegrationValidationResult | null;
  importResult?: GitLabIssueImportResult | null;
  canManageExistingIntegration?: boolean;
  onSubmit: (values: ProjectEditorValues) => Promise<void> | void;
  onValidate?: (values: ProjectEditorValues) => Promise<void> | void;
  onImport?: () => Promise<void> | void;
  onDelete?: () => Promise<void> | void;
  onDeleteIntegration?: () => Promise<void> | void;
};

export function ProjectEditorForm({
  title,
  submitLabel,
  initialValues,
  loading,
  validationLoading,
  importLoading,
  validationResult,
  importResult,
  canManageExistingIntegration,
  onSubmit,
  onValidate,
  onImport,
  onDelete,
	onDeleteIntegration,
}: ProjectEditorFormProps) {
  const { t } = useAppContext();
  const form = useForm<ProjectEditorValues>({
    initialValues,
    validate: {
      slug: (value) => {
        if (!value.trim()) return t('projects.slugRequired');
        if (!/^[a-z0-9-]+$/.test(value)) return t('projects.slugFormat');
        return null;
      },
      name: (value) => (value.trim() ? null : t('projects.nameRequired')),
      description: (value) => (value.length > 500 ? t('projects.descriptionTooLong') : null),
      gitlab_base_url: (value, values) =>
        values.enable_gitlab_integration && !/^https?:\/\//.test(value)
          ? t('projects.gitlabBaseUrlRequired')
          : null,
      gitlab_api_base_url: (value, values) =>
        values.enable_gitlab_integration && !/^https?:\/\//.test(value)
          ? t('projects.gitlabApiUrlRequired')
          : null,
      gitlab_project_id: (value, values) =>
        values.enable_gitlab_integration && !/^\d+$/.test(value)
          ? t('projects.gitlabProjectIdRequired')
          : null,
      token: (value, values) =>
        values.enable_gitlab_integration && !canManageExistingIntegration && value.trim().length < 6
          ? t('projects.tokenRequired')
          : null,
      webhook_secret: (value, values) =>
        values.enable_gitlab_integration && !canManageExistingIntegration && value.trim().length < 4
          ? t('projects.webhookSecretRequired')
          : null,
    },
  });

  return (
    <Card withBorder radius="md" padding="lg">
      <Stack gap="md">
        <Title order={3}>{title}</Title>
        <form onSubmit={form.onSubmit((values) => void onSubmit(values))}>
          <Stack gap="lg">
            <Grid>
              <Grid.Col span={{ base: 12, md: 6 }}>
                <TextInput label={t('projects.slug')} placeholder="customer-portal" {...form.getInputProps('slug')} required />
              </Grid.Col>
              <Grid.Col span={{ base: 12, md: 6 }}>
                <TextInput label={t('projects.name')} placeholder="Customer Portal" {...form.getInputProps('name')} required />
              </Grid.Col>
              <Grid.Col span={12}>
                <Textarea
                  label={t('projects.description')}
                  placeholder={t('projects.descriptionPlaceholder')}
                  minRows={4}
                  {...form.getInputProps('description')}
                />
              </Grid.Col>
              <Grid.Col span={{ base: 12, md: 4 }}>
                <Checkbox label={t('projects.active')} mt="xl" {...form.getInputProps('active', { type: 'checkbox' })} />
              </Grid.Col>
            </Grid>

            <Divider />

            <Stack gap="sm">
              <Group justify="space-between" align="center">
                <div>
                  <Title order={4}>{t('projects.gitlabIntegrationTitle')}</Title>
                  <Text c="dimmed" size="sm">
                    {t('projects.gitlabOptional')}
                  </Text>
                </div>
                <Checkbox
                  label={t('projects.enableGitlab')}
                  {...form.getInputProps('enable_gitlab_integration', { type: 'checkbox' })}
                />
              </Group>

              {form.values.enable_gitlab_integration ? (
                <Grid>
                  <Grid.Col span={{ base: 12, md: 6 }}>
                    <TextInput
                      label={t('projects.gitlabBaseUrl')}
                      placeholder="https://gitlab.example.com"
                      {...form.getInputProps('gitlab_base_url')}
                      required
                    />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 6 }}>
                    <TextInput
                      label={t('projects.gitlabApiBaseUrl')}
                      placeholder="https://gitlab.example.com/api/v4"
                      {...form.getInputProps('gitlab_api_base_url')}
                      required
                    />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 4 }}>
                    <TextInput
                      label={t('projects.gitlabProjectId')}
                      placeholder="12345"
                      {...form.getInputProps('gitlab_project_id')}
                      required
                    />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 4 }}>
                    <PasswordInput
                      label={t('projects.token')}
                      placeholder={canManageExistingIntegration ? t('projects.tokenRotatePlaceholder') : 'glpat-...'}
                      {...form.getInputProps('token')}
                    />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 4 }}>
                    <PasswordInput
                      label={t('projects.webhookSecret')}
                      placeholder={canManageExistingIntegration ? t('projects.secretRotatePlaceholder') : 'secret'}
                      {...form.getInputProps('webhook_secret')}
                    />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 3 }}>
                    <Checkbox mt="xl" label={t('projects.verifyTls')} {...form.getInputProps('verify_tls', { type: 'checkbox' })} />
                  </Grid.Col>
                  <Grid.Col span={{ base: 12, md: 3 }}>
                    <Checkbox mt="xl" label={t('projects.syncEnabled')} {...form.getInputProps('sync_enabled', { type: 'checkbox' })} />
                  </Grid.Col>
                </Grid>
              ) : null}
            </Stack>

            <Group justify="space-between">
              <Group>
                {onDelete ? (
                  <Button type="button" color="red" variant="light" onClick={() => void onDelete()}>
                    {t('projects.delete')}
                  </Button>
                ) : null}
                {onDeleteIntegration && canManageExistingIntegration ? (
                  <Button type="button" color="red" variant="default" onClick={() => void onDeleteIntegration()}>
                    {t('projects.deleteIntegration')}
                  </Button>
                ) : null}
              </Group>

              <Group>
                {onValidate && form.values.enable_gitlab_integration && canManageExistingIntegration ? (
                  <Button type="button" variant="light" loading={validationLoading} onClick={() => void onValidate(form.getValues())}>
                    {t('projects.validateIntegration')}
                  </Button>
                ) : null}
                {onImport && form.values.enable_gitlab_integration && canManageExistingIntegration ? (
                  <Button type="button" variant="default" loading={importLoading} onClick={() => void onImport()}>
                    {t('projects.syncIssues')}
                  </Button>
                ) : null}
                <Button type="submit" loading={loading}>
                  {submitLabel}
                </Button>
              </Group>
            </Group>
          </Stack>
        </form>

        {validationResult ? (
          <Alert color="green" title={t('projects.validationSucceeded')}>
            <Stack gap={4}>
              <Text size="sm">
                {t('projects.table.name')}: <strong>{validationResult.project_name}</strong>
              </Text>
              <Text size="sm">
                {t('projects.visibility')}: <strong>{validationResult.visibility}</strong>
              </Text>
              <Text size="sm">{validationResult.web_url}</Text>
            </Stack>
          </Alert>
        ) : null}

        {importResult ? (
          <Alert color="blue" title={t('projects.importCompleted')}>
            <Stack gap={4}>
              <Text size="sm">
                {t('projects.imported')}: <strong>{importResult.imported_count}</strong>
              </Text>
              <Text size="sm">
                {t('projects.created')}: <strong>{importResult.created_count}</strong>
              </Text>
              <Text size="sm">
                {t('projects.updated')}: <strong>{importResult.updated_count}</strong>
              </Text>
            </Stack>
          </Alert>
        ) : null}
      </Stack>
    </Card>
  );
}
