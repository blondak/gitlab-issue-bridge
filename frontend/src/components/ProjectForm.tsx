import { Button, Checkbox, Group, Stack, TextInput, Textarea, Title } from '@mantine/core';
import { useForm } from '@mantine/form';

import type { ProjectFormValues } from '../types';

type ProjectFormProps = {
  title: string;
  submitLabel: string;
  initialValues: ProjectFormValues;
  loading?: boolean;
  includeActive?: boolean;
  onSubmit: (values: ProjectFormValues) => Promise<void> | void;
  onDelete?: () => Promise<void> | void;
  deleteLabel?: string;
};

export function ProjectForm({
  title,
  submitLabel,
  initialValues,
  loading,
  includeActive,
  onSubmit,
  onDelete,
  deleteLabel = 'Delete project',
}: ProjectFormProps) {
  const form = useForm<ProjectFormValues>({
    initialValues,
    validate: {
      slug: (value) => {
        if (!value.trim()) return 'Slug je povinny.';
        if (!/^[a-z0-9-]+$/.test(value)) return 'Pouzij jen mala pismena, cisla a pomlcky.';
        return null;
      },
      name: (value) => (value.trim() ? null : 'Name je povinny.'),
      description: (value) => (value.length > 500 ? 'Popis muze mit max 500 znaku.' : null),
    },
    enhanceGetInputProps: () => ({
      autoComplete: 'off',
    }),
  });

  return (
    <Stack gap="md">
      <Title order={3}>{title}</Title>
      <form onSubmit={form.onSubmit((values) => void onSubmit(values))}>
        <Stack gap="md">
          <TextInput label="Slug" placeholder="customer-portal" {...form.getInputProps('slug')} required />
          <TextInput label="Name" placeholder="Customer Portal" {...form.getInputProps('name')} required />
          <Textarea
            label="Description"
            placeholder="Interni projekt pro customer portal"
            minRows={4}
            {...form.getInputProps('description')}
          />
          {includeActive ? <Checkbox label="Active" {...form.getInputProps('active', { type: 'checkbox' })} /> : null}
          <Group justify="space-between">
            {onDelete ? (
              <Button type="button" color="red" variant="light" onClick={() => void onDelete()}>
                {deleteLabel}
              </Button>
            ) : (
              <span />
            )}
            <Button type="submit" loading={loading}>
              {submitLabel}
            </Button>
          </Group>
        </Stack>
      </form>
    </Stack>
  );
}
