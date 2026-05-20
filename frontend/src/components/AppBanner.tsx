import { Alert, CloseButton, Group } from '@mantine/core';
import { useAppContext } from '../context/AppContext';

type AppBannerProps = {
  type: 'error' | 'success';
  message: string;
  onClose: () => void;
};

export function AppBanner({ type, message, onClose }: AppBannerProps) {
  const { t } = useAppContext();
  return (
    <Alert
      color={type === 'error' ? 'red' : 'green'}
      title={type === 'error' ? t('common.error') : t('common.saved')}
      variant="light"
    >
      <Group justify="space-between" align="center" wrap="nowrap">
        <span>{message}</span>
        <CloseButton onClick={onClose} />
      </Group>
    </Alert>
  );
}
