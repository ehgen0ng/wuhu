import { ActionIcon, Button, Card, Group, Stack, Switch, Text, Title } from "@mantine/core";
import { Download, PackagePlus, Trash2 } from "lucide-react";
import { GameArt } from "../../components/GameArt";
import { formatManifestTime, packageSubtitle, searchResultSubtitle } from "../../lib/format";
import { canAddManifest, manifestIssueText, manifestStatusText } from "../../lib/hubcap";
import { steamHeaderImage } from "../../lib/steam";
import type { PackageItem, PackageUpdateCheck, SteamSearchResult } from "../../types";

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
  hasSteamPath: boolean;
  updateCheck?: PackageUpdateCheck;
  onUpdate: (pkg: PackageItem) => void;
  onToggle: (pkg: PackageItem, enabled: boolean) => void;
  onDelete: (pkg: PackageItem) => void;
};

function updateCheckColor(kind: PackageUpdateCheck["kind"]) {
  if (kind === "success") return "green.4";
  if (kind === "warning") return "yellow.5";
  if (kind === "error") return "red.4";
  return "steam.3";
}

export function SavedPackageCard({
  pkg,
  index,
  busy,
  hasSteamPath,
  updateCheck,
  onUpdate,
  onToggle,
  onDelete,
}: SavedPackageCardProps) {
  const isUpdating = busy === `update-hubcap-${pkg.id}`;
  const toggleTitle = hasSteamPath ? (pkg.enabled ? "禁用" : "启用") : "设置 Steam 路径后可启用";

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
          {updateCheck && (
            <Text c={updateCheckColor(updateCheck.kind)} size="xs" lh={1.35}>
              {updateCheck.message}
            </Text>
          )}
        </Stack>

        <Stack gap="sm" align="flex-end" justify="space-between">
          <Group gap="xs" align="center" wrap="nowrap">
            <Switch
              checked={pkg.enabled}
              thumbIcon={null}
              title={toggleTitle}
              aria-label={`${toggleTitle} ${pkg.title}`}
              disabled={!hasSteamPath || busy === `toggle-${pkg.id}`}
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
          {updateCheck?.hasUpdate && (
            <Button
              miw={76}
              variant="light"
              leftSection={<Download size={17} />}
              loading={isUpdating}
              onClick={() => onUpdate(pkg)}
              disabled={isUpdating}
              aria-busy={isUpdating}
            >
              更新
            </Button>
          )}
        </Stack>
      </Group>
    </Card>
  );
}
