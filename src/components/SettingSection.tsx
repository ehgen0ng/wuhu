import { Card, Group, ThemeIcon, Title } from "@mantine/core";
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

type SettingSectionProps = {
  icon: LucideIcon;
  title: string;
  aside?: ReactNode;
  children: ReactNode;
};

export function SettingSection({ icon: Icon, title, aside, children }: SettingSectionProps) {
  return (
    <Card className="settings-section" component="section" mb="lg" p="lg">
      <Group justify="space-between" mb="lg" wrap="nowrap">
        <Group gap="sm" wrap="nowrap">
          <ThemeIcon color="steam" radius="md" variant="subtle">
            <Icon size={20} />
          </ThemeIcon>
          <Title order={2} size={20}>
            {title}
          </Title>
        </Group>
        {aside}
      </Group>
      {children}
    </Card>
  );
}
