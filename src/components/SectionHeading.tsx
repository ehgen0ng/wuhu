import { Group, Text, Title } from "@mantine/core";

type SectionHeadingProps = {
  title: string;
  meta?: string;
};

export function SectionHeading({ title, meta }: SectionHeadingProps) {
  return (
    <Group align="baseline" justify="space-between" mb="sm">
      <Title order={2} size={20}>
        {title}
      </Title>
      {meta && (
        <Text c="dimmed" size="sm">
          {meta}
        </Text>
      )}
    </Group>
  );
}
