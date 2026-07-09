import { ActionIcon, Button, Card, Group, Stack, Switch, Text, Title } from "@mantine/core";
import { Download, PackagePlus, Trash2 } from "lucide-react";
import { GameArt } from "../../components/GameArt";
import { formatManifestTime, packageSubtitle, searchResultSubtitle } from "../../domain/display";
import { canAddManifest } from "../../domain/manifest";
import { manifestIssueText, manifestStatusText } from "../../domain/manifestText";
import { steamHeaderImage } from "../../domain/packageMetadata";
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
  const isAdding = busy === `add-manifest-${item.id}`;
  const manifestText = manifestStatusText(item);
  const manifestIssue = manifestIssueText(item);

  return (
    <Card className="package-card" p={0}>
      <Card.Section>
        <GameArt primary={steamHeaderImage(item.id)} fallback={item.tinyImage} tone={index} />
      </Card.Section>
      <Group
        className={`package-card__body${canAdd ? " package-card__body--with-actions" : ""}`}
        align="flex-start"
        justify="space-between"
        gap="md"
        p="lg"
        wrap="nowrap"
      >
        <Stack className="package-card__content" gap={7} miw={0}>
          <Title order={2} className="package-title" size={21}>
            {item.name}
          </Title>
          <Text className="package-meta" c="dimmed" size="sm">
            {searchResultSubtitle(item)}
          </Text>
          {manifestText && (
            <Text className="package-meta" c="steam.3" size="xs" lh={1.35}>
              {manifestText}
            </Text>
          )}
          {manifestIssue && (
            <Text c="yellow.5" size="xs" lh={1.35}>
              {manifestIssue}
            </Text>
          )}
          {existingPackage?.manifestUpdatedAt && (
            <Text className="package-meta" c="dimmed" size="xs" lh={1.35}>
              已添加：{formatManifestTime(existingPackage.manifestUpdatedAt)}
            </Text>
          )}
        </Stack>

        {canAdd && (
          <Button
            className="package-card__actions"
            miw={existingPackage ? 112 : 86}
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
  packageSyncSupported: boolean;
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
  packageSyncSupported,
  updateCheck,
  onUpdate,
  onToggle,
  onDelete,
}: SavedPackageCardProps) {
  const isUpdating = busy === `update-manifest-${pkg.id}`;
  const canSyncPackage = hasSteamPath && packageSyncSupported;
  const toggleTitle = packageSyncSupported
    ? hasSteamPath
      ? pkg.enabled
        ? "禁用"
        : "启用"
      : "设置 Steam 路径后可启用"
    : "清单启用目前只支持 Windows";

  return (
    <Card className="package-card" p={0}>
      <Card.Section>
        <GameArt primary={steamHeaderImage(pkg.appId)} fallback={pkg.imageUrl} tone={index} />
      </Card.Section>
      <Group
        className="package-card__body package-card__body--with-actions"
        align="flex-start"
        justify="space-between"
        gap="md"
        p="lg"
        wrap="nowrap"
      >
        <Stack className="package-card__content" gap={7} miw={0}>
          <Title order={2} className="package-title" size={21}>
            {pkg.title}
          </Title>
          <Text className="package-meta" c="dimmed" size="sm">
            {packageSubtitle(pkg)}
          </Text>
          {pkg.manifestUpdatedAt && (
            <Text className="package-meta" c="dimmed" size="xs" lh={1.35}>
              清单更新：{formatManifestTime(pkg.manifestUpdatedAt)}
            </Text>
          )}
          {!pkg.manifestUpdatedAt && pkg.manifestFiles.length > 0 && (
            <Text className="package-meta" c="dimmed" size="xs" lh={1.35}>
              清单更新：未知
            </Text>
          )}
          {updateCheck && (
            <Text c={updateCheckColor(updateCheck.kind)} size="xs" lh={1.35}>
              {updateCheck.message}
            </Text>
          )}
        </Stack>

        <Stack className="package-card__actions" gap="sm" align="flex-end" justify="space-between">
          <Group gap="xs" align="center" wrap="nowrap">
            <Switch
              checked={pkg.enabled}
              thumbIcon={null}
              title={toggleTitle}
              aria-label={`${toggleTitle} ${pkg.title}`}
              disabled={!canSyncPackage || busy === `toggle-${pkg.id}`}
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
