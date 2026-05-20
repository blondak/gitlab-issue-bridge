import { Card, Stack, Text, Title } from '@mantine/core';

export function MetricCard({ label, value }: { label: string; value: number | undefined }) {
  return (
    <Card withBorder radius="md" padding="lg">
      <Stack gap={4}>
        <Text c="dimmed" size="sm">
          {label}
        </Text>
        <Title order={2}>{value ?? '...'}</Title>
      </Stack>
    </Card>
  );
}
