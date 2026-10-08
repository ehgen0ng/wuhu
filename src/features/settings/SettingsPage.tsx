import { useLanguage } from "../../app/LanguageProvider";
import { t } from "../../i18n";
import {
  ActionIcon,
  Badge,
  Button,
  Group,
  Loader,
  PasswordInput,
  SimpleGrid,
  Switch,
  TextInput,
} from "@mantine/core";
import {
  CheckCircle2,
  FolderCog,
  FolderOpen,
  Info,
  KeyRound,
  Languages,
  LockKeyhole,
  Play,
  RefreshCcw,
  Wrench,
} from "lucide-react";
import { NoticeAlert } from "../../components/NoticeAlert";
import { PageHeader } from "../../components/PageHeader";
import { SettingSection } from "../../components/SettingSection";
import { formatHubcapQuota, formatSteamBuildDate, formatSteamVersion } from "../../domain/display";
import type { AppRelease, AppState, HubcapQuota, Notice } from "../../types";
import { InfoTile } from "./InfoTile";

type SettingsPageProps = {
  appVersion: string;
  latestRelease: AppRelease | null;
  releaseCheckBusy: boolean;
  notice: Notice | null;
  state: AppState | null;
  steamPathInput: string;
  hubcapKeyInput: string;
  depotboxKeyInput: string;
  hubcapQuota: HubcapQuota | null;
  onSteamPathChange: (value: string) => void;
  onHubcapKeyChange: (value: string) => void;
  onDepotboxKeyChange: (value: string) => void;
  onSaveSteamPath: () => void;
  onDetectSteamPath: () => void;
  onChooseSteamPath: () => void;
  onSaveHubcapKey: () => void;
  onSaveDepotboxKey: () => void;
  onRefreshHubcapQuota: () => void;
  onCheckLatestRelease: () => void;
  onInstallOpenSteamTool: () => void;
  onLaunchSteamWithOpenSteamTool: () => void;
  onRestoreOpenSteamTool: () => void;
  onToggleSteamClientLock: (locked: boolean) => void;
};

