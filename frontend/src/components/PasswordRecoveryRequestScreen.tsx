import { Button, Container, Paper, Stack, Text, TextInput, Title } from '@mantine/core';
import { useForm } from '@mantine/form';

import classes from './AuthenticationTitle.module.css';
import { useAppContext } from '../context/AppContext';
import type { PasswordRecoveryRequestValues } from '../types';

type PasswordRecoveryRequestScreenProps = {
  loading?: boolean;
  error?: string | null;
  onSubmit: (values: PasswordRecoveryRequestValues) => Promise<void> | void;
};

export function PasswordRecoveryRequestScreen({
  loading,
  error,
  onSubmit,
}: PasswordRecoveryRequestScreenProps) {
  const { t } = useAppContext();
  const form = useForm<PasswordRecoveryRequestValues>({
    initialValues: { email: '' },
    validate: {
      email: (value) => (/.+@.+\..+/.test(value) ? null : 'Zadej validni email.'),
    },
  });

  return (
    <Container size={420} my={40}>
      <Title ta="center" order={1} className={classes.title}>
        {t('passwordRecovery.title')}
      </Title>
      <Text className={classes.subtitle}>{t('passwordRecovery.subtitle')}</Text>

      <Paper withBorder shadow="sm" p={22} mt={30} radius="md">
        <form onSubmit={form.onSubmit(async (values) => onSubmit(values))}>
          <Stack gap="md">
            <TextInput
              label={t('login.email')}
              placeholder="user@example.com"
              required
              radius="md"
              {...form.getInputProps('email')}
            />
            {error ? (
              <Text c="red" size="sm">
                {error}
              </Text>
            ) : null}
            <Button type="submit" radius="md" loading={loading}>
              {t('passwordRecovery.send')}
            </Button>
          </Stack>
        </form>
      </Paper>
    </Container>
  );
}
