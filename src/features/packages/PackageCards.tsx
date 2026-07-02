import { ActionIcon, Button, Card, Group, Stack, Switch, Text, Title } from "@mantine/core";
import { PackagePlus, Trash2 } from "lucide-react";
import { GameArt } from "../../components/GameArt";
import { formatManifestTime, packageSubtitle, searchResultSubtitle } from "../../lib/format";
import { canAddManifest, manifestIssueText, manifestStatusText } from "../../lib/hubcap";
import { steamHeaderImage } from "../../lib/steam";
import type { PackageItem, SteamSearchResult } from "../../types";

type SearchResultCardProps = {
  item: SteamSearchResult;
  index: number;
  existingPackage?: PackageItem;
  busy: string | null;
  onAdd: (item: SteamSearchResult) => void;
};

export function SearchResultCard({ item, index, existingPackage, busy, onAdd }: SearchResultCardProps) {
  const canAdd = canAddManifest(item);
  const isAdding = busy === `add-hubcap-${item.id}`;
  const manifestText = manifestStatusText(item);
  const manifestIssue = manifestIssueText(item);

  return (
    <Card className="package-card" p={0}>
      <Card.Section>
        <GameArt primary={steamHeaderImage(item.id)} fallback={item.tinyImage} tone={index} />
      </Card.Section>
      <Group align="flex-start" justify="space-between" gap="md" p="lg" wrap="nowrap">
        <Stack gap={7} miw={0}>
          <Title order={2} className="package-title" size={21}>
            {item.name}
          </Title>
          <Text c="dimmed" size="sm">
            {searchResultSubtitle(item)}
          </Text>
          {manifestText && (
            <Text c="steam.3" size="xs" lh={1.35}>
              {manifestText}
            </Text>
          )}
          {manifestIssue && (
            <Text c="yellow.5" size="xs" lh={1.35}>
              {manifestIssue}
            </Text>
          )}
          {existingPackage?.manifestUpdatedAt && (
            <Text c="dimmed" size="xs" lh={1.35}>
              已添加：{formatManifestTime(existingPackage.manifestUpdatedAt)}
            </Text>
          )}
        </Stack>

        {canAdd && (
          <Button
            miw={86}
            variant="light"
            leftSection={<PackagePlus size={17} />}
            loading={isAdding}
            onClick={() => onAdd(item)}
            disabled={isAdding}
            aria-busy={isAdding}
          >
            {isAdding ? "添加中" : existingPackage ? "重新添加" : "添加"}
          </Button>
        )}
      </Group>
    </Card>
  );
}

type SavedPackageCardProps = {
  pkg: PackageItem;
  index: number;
  busy: string | null;
  onToggle: (pkg: PackageItem, enabled: boolean) => void;
  onDelete: (pkg: PackageItem) => void;
};

export function SavedPackageCard({ pkg, index, busy, onToggle, onDelete }: SavedPackageCardProps) {
  return (
      <Card className="package-card" p={0}>
      <Card.Section>
        <GameArt primary={steamHeaderImage(pkg.appId)} fallback={pkg.imageUrl} tone={index} />
      </Card.Section>
      <Group align="flex-start" justify="space-between" gap="md" p="lg" wrap="nowrap">
        <Stack gap={7} miw={0}>
          <Title order={2} className="package-title" size={21}>
            {pkg.title}
          </Title>
          <Text c="dimmed" size="sm">
            {packageSubtitle(pkg)}
          </Text>
          {pkg.manifestUpdatedAt && (
            <Text c="dimmed" size="xs" lh={1.35}>
              清单更新：{formatManifestTime(pkg.manifestUpdatedAt)}
            </Text>
          )}
        </Stack>

        <Group gap="xs" align="flex-start" wrap="nowrap">
          <Switch
            checked={pkg.enabled}
            thumbIcon={null}
            title={pkg.enabled ? "禁用" : "启用"}
            aria-label={`${pkg.enabled ? "禁用" : "启用"} ${pkg.title}`}
            onChange={(event) => onToggle(pkg, event.currentTarget.checked)}
          />
          <ActionIcon
            color="red"
            variant="subtle"
            aria-label={`删除 ${pkg.title}`}
            title="删除"
            onClick={() => onDelete(pkg)}
            disabled={busy === `delete-${pkg.id}`}
          >
            <Trash2 size={18} />
          </ActionIcon>
        </Group>
      </Group>
    </Card>
  );
}
