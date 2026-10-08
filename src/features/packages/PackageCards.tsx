import { useLanguage } from "../../app/LanguageProvider";
import { t, renderText } from "../../i18n";
import { renderError } from "../../i18n/errors";
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
  const { locale } = useLanguage();
  const canAdd = canAddManifest(item);
  const isAdding = busy === `add-manifest-${item.id}`;
  const manifestText = manifestStatusText(item, locale);
  const manifestIssue = manifestIssueText(item, locale);

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
            {searchResultSubtitle(item, locale)}
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
              {t(locale, "packages.addedLabel")}{formatManifestTime(existingPackage.manifestUpdatedAt, locale)}
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
            {isAdding ? t(locale, "packages.adding") : existingPackage ? t(locale, "packages.reAdd") : t(locale, "common.add")}
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
  const { locale } = useLanguage();
  const isUpdating = busy === `update-manifest-${pkg.id}`;
  const canSyncPackage = hasSteamPath && packageSyncSupported;
  const toggleTitle = packageSyncSupported
    ? hasSteamPath
      ? pkg.enabled
        ? t(locale, "common.disable")
        : t(locale, "common.enable")
      : t(locale, "packages.pathRequired")
    : t(locale, "packages.syncUnsupported");

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
            {packageSubtitle(pkg, locale)}
          </Text>
          {pkg.manifestUpdatedAt && (
            <Text className="package-meta" c="dimmed" size="xs" lh={1.35}>
              {t(locale, "manifest.updatedLabel")}{formatManifestTime(pkg.manifestUpdatedAt, locale)}
            </Text>
          )}
          {!pkg.manifestUpdatedAt && pkg.manifestFiles.length > 0 && (
            <Text className="package-meta" c="dimmed" size="xs" lh={1.35}>
              {t(locale, "manifest.updatedUnknown")}
            </Text>
          )}
          {updateCheck && (
            <Text c={updateCheckColor(updateCheck.kind)} size="xs" lh={1.35}>
              {updateCheck.kind === "error"
                ? renderError(locale, updateCheck.message)
                : renderText(locale, updateCheck.message)}
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
              aria-label={t(locale, "packages.deleteAction", { title: pkg.title })}
              title={t(locale, "common.delete")}
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
              {t(locale, "common.update")}
            </Button>
          )}
        </Stack>
      </Group>
    </Card>
  );
}