export function SettingsPage({
  appVersion,
  latestRelease,
  releaseCheckBusy,
  notice,
  state,
  steamPathInput,
  hubcapKeyInput,
  depotboxKeyInput,
  hubcapQuota,
  onSteamPathChange,
  onHubcapKeyChange,
  onDepotboxKeyChange,
  onSaveSteamPath,
  onDetectSteamPath,
  onChooseSteamPath,
  onSaveHubcapKey,
  onSaveDepotboxKey,
  onRefreshHubcapQuota,
  onCheckLatestRelease,
  onInstallOpenSteamTool,
  onLaunchSteamWithOpenSteamTool,
  onRestoreOpenSteamTool,
  onToggleSteamClientLock,
}: SettingsPageProps) {
  const { locale, setLocale } = useLanguage();
  const hasSteamPath = Boolean(state?.settings.steamPath);
  const hasSavedHubcapKey = Boolean(state?.settings.hubcapApiKey?.trim());
  const componentInstallSupported = Boolean(state?.installStatus.supported);
  const launchRequired = Boolean(state?.installStatus.launchRequired);
  const launchedViaWuhu = Boolean(state?.installStatus.launchedViaWuhu);
  const updateAvailable = Boolean(state?.installStatus.updateAvailable);
  const steamClientLockSupported = Boolean(state?.steamClient.lockSupported);
  const componentStatus = componentInstallSupported
    ? launchRequired
      ? launchedViaWuhu
        ? t(locale, "steam.launched")
        : state?.installStatus.installed
          ? t(locale, "steam.notLaunched")
          : t(locale, "component.prepareOnLaunch")
      : state?.installStatus.installed
        ? updateAvailable
          ? t(locale, "common.updateAvailable")
          : t(locale, "common.installed")
        : t(locale, "common.notInstalled")
    : t(locale, "common.unsupported");
  const componentDetail = launchRequired
    ? t(locale, "steam.macLaunchHint")
    : updateAvailable
      ? t(locale, "component.updateHint")
      : undefined;

  return (
    <section className="page settings-page">
      <PageHeader
        title={t(locale, "nav.settings")}
        actions={
          <ActionIcon
            color="gray"
            variant="subtle"
            aria-label={t(locale, locale === "zh-CN" ? "language.switchToEnglish" : "language.switchToChinese")}
            title={t(locale, locale === "zh-CN" ? "language.switchToEnglish" : "language.switchToChinese")}
            onClick={() => setLocale(locale === "zh-CN" ? "en" : "zh-CN")}
          >
            <Languages size={22} strokeWidth={1.8} aria-hidden="true" />
          </ActionIcon>
        }
      />

      {notice?.page === "settings" && <NoticeAlert notice={notice} />}

      <SettingSection icon={FolderCog} title={t(locale, "steam.path")}>
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <TextInput
            aria-label={t(locale, "steam.path")}
            value={steamPathInput}
            onChange={(event) => onSteamPathChange(event.currentTarget.value)}
            placeholder={t(locale, "steam.pathPlaceholder")}
            className="grow-control"
          />
          <Button
            variant="light"
            leftSection={<RefreshCcw size={17} />}
            onClick={onDetectSteamPath}
          >
            {t(locale, "steam.detect")}
          </Button>
          <Button variant="light" leftSection={<FolderOpen size={17} />} onClick={onChooseSteamPath}>
            {t(locale, "steam.browse")}
          </Button>
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            onClick={onSaveSteamPath}
          >
            {t(locale, "common.save")}
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={Wrench} title={launchRequired ? t(locale, "steam.launchSection") : t(locale, "component.installSection")}>
        <InfoTile
          label={t(locale, "common.currentStatus")}
          value={componentStatus}
          detail={componentDetail}
        />

        <Group mt="md" gap="sm">
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            leftSection={launchRequired ? <Play size={18} /> : <CheckCircle2 size={18} />}
            onClick={launchRequired ? onLaunchSteamWithOpenSteamTool : onInstallOpenSteamTool}
            disabled={
              !componentInstallSupported || !hasSteamPath || (launchRequired && launchedViaWuhu)
            }
          >
            {launchRequired ? t(locale, "steam.launch") : updateAvailable ? t(locale, "common.update") : t(locale, "common.install")}
          </Button>
          <Button
            color="red"
            variant="subtle"
            onClick={onRestoreOpenSteamTool}
            disabled={
              !componentInstallSupported ||
              !state?.installStatus.installed ||
              (launchRequired && launchedViaWuhu)
            }
          >
            {t(locale, "common.restore")}
          </Button>
        </Group>
      </SettingSection>

      <SettingSection
        icon={KeyRound}
        title="Hubcap Key"
        aside={
          <Badge className="quota-badge" color="steam" size="lg" variant="subtle">
            {formatHubcapQuota(hubcapQuota)}
          </Badge>
        }
      >
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <PasswordInput
            aria-label="Hubcap Key"
            visibilityToggleButtonProps={{ "aria-label": t(locale, "settings.toggleKeyVisibility") }}
            value={hubcapKeyInput}
            onChange={(event) => onHubcapKeyChange(event.currentTarget.value)}
            placeholder="Key"
            autoComplete="off"
            className="grow-control"
          />
          <ActionIcon
            color="steam"
            variant="light"
            onClick={onRefreshHubcapQuota}
            disabled={!hasSavedHubcapKey}
            aria-label={t(locale, "settings.refreshQuota")}
            title={t(locale, "settings.refreshQuota")}
          >
            <RefreshCcw size={17} />
          </ActionIcon>
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            onClick={onSaveHubcapKey}
          >
            {t(locale, "common.save")}
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={KeyRound} title="DepotBox Key">
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <PasswordInput
            aria-label="DepotBox Key"
            visibilityToggleButtonProps={{ "aria-label": t(locale, "settings.toggleKeyVisibility") }}
            value={depotboxKeyInput}
            onChange={(event) => onDepotboxKeyChange(event.currentTarget.value)}
            placeholder="Key"
            autoComplete="off"
            className="grow-control"
          />
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            onClick={onSaveDepotboxKey}
          >
            {t(locale, "common.save")}
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={LockKeyhole} title={t(locale, "steam.clientVersion")}>
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <InfoTile
            label={t(locale, "steam.version")}
            value={formatSteamVersion(state?.steamClient.version, locale)}
            detail={t(locale, "steam.buildDate", { date: formatSteamBuildDate(state?.steamClient.clientBuildDate, locale) })}
          />
          <InfoTile
            label={t(locale, "steam.lockVersion")}
            value={state?.steamClient.locked ? t(locale, "common.locked") : t(locale, "common.unlocked")}
            action={
              <Switch
                checked={Boolean(state?.steamClient.locked)}
                disabled={!steamClientLockSupported || !hasSteamPath}
                thumbIcon={null}
                title={state?.steamClient.locked ? t(locale, "common.unlock") : t(locale, "common.lock")}
                aria-label={state?.steamClient.locked ? t(locale, "steam.unlockAction") : t(locale, "steam.lockAction")}
                onChange={(event) => onToggleSteamClientLock(event.currentTarget.checked)}
              />
            }
          />
        </SimpleGrid>
      </SettingSection>

      <SettingSection icon={Info} title={t(locale, "settings.appVersion")}>
        <InfoTile
          label="wuhu"
          value={`v${appVersion}`}
          detail={latestRelease ? t(locale, "release.latest", { version: latestRelease.version }) : undefined}
          action={
            <Group gap="sm" wrap="nowrap">
              {latestRelease && (
                <span
                  className="release-update-dot"
                  aria-label={t(locale, "release.latest", { version: latestRelease.version })}
                  title={t(locale, "release.latest", { version: latestRelease.version })}
                />
              )}
              <ActionIcon
                color="steam"
                variant="light"
                onClick={onCheckLatestRelease}
                disabled={releaseCheckBusy}
                aria-busy={releaseCheckBusy}
                aria-label={t(locale, "release.check")}
                title={t(locale, "release.check")}
              >
                {releaseCheckBusy ? <Loader color="steam" size={17} /> : <RefreshCcw size={17} />}
              </ActionIcon>
            </Group>
          }
        />
      </SettingSection>
    </section>
  );
}
