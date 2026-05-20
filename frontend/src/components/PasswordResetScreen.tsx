import { Button, Container, Paper, PasswordInput, Stack, Text, Title } from '@mantine/core';
import { useForm } from '@mantine/form';

import classes from './AuthenticationTitle.module.css';
import { useAppContext } from '../context/AppContext';
import type { PasswordRecoveryPreview, PasswordResetValues } from '../types';

type PasswordResetFormValues = PasswordResetValues & {
  password_confirm: string;
};

type PasswordResetScreenProps = {
  preview: PasswordRecoveryPreview;
  loading?: boolean;
  error?: string | null;
  onSubmit: (values: PasswordResetValues) => Promise<void> | void;
};

export function PasswordResetScreen({ preview, loading, error, onSubmit }: PasswordResetScreenProps) {
  const { t } = useAppContext();
  const form = useForm<PasswordResetFormValues>({
    initialValues: {
      password: '',
      password_confirm: '',
    },
    validate: {
      password: (value) => (value.trim().length >= 8 ? null : t('passwordReset.passwordTooShort')),
      password_confirm: (value, values) =>
        value === values.password ? null : t('passwordReset.passwordsMismatch'),
    },
  });

  return (
    <Container size={420} my={40}>
      <Title ta="center" order={1} className={classes.title}>
        {t('passwordReset.title')}
      </Title>
      <Text className={classes.subtitle}>
        {t('passwordReset.subtitle')} {preview.email}
      </Text>

      <Paper withBorder shadow="sm" p={22} mt={30} radius="md">
        <form
          onSubmit={form.onSubmit(async (values) => {
            await onSubmit({ password: values.password });
          })}
        >
          <Stack gap="md">
            <PasswordInput
              label={t('passwordReset.password')}
              required
              radius="md"
              {...form.getInputProps('password')}
            />
            <PasswordInput
              label={t('passwordReset.passwordConfirm')}
              required
              radius="md"
              {...form.getInputProps('password_confirm')}
            />
            {error ? (
              <Text c="red" size="sm">
                {error}
              </Text>
            ) : null}
            <Button type="submit" radius="md" loading={loading}>
              {t('passwordReset.submit')}
            </Button>
          </Stack>
        </form>
      </Paper>
    </Container>
  );
}
