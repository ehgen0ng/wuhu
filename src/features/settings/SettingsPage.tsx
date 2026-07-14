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
  const hasSteamPath = Boolean(state?.settings.steamPath);
  const hasSavedHubcapKey = Boolean(state?.settings.hubcapApiKey?.trim());
  const componentInstallSupported = Boolean(state?.installStatus.supported);
  const launchRequired = Boolean(state?.installStatus.launchRequired);
  const launchedViaWuhu = Boolean(state?.installStatus.launchedViaWuhu);
  const steamClientLockSupported = Boolean(state?.steamClient.lockSupported);
  const componentStatus = componentInstallSupported
    ? launchRequired
      ? launchedViaWuhu
        ? "Steam 已通过 wuhu 启动"
        : state?.installStatus.installed
          ? "Steam 未通过 wuhu 启动"
          : "首次启动时自动准备"
      : state?.installStatus.installed
        ? "已安装"
        : "未安装"
    : "不支持当前系统";

  return (
    <section className="page settings-page">
      <PageHeader title="设置" />

      {notice?.page === "settings" && <NoticeAlert notice={notice} />}

      <SettingSection icon={FolderCog} title="Steam 路径">
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <TextInput
            value={steamPathInput}
            onChange={(event) => onSteamPathChange(event.currentTarget.value)}
            placeholder="Steam 根目录或 Steam.app"
            className="grow-control"
          />
          <Button
            variant="light"
            leftSection={<RefreshCcw size={17} />}
            onClick={onDetectSteamPath}
          >
            自动读取
          </Button>
          <Button variant="light" leftSection={<FolderOpen size={17} />} onClick={onChooseSteamPath}>
            选择目录
          </Button>
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            onClick={onSaveSteamPath}
          >
            保存
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
            aria-label="刷新额度"
            title="刷新额度"
          >
            <RefreshCcw size={17} />
          </ActionIcon>
          <Button
            color="steam"
            variant="filled"
            c="#06121e"
            onClick={onSaveHubcapKey}
          >
            保存
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={KeyRound} title="DepotBox Key">
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <PasswordInput
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
            保存
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={Wrench} title={launchRequired ? "Steam 启动" : "组件安装"}>
        <InfoTile
          label="当前状态"
          value={componentStatus}
          detail={launchRequired ? "macOS 需要通过 wuhu 启动 Steam 才会加载组件" : undefined}
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
            {launchRequired ? "启动 Steam" : "安装"}
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
            恢复
          </Button>
        </Group>
      </SettingSection>

      <SettingSection icon={LockKeyhole} title="Steam 客户端版本">
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <InfoTile
            label="Steam 版本"
            value={formatSteamVersion(state?.steamClient.version)}
            detail={`客户端生成日期：${formatSteamBuildDate(state?.steamClient.clientBuildDate)}`}
          />
          <InfoTile
            label="锁定版本"
            value={state?.steamClient.locked ? "已锁定" : "未锁定"}
            action={
              <Switch
                checked={Boolean(state?.steamClient.locked)}
                disabled={!steamClientLockSupported || !hasSteamPath}
                thumbIcon={null}
                title={state?.steamClient.locked ? "取消锁定" : "锁定"}
                aria-label={state?.steamClient.locked ? "取消锁定 Steam 客户端版本" : "锁定 Steam 客户端版本"}
                onChange={(event) => onToggleSteamClientLock(event.currentTarget.checked)}
              />
            }
          />
        </SimpleGrid>
      </SettingSection>

      <SettingSection icon={Info} title="当前版本">
        <InfoTile
          label="wuhu"
          value={`v${appVersion}`}
          detail={latestRelease ? `最新版本：v${latestRelease.version}` : undefined}
          action={
            <Group gap="sm" wrap="nowrap">
              {latestRelease && (
                <span
                  className="release-update-dot"
                  aria-label={`最新版本：v${latestRelease.version}`}
                  title={`最新版本：v${latestRelease.version}`}
                />
              )}
              <ActionIcon
                color="steam"
                variant="light"
                onClick={onCheckLatestRelease}
                disabled={releaseCheckBusy}
                aria-busy={releaseCheckBusy}
                aria-label="检查最新版本"
                title="检查最新版本"
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
