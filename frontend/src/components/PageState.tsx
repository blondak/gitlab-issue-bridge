import { Alert, Center, Loader, Stack, Text } from '@mantine/core';
import { useAppContext } from '../context/AppContext';

type PageStateProps = {
  loading?: boolean;
  error?: string | null;
  empty?: boolean;
  emptyMessage?: string;
};

export function PageState({ loading, error, empty, emptyMessage }: PageStateProps) {
  const { t } = useAppContext();
  if (loading) {
    return (
      <Center py={80}>
        <Loader color="blue" />
      </Center>
    );
  }

  if (error) {
    return (
      <Alert color="red" title={t('common.error')}>
        {error}
      </Alert>
    );
  }

  if (empty) {
    return (
      <Center py={80}>
        <Stack gap={4} align="center">
          <Text fw={600}>{t('common.nothing')}</Text>
          <Text c="dimmed" size="sm">
            {emptyMessage}
          </Text>
        </Stack>
      </Center>
    );
  }

  return null;
}
