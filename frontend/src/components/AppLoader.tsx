import { Center, Loader, Stack, Text } from '@mantine/core';
import { useAppContext } from '../context/AppContext';

export function AppLoader() {
  const context = useAppContext();
  return (
    <Center mih="100vh">
      <Stack gap="sm" align="center">
        <Loader color="blue" />
        <Text c="dimmed" size="sm">
          {context.t('common.loading')}
        </Text>
      </Stack>
    </Center>
  );
}
