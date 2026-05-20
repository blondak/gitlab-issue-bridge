import {
  Badge,
  Button,
  Container,
  Paper,
  PasswordInput,
  Stack,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';

import type { InvitationPreview } from '../types';

type InvitationAcceptanceValues = {
  full_name: string;
  password: string;
  password_confirm: string;
};

type InvitationAcceptanceScreenProps = {
  invitation: InvitationPreview;
  loading?: boolean;
  error?: string | null;
  onSubmit: (values: { full_name: string; password: string }) => Promise<void> | void;
};

export function InvitationAcceptanceScreen({
  invitation,
  loading,
  error,
  onSubmit,
}: InvitationAcceptanceScreenProps) {
  const form = useForm<InvitationAcceptanceValues>({
    initialValues: {
      full_name: '',
      password: '',
      password_confirm: '',
    },
    validate: {
      full_name: (value) => (value.trim().length >= 2 ? null : 'Zadej sve jmeno.'),
      password: (value) =>
        value.length >= 8 ? null : 'Heslo musi mit alespon 8 znaku.',
      password_confirm: (value, values) =>
        value === values.password ? null : 'Hesla se musi shodovat.',
    },
  });

  return (
    <Container size={460} my={96}>
      <Title ta="center" order={1}>
        Accept invitation
      </Title>
      <Text c="dimmed" size="sm" ta="center" mt={6}>
        Dokonci onboarding a aktivuj svuj ucet v IssueHub.
      </Text>

      <Paper withBorder radius="md" p="xl" mt="xl">
        <Stack gap="md">
          <Stack gap={4}>
            <Text fw={600}>{invitation.email}</Text>
            <Badge color={invitation.is_admin ? 'teal' : 'blue'} variant="light" w="fit-content">
              {invitation.is_admin ? 'admin invitation' : 'member invitation'}
            </Badge>
            <Text c="dimmed" size="sm">
              Invitation expires {new Date(invitation.expires_at).toLocaleString('cs-CZ')}
            </Text>
          </Stack>

          <form
            onSubmit={form.onSubmit(async (values) => {
              await onSubmit({ full_name: values.full_name, password: values.password });
            })}
          >
            <Stack gap="md">
              <TextInput
                label="Full name"
                placeholder="John Doe"
                required
                {...form.getInputProps('full_name')}
              />
              <PasswordInput
                label="Password"
                placeholder="Create password"
                required
                {...form.getInputProps('password')}
              />
              <PasswordInput
                label="Confirm password"
                placeholder="Repeat password"
                required
                {...form.getInputProps('password_confirm')}
              />

              {error ? (
                <Text c="red" size="sm">
                  {error}
                </Text>
              ) : null}

              <Button type="submit" fullWidth loading={loading}>
                Activate account
              </Button>
            </Stack>
          </form>
        </Stack>
      </Paper>
    </Container>
  );
}
