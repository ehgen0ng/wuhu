import { Group, Title } from "@mantine/core";
import type { ReactNode } from "react";

type PageHeaderProps = {
  title: string;
  actions?: ReactNode;
};

export function PageHeader({ title, actions }: PageHeaderProps) {
  return (
    <Group align="flex-start" justify="space-between" mb={28} wrap="wrap">
      <Title order={1} size={34} lh={1.12}>
        {title}
      </Title>
      {actions && <Group gap="sm">{actions}</Group>}
    </Group>
  );
}
