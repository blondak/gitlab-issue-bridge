import { Alert, Button, Checkbox, Grid, Group, PasswordInput, Stack, Text, TextInput, Title } from '@mantine/core';
import { useForm } from '@mantine/form';

import type {
  GitLabIssueImportResult,
  GitLabIntegrationValidationResult,
  IntegrationFormValues,
  Project,
} from '../types';

type IntegrationFormProps = {
  project: Project | null;
  initialValues: IntegrationFormValues;
  loading?: boolean;
  validationLoading?: boolean;
  importLoading?: boolean;
  validationResult?: GitLabIntegrationValidationResult | null;
  importResult?: GitLabIssueImportResult | null;
  onSubmit: (values: IntegrationFormValues) => Promise<void> | void;
  onValidate?: (values: IntegrationFormValues) => Promise<void> | void;
  onImport?: () => Promise<void> | void;
  onDelete?: () => Promise<void> | void;
};

export function IntegrationForm({
  project,
  initialValues,
  loading,
  validationLoading,
  importLoading,
  validationResult,
  importResult,
  onSubmit,
  onValidate,
  onImport,
  onDelete,
}: IntegrationFormProps) {
  const form = useForm<IntegrationFormValues>({
    initialValues,
    validate: {
      gitlab_base_url: (value) => (/^https?:\/\//.test(value) ? null : 'Zadej validni GitLab base URL.'),
      gitlab_api_base_url: (value) => (/^https?:\/\//.test(value) ? null : 'Zadej validni GitLab API URL.'),
      gitlab_project_id: (value) => (/^\d+$/.test(value) ? null : 'GitLab project ID musi byt cislo.'),
      token: (value) => (value.trim().length >= 6 ? null : 'Token je povinny.'),
      webhook_secret: (value) => (value.trim().length >= 4 ? null : 'Webhook secret je povinny.'),
    },
  });

  if (!project) {
    return <Text c="dimmed">Vyber projekt, pro ktery chces nastavit mapovani na GitLab.</Text>;
  }

  return (
    <Stack gap="md">
      <Title order={3}>GitLab integration</Title>
      <form onSubmit={form.onSubmit((values) => void onSubmit(values))}>
        <Grid>
          <Grid.Col span={{ base: 12, md: 6 }}>
            <TextInput
              label="GitLab base URL"
              placeholder="https://gitlab.example.com"
              {...form.getInputProps('gitlab_base_url')}
              required
            />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 6 }}>
            <TextInput
              label="GitLab API base URL"
              placeholder="https://gitlab.example.com/api/v4"
              {...form.getInputProps('gitlab_api_base_url')}
              required
            />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 4 }}>
            <TextInput
              label="GitLab project ID"
              placeholder="12345"
              {...form.getInputProps('gitlab_project_id')}
              required
            />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 4 }}>
            <PasswordInput
              label="Token"
              placeholder={project.gitlab_integration ? 'Vypln novy token pro update' : 'glpat-...'}
              {...form.getInputProps('token')}
              required
            />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 4 }}>
            <PasswordInput
              label="Webhook secret"
              placeholder="secret"
              {...form.getInputProps('webhook_secret')}
              required
            />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 3 }}>
            <Checkbox mt="xl" label="Verify TLS" {...form.getInputProps('verify_tls', { type: 'checkbox' })} />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 3 }}>
            <Checkbox mt="xl" label="Sync enabled" {...form.getInputProps('sync_enabled', { type: 'checkbox' })} />
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 6 }}>
            <Group justify="flex-end" mt="lg">
              {onValidate ? (
                <Button
                  type="button"
                  variant="light"
                  loading={validationLoading}
                  onClick={() => {
                    const values = form.getValues();
                    const validationErrors = {
                      gitlab_base_url:
                        /^https?:\/\//.test(values.gitlab_base_url)
                          ? null
                          : 'Zadej validni GitLab base URL.',
                      gitlab_api_base_url:
                        /^https?:\/\//.test(values.gitlab_api_base_url)
                          ? null
                          : 'Zadej validni GitLab API URL.',
                      gitlab_project_id:
                        /^\d+$/.test(values.gitlab_project_id)
                          ? null
                          : 'GitLab project ID musi byt cislo.',
                    };

                    form.setErrors(validationErrors);
                    if (Object.values(validationErrors).every((value) => value === null)) {
                      void onValidate(form.getValues());
                    }
                  }}
                >
                  Validate integration
                </Button>
              ) : null}
              {onDelete && project.gitlab_integration ? (
                <Button type="button" color="red" variant="light" onClick={() => void onDelete()}>
                  Delete integration
                </Button>
              ) : null}
              {onImport && project.gitlab_integration ? (
                <Button type="button" variant="default" loading={importLoading} onClick={() => void onImport()}>
                  Import issues
                </Button>
              ) : null}
              <Button type="submit" loading={loading}>
                Save GitLab integration
              </Button>
            </Group>
          </Grid.Col>
        </Grid>
      </form>
      {validationResult ? (
        <Alert color="green" title="GitLab validation succeeded">
          <Stack gap={4}>
            <Text size="sm">
              Project: <strong>{validationResult.project_name}</strong>
            </Text>
            <Text size="sm">
              Visibility: <strong>{validationResult.visibility}</strong>
            </Text>
            <Text size="sm">{validationResult.web_url}</Text>
          </Stack>
        </Alert>
      ) : null}
      {importResult ? (
        <Alert color="blue" title="GitLab import completed">
          <Stack gap={4}>
            <Text size="sm">
              Imported: <strong>{importResult.imported_count}</strong>
            </Text>
            <Text size="sm">
              Created: <strong>{importResult.created_count}</strong>
            </Text>
            <Text size="sm">
              Updated: <strong>{importResult.updated_count}</strong>
            </Text>
          </Stack>
        </Alert>
      ) : null}
    </Stack>
  );
}
