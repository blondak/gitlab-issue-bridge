import {
  Anchor,
  Button,
  Checkbox,
  Container,
  Group,
  Paper,
  PasswordInput,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';

import classes from './AuthenticationTitle.module.css';
import { useAppContext } from '../context/AppContext';

type LoginValues = {
  email: string;
  password: string;
  remember: boolean;
};

type LoginScreenProps = {
  loading?: boolean;
  error?: string | null;
  onSubmit: (values: { email: string; password: string }) => Promise<void> | void;
  onForgotPassword?: () => void;
};

export function LoginScreen({ loading, error, onSubmit, onForgotPassword }: LoginScreenProps) {
  const { t } = useAppContext();
  const form = useForm<LoginValues>({
    initialValues: {
      email: 'admin@example.com',
      password: 'admin1234',
      remember: true,
    },
    validate: {
      email: (value) => (/.+@.+\..+/.test(value) ? null : 'Zadej validni email.'),
      password: (value) => (value.length >= 4 ? null : 'Zadej heslo.'),
    },
  });

  return (
    <Container size={420} my={40}>
      <Title ta="center" order={1} className={classes.title}>
        {t('login.welcome')}
      </Title>
      <Text className={classes.subtitle}>
        {t('login.subtitle')}
      </Text>

      <Paper withBorder shadow="sm" p={22} mt={30} radius="md">
        <Text size="sm" fw={600} mb="lg">
          {t('login.signIn')}
        </Text>

        <form
          onSubmit={form.onSubmit(async (values) => {
            await onSubmit({ email: values.email, password: values.password });
          })}
        >
          <>
            <TextInput
              label={t('login.email')}
              placeholder="admin@example.com"
              required
              radius="md"
              {...form.getInputProps('email')}
            />

            <PasswordInput
              label={t('login.password')}
              placeholder="Your password"
              required
              mt="md"
              radius="md"
              {...form.getInputProps('password')}
            />

            <Group justify="space-between" mt="lg">
              <Checkbox
                label={t('login.remember')}
                {...form.getInputProps('remember', { type: 'checkbox' })}
              />
              <Anchor component="button" type="button" size="sm" onClick={onForgotPassword}>
                {t('login.forgot')}
              </Anchor>
            </Group>

            {error ? (
              <Text c="red" size="sm" mt="md">
                {error}
              </Text>
            ) : null}

            <Button type="submit" fullWidth mt="xl" radius="md" loading={loading}>
              {t('login.submit')}
            </Button>
          </>
        </form>
      </Paper>
    </Container>
  );
}
