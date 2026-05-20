import { Button, Card, PasswordInput, Select, Stack, Text, TextInput, Title } from '@mantine/core';
import { useForm } from '@mantine/form';

import { useAppContext } from '../context/AppContext';
import type { ChangePasswordValues, UpdateProfileValues } from '../types';

export function ProfilePage() {
  const { currentUser, updateProfile, changePassword, t, locale } = useAppContext();
  const form = useForm<UpdateProfileValues>({
    initialValues: {
      full_name: currentUser?.full_name ?? '',
      preferred_language: currentUser?.preferred_language ?? null,
    },
  });
  const passwordForm = useForm<ChangePasswordValues & { new_password_confirm: string }>({
    initialValues: {
      current_password: '',
      new_password: '',
      new_password_confirm: '',
    },
    validate: {
      current_password: (value) => (value.trim() ? null : t('profile.currentPasswordRequired')),
      new_password: (value) => (value.trim().length >= 8 ? null : t('profile.newPasswordTooShort')),
      new_password_confirm: (value, values) =>
        value === values.new_password ? null : t('profile.passwordsMismatch'),
    },
  });

  return (
    <Stack gap="lg">
      <Card withBorder radius="md" padding="lg">
        <Stack gap="md">
          <Title order={2}>{t('profile.title')}</Title>
          <Text c="dimmed">{t('profile.subtitle')}</Text>
          <form
            onSubmit={form.onSubmit(async (values) => {
              await updateProfile(values);
            })}
          >
            <Stack gap="md">
              <TextInput
                label={t('profile.fullName')}
                {...form.getInputProps('full_name')}
                required
              />
              <Select
                label={t('profile.preferredLanguage')}
                clearable
                data={[
                  { value: 'cs', label: t('common.czech') },
                  { value: 'en', label: t('common.english') },
                ]}
                placeholder={t('profile.followBrowser')}
                {...form.getInputProps('preferred_language')}
              />
              <Button type="submit">{t('profile.save')}</Button>
              <Text size="sm" c="dimmed">
                {locale === 'cs'
                  ? 'Pokud jazyk nevybereš, použije se preference prohlížeče.'
                  : 'If no language is selected, browser preference is used.'}
              </Text>
            </Stack>
          </form>
        </Stack>
      </Card>

      <Card withBorder radius="md" padding="lg">
        <Stack gap="md">
          <Title order={3}>{t('profile.changePasswordTitle')}</Title>
          <Text c="dimmed">{t('profile.changePasswordSubtitle')}</Text>
          <form
            onSubmit={passwordForm.onSubmit(async (values) => {
              await changePassword({
                current_password: values.current_password,
                new_password: values.new_password,
              });
              passwordForm.reset();
            })}
          >
            <Stack gap="md">
              <PasswordInput
                label={t('profile.currentPassword')}
                {...passwordForm.getInputProps('current_password')}
                required
              />
              <PasswordInput
                label={t('profile.newPassword')}
                {...passwordForm.getInputProps('new_password')}
                required
              />
              <PasswordInput
                label={t('profile.newPasswordConfirm')}
                {...passwordForm.getInputProps('new_password_confirm')}
                required
              />
              <Button type="submit">{t('profile.changePassword')}</Button>
            </Stack>
          </form>
        </Stack>
      </Card>
    </Stack>
  );
}
