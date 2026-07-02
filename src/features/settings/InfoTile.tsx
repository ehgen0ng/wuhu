import { Card, Group, Stack, Text, Title } from "@mantine/core";
import type { ReactNode } from "react";

type InfoTileProps = {
  label: string;
  value: string;
  detail?: string;
  action?: ReactNode;
};

export function InfoTile({ label, value, detail, action }: InfoTileProps) {
  return (
    <Card className="info-tile" p="md">
      <Group align="center" justify="space-between" wrap="nowrap">
        <Stack gap={6} miw={0}>
          <Text c="dimmed" size="xs">
            {label}
          </Text>
          <Title order={3} size={19}>
            {value}
          </Title>
          {detail && (
            <Text c="dimmed" size="xs" lh={1.45}>
              {detail}
            </Text>
          )}
        </Stack>
        {action}
      </Group>
    </Card>
  );
}
