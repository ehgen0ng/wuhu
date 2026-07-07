import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import {
  addRemoteManifest,
  deletePackage as deletePackageCommand,
  detectSteamPath as detectSteamPathCommand,
  getHubcapQuota,
  getInitialState,
  getLatestAppRelease,
  importPackageFromBytes,
  installOpenSteamTool,
  restoreOpenSteamTool,
  setDepotboxApiKey,
  setHubcapApiKey,
  setPackageEnabled as setPackageEnabledCommand,
  setSteamClientVersionLocked,
  setSteamPath as setSteamPathCommand,
  updateRemoteManifest,
} from "../api/commands";
import { buildPackageUpdateCheck, canAddManifest } from "../domain/manifest";
import { enrichPackageMetadata } from "../domain/packageMetadata";
import { PackagesPage } from "../features/packages/PackagesPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { createGameSearchSources } from "../integrations/gameSearch";
import { fetchPreferredManifestStatuses, hasConfiguredManifestSource } from "../integrations/manifests";
import { arrayBufferToBase64 } from "../lib/file";
import { wait, waitForNextPaint } from "../lib/render";
import type {
  AppState,
  HubcapQuota,
  AppRelease,
  Notice,
  PackageUpdateCheck,
  Page,
  PackageItem,
  SteamSearchResult,
} from "../types";
import { AppLayout } from "./AppLayout";
import { APP_VERSION, isVersionNewer, waitForTauriRuntime } from "./runtime";

export default function App() {
  const [page, setPage] = useState<Page>("packages");
  const [state, setState] = useState<AppState | null>(null);
  const [steamPathInput, setSteamPathInput] = useState("");
  const [hubcapKeyInput, setHubcapKeyInput] = useState("");
  const [depotboxKeyInput, setDepotboxKeyInput] = useState("");
  const [hubcapQuota, setHubcapQuota] = useState<HubcapQuota | null>(null);
  const [latestRelease, setLatestRelease] = useState<AppRelease | null>(null);
  const [releaseCheckBusy, setReleaseCheckBusy] = useState(false);
  const [packageUpdateChecks, setPackageUpdateChecks] = useState<Record<string, PackageUpdateCheck>>({});
  const [searchTerm, setSearchTerm] = useState("");
  const [searchResults, setSearchResults] = useState<SteamSearchResult[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const busyRef = useRef<string | null>(null);
  const searchRunId = useRef(0);
  const stateApplyVersion = useRef(0);

  const packages = state?.packages ?? [];
  const hasLoadedState = state !== null;
  const manifestSettings = state?.settings ?? {
    steamPath: steamPathInput || null,
    hubcapApiKey: hubcapKeyInput || null,
    depotboxApiKey: depotboxKeyInput || null,
  };
  const appUpdateRelease =
    latestRelease && isVersionNewer(latestRelease.version, APP_VERSION) ? latestRelease : null;

  async function applyAppState(nextState: AppState) {
    const applyVersion = stateApplyVersion.current + 1;
    stateApplyVersion.current = applyVersion;
    setState(nextState);
    setSteamPathInput(nextState.settings.steamPath ?? "");
    setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
    setDepotboxKeyInput(nextState.settings.depotboxApiKey ?? "");

    const enrichedState = await enrichPackageMetadata(nextState);
    if (stateApplyVersion.current !== applyVersion) return;
    setState(enrichedState);
    setSteamPathInput(enrichedState.settings.steamPath ?? "");
    setHubcapKeyInput(enrichedState.settings.hubcapApiKey ?? "");
    setDepotboxKeyInput(enrichedState.settings.depotboxApiKey ?? "");
  }

  async function checkLatestRelease(showResult = false) {
    if (releaseCheckBusy) return;

    setReleaseCheckBusy(true);
    try {
      const release = await getLatestAppRelease();
      const hasUpdate = isVersionNewer(release.version, APP_VERSION);
      setLatestRelease(hasUpdate ? release : null);
      if (showResult) {
        setNotice({
          page: "settings",
          text: hasUpdate ? `发现最新版本：v${release.version}。` : "已是最新版本。",
          kind: hasUpdate ? "warning" : "success",
        });
      }
    } catch (error) {
      console.info("[wuhu] latest release check skipped", error);
      setLatestRelease(null);
      if (showResult) {
        setNotice({ page: "settings", text: "暂时没有检查到新版本。", kind: "info" });
      }
    } finally {
      setReleaseCheckBusy(false);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function loadInitialState() {
      const hasTauriRuntime = await waitForTauriRuntime();
      if (cancelled) return;

      if (!hasTauriRuntime) {
        console.warn("[wuhu] Tauri runtime was not detected before get_initial_state.");
      }

      try {
        const nextState = await getInitialState();
        if (!cancelled) {
          await applyAppState(nextState);
        }
      } catch (error) {
        if (cancelled) return;
        console.error("[wuhu] get_initial_state failed", error);
        setNotice({ page: "packages", text: String(error), kind: "error" });
      }
    }

    void loadInitialState();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    void checkLatestRelease();
  }, []);

  function switchPage(nextPage: Page) {
    setPage(nextPage);
    setNotice(null);
  }

  function clearSearchState() {
    searchRunId.current += 1;
    setIsSearching(false);
    setSearchTerm("");
    setSearchResults([]);
    setHasSearched(false);
  }

  function beginAction(label: string, visual = false) {
    if (busyRef.current) return false;
    busyRef.current = label;
    if (visual) setBusy(label);
    return true;
  }

  function endAction(label: string, visual = false) {
    if (busyRef.current === label) {
      busyRef.current = null;
    }
    if (visual) setBusy(null);
  }

  async function refreshState() {
    const label = "refresh";
    if (!beginAction(label, true)) return;
    try {
      setNotice(null);
      const [nextState] = await Promise.all([getInitialState(), wait(320)]);
      await applyAppState(nextState);
      clearSearchState();
      setPackageUpdateChecks({});
    } catch (error) {
      setNotice({ page: "packages", text: String(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function runAction(
    label: string,
    noticePage: Page,
    action: () => Promise<AppState | void>,
    success?: string | ((state: AppState | void) => string),
    pending?: string,
  ) {
    if (!beginAction(label)) return;
    try {
      if (pending) {
        setNotice({ page: noticePage, text: pending, kind: "info" });
        await waitForNextPaint();
      } else {
        setNotice(null);
      }
      const nextState = await action();
      if (nextState) {
        await applyAppState(nextState);
      }
      if (success) {
        const successText = typeof success === "function" ? success(nextState) : success;
        setNotice({ page: noticePage, text: successText, kind: "success" });
      }
    } catch (error) {
      setNotice({ page: noticePage, text: String(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function handleImportFile(file: File | null) {
    if (!file) return;

    await runAction(
      "import",
      "packages",
      async () => {
        const dataBase64 = arrayBufferToBase64(await file.arrayBuffer());
        return importPackageFromBytes(file.name, dataBase64);
      },
      (nextState) =>
        nextState?.settings.steamPath
          ? "已导入清单。"
          : "已导入清单，已保存到本地；设置 Steam 路径后可启用。",
    );
    setPackageUpdateChecks({});
  }

  async function searchSteamGames(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = searchTerm.trim();
    if (!query) {
      setNotice({ page: "packages", text: "请输入游戏名称。", kind: "warning" });
      return;
    }

    const runId = searchRunId.current + 1;
    searchRunId.current = runId;
    const sources = createGameSearchSources();
    const seenAppIds = new Set<number>();
    let failedSourceCount = 0;
    let resultCount = 0;

    setIsSearching(true);
    setNotice(null);
    setHasSearched(true);
    setSearchResults([]);

    const hasManifestKey = hasConfiguredManifestSource(manifestSettings);

    await Promise.all(
      sources.map(async (source) => {
        try {
          const results = await source.search(query);
          if (searchRunId.current !== runId) return;

          const newResults = appendSearchSourceResults(results, seenAppIds, hasManifestKey);
          resultCount += newResults.length;

          if (newResults.length && hasManifestKey) {
            void checkSearchResultManifestStatuses(newResults, runId);
          }
        } catch (error) {
          console.warn(`[wuhu] search failed: ${source.id}`, error);
          failedSourceCount += 1;
        }
      }),
    );

    if (searchRunId.current !== runId) return;

    setIsSearching(false);
    if (resultCount === 0) {
      setNotice({
        page: "packages",
        text: failedSourceCount === sources.length ? "搜索失败，请稍后重试。" : "没有搜索结果。",
        kind: failedSourceCount === sources.length ? "error" : "info",
      });
    }
  }

  function appendSearchSourceResults(
    results: SteamSearchResult[],
    seenAppIds: Set<number>,
    checkManifests: boolean,
  ) {
    const newResults = results
      .filter((item) => {
        if (!item.id || seenAppIds.has(item.id)) return false;
        seenAppIds.add(item.id);
        return true;
      })
      .map((item) => ({
        ...item,
        manifestChecking: checkManifests,
        manifestChecked: false,
        manifestStatus: null,
      }));

    if (newResults.length) {
      setSearchResults((current) => [...current, ...newResults]);
    }

    return newResults;
  }

  async function checkSearchResultManifestStatuses(results: SteamSearchResult[], runId: number) {
    try {
      const statuses = await fetchPreferredManifestStatuses(results.map((item) => item.id), manifestSettings);
      if (searchRunId.current !== runId) return;

      const byAppId = new Map(statuses.map((status) => [status.appId, status]));
      const checkedAppIds = new Set(results.map((item) => item.id));

      setSearchResults((current) =>
        current.map((item) => {
          if (!checkedAppIds.has(item.id)) return item;
          return {
            ...item,
            manifestChecking: false,
            manifestChecked: true,
            manifestStatus: byAppId.get(item.id) ?? null,
          };
        }),
      );
    } catch (error) {
      if (searchRunId.current !== runId) return;

      console.warn("[wuhu] search result manifest status check failed", error);
      const checkedAppIds = new Set(results.map((item) => item.id));
      setSearchResults((current) =>
        current.map((item) =>
          checkedAppIds.has(item.id)
            ? {
              ...item,
              manifestChecking: false,
              manifestChecked: true,
              manifestStatus: null,
            }
            : item,
        ),
      );
    }
  }

  async function addSearchResult(item: SteamSearchResult) {
    const label = `add-manifest-${item.id}`;
    if (!beginAction(label, true)) return;
    try {
      setNotice({ page: "packages", text: `正在添加 ${item.name}，请稍候。`, kind: "info" });

      if (!canAddManifest(item)) {
        throw new Error("当前没有可用清单。");
      }

      await waitForNextPaint();

      const nextState = await addRemoteManifest(item.id, item.name, item.tinyImage);
      await applyAppState(nextState);
      setPackageUpdateChecks({});

      setNotice({
        page: "packages",
        text: nextState.settings.steamPath
          ? `已添加 ${item.name}。`
          : `已添加 ${item.name}，已保存到本地；设置 Steam 路径后可启用。`,
        kind: "success",
      });
    } catch (error) {
      setNotice({ page: "packages", text: String(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function togglePackage(pkg: PackageItem, enabled: boolean) {
    setState((current) =>
      current
        ? {
          ...current,
          packages: current.packages.map((item) => (item.id === pkg.id ? { ...item, enabled } : item)),
        }
        : current,
    );

    await runAction(`toggle-${pkg.id}`, "packages", () => setPackageEnabledCommand(pkg.id, enabled));
  }

  async function deletePackage(pkg: PackageItem) {
    const confirmed = window.confirm(
      `确定删除「${pkg.title}」吗？\n\n会删除本地 data 里的清单；已配置 Steam 路径时，也会移除 Steam 中启用的 Lua 和 manifest 副本。`,
    );
    if (!confirmed) return;

    await runAction(`delete-${pkg.id}`, "packages", () => deletePackageCommand(pkg.id), "已删除清单。");
    setPackageUpdateChecks((current) => {
      const next = { ...current };
      delete next[pkg.id];
      return next;
    });
  }

  async function updatePackage(pkg: PackageItem) {
    const label = `update-manifest-${pkg.id}`;
    if (!beginAction(label, true)) return;

    try {
      setNotice({ page: "packages", text: `正在更新 ${pkg.title}，请稍候。`, kind: "info" });
      await waitForNextPaint();

      const nextState = await updateRemoteManifest(pkg.id);
      await applyAppState(nextState);
      setPackageUpdateChecks((current) => {
        const next = { ...current };
        delete next[pkg.id];
        return next;
      });

      setNotice({ page: "packages", text: `已更新 ${pkg.title}。`, kind: "success" });
    } catch (error) {
      setNotice({ page: "packages", text: String(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function checkPackageUpdates() {
    const label = "check-package-updates";
    if (!beginAction(label, true)) return;

    try {
      const checkablePackages = packages.filter((pkg) => typeof pkg.appId === "number" && pkg.appId > 0);
      const skippedCount = packages.length - checkablePackages.length;

      if (!checkablePackages.length) {
        setNotice({ page: "packages", text: "没有可检查的清单：需要先识别 AppID。", kind: "warning" });
        return;
      }

      if (!state || !hasConfiguredManifestSource(state.settings)) {
        setNotice({ page: "packages", text: "请先在设置里保存 Key，才能检查清单。", kind: "warning" });
        return;
      }

      setNotice({ page: "packages", text: "正在检查清单可用性，不会下载文件。", kind: "info" });
      await waitForNextPaint();

      const statuses = await fetchPreferredManifestStatuses(checkablePackages.map((pkg) => pkg.appId ?? 0), state.settings);
      const byAppId = new Map(statuses.map((status) => [status.appId, status]));
      const nextChecks: Record<string, PackageUpdateCheck> = {};
      for (const pkg of checkablePackages) {
        nextChecks[pkg.id] = buildPackageUpdateCheck(pkg, byAppId.get(pkg.appId ?? 0) ?? null);
      }

      setPackageUpdateChecks((current) => ({ ...current, ...nextChecks }));

      const updatedPackages = checkablePackages.filter((pkg) => nextChecks[pkg.id]?.hasUpdate);
      const unknownTimeCount = checkablePackages.filter((pkg) => {
        const status = nextChecks[pkg.id]?.status;
        return status?.available && !status.fileModified;
      }).length;
      const skippedText = skippedCount ? `，另有 ${skippedCount} 个未识别 AppID 的清单已跳过` : "";
      if (updatedPackages.length) {
        const examples = updatedPackages
          .slice(0, 3)
          .map((pkg) => pkg.title)
          .join("、");
        const suffix = updatedPackages.length > 3 ? " 等" : "";
        const unknownText = unknownTimeCount ? `，另有 ${unknownTimeCount} 个清单更新时间未知` : "";
        setNotice({
          page: "packages",
          text: `发现 ${updatedPackages.length} 个清单有更新：${examples}${suffix}${unknownText}${skippedText}。`,
          kind: "warning",
        });
      } else {
        const checkedText = unknownTimeCount
          ? `检查完成，已更新可用性状态；${unknownTimeCount} 个清单更新时间未知${skippedText}。`
          : `检查完成，没有发现可用更新${skippedText}。`;
        setNotice({
          page: "packages",
          text: checkedText,
          kind: "success",
        });
      }
    } catch (error) {
      setNotice({ page: "packages", text: String(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function saveSteamPath() {
    await runAction(
      "steam-path",
      "settings",
      () => setSteamPathCommand(steamPathInput.trim()),
      "Steam 路径已保存，请重新启用需要的清单。",
    );
  }

  async function saveHubcapKey() {
    const label = "hubcap-key";
    if (!beginAction(label)) return;
    try {
      setNotice(null);

      const nextState = await setHubcapApiKey(hubcapKeyInput.trim());
      await applyAppState(nextState);

      if (hubcapKeyInput.trim()) {
        setHubcapQuota(await getHubcapQuota());
      } else {
        setHubcapQuota(null);
      }

      setNotice({ page: "settings", text: "Key 已保存。", kind: "success" });
    } catch (error) {
      setHubcapQuota(null);
      setNotice({ page: "settings", text: String(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function saveDepotboxKey() {
    const label = "depotbox-key";
    if (!beginAction(label)) return;
    try {
      setNotice(null);

      const nextState = await setDepotboxApiKey(depotboxKeyInput.trim());
      await applyAppState(nextState);
      setPackageUpdateChecks({});

      setNotice({ page: "settings", text: "Key 已保存。", kind: "success" });
    } catch (error) {
      setNotice({ page: "settings", text: String(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function refreshHubcapQuota() {
    const label = "hubcap-quota";
    if (!beginAction(label)) return;
    try {
      setNotice(null);
      setHubcapQuota(await getHubcapQuota());
    } catch (error) {
      setHubcapQuota(null);
      setNotice({ page: "settings", text: String(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function detectSteamPath() {
    const label = "detect-steam";
    if (!beginAction(label)) return;

    try {
      const path = await detectSteamPathCommand();
      if (!path) throw new Error("没有自动检测到 Steam 路径，可以手动填写 Steam 根目录。");

      setSteamPathInput(path);
      if (path === state?.settings.steamPath) {
        setNotice({ page: "settings", text: "Steam 路径已是最新。", kind: "success" });
        return;
      }

      const nextState = await setSteamPathCommand(path);
      await applyAppState(nextState);
      setNotice({ page: "settings", text: "已自动检测并保存 Steam 路径，请重新启用需要的清单。", kind: "success" });
    } catch (error) {
      setNotice({ page: "settings", text: String(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function chooseSteamPath() {
    try {
      const selected = await open({
        title: "选择 Steam 根目录",
        directory: true,
        multiple: false,
      });
      const selectedPath = Array.isArray(selected) ? selected[0] : selected;
      if (!selectedPath) return;

      setSteamPathInput(selectedPath);
      await runAction(
        "steam-path",
        "settings",
        () => setSteamPathCommand(selectedPath),
        "Steam 路径已保存，请重新启用需要的清单。",
      );
    } catch (error) {
      setNotice({ page: "settings", text: String(error), kind: "error" });
    }
  }

  async function toggleSteamClientLock(locked: boolean) {
    await runAction(
      "steam-client-lock",
      "settings",
      () => setSteamClientVersionLocked(locked),
      locked ? "已锁定 Steam 客户端版本。" : "已取消锁定 Steam 客户端版本。",
    );
  }

  return (
    <AppLayout
      page={page}
      installed={Boolean(state?.installStatus.installed)}
      hasLoadedState={hasLoadedState}
      onPageChange={switchPage}
    >
      {page === "packages" ? (
        <PackagesPage
          notice={notice}
          packages={packages}
          packageUpdateChecks={packageUpdateChecks}
          searchResults={searchResults}
          searchTerm={searchTerm}
          hasSearched={hasSearched}
          hasLoadedState={hasLoadedState}
          hasSteamPath={Boolean(state?.settings.steamPath)}
          busy={busy}
          isSearching={isSearching}
          onRefresh={refreshState}
          onCheckPackageUpdates={checkPackageUpdates}
          onImportFile={handleImportFile}
          onSearch={searchSteamGames}
          onSearchTermChange={setSearchTerm}
          onAddSearchResult={addSearchResult}
          onUpdatePackage={updatePackage}
          onTogglePackage={togglePackage}
          onDeletePackage={deletePackage}
        />
      ) : (
        <SettingsPage
          appVersion={APP_VERSION}
          latestRelease={appUpdateRelease}
          releaseCheckBusy={releaseCheckBusy}
          notice={notice}
          state={state}
          steamPathInput={steamPathInput}
          hubcapKeyInput={hubcapKeyInput}
          depotboxKeyInput={depotboxKeyInput}
          hubcapQuota={hubcapQuota}
          onSteamPathChange={setSteamPathInput}
          onHubcapKeyChange={(value) => {
            setHubcapKeyInput(value);
            setHubcapQuota(null);
          }}
          onDepotboxKeyChange={setDepotboxKeyInput}
          onSaveSteamPath={saveSteamPath}
          onDetectSteamPath={detectSteamPath}
          onChooseSteamPath={chooseSteamPath}
          onSaveHubcapKey={saveHubcapKey}
          onSaveDepotboxKey={saveDepotboxKey}
          onRefreshHubcapQuota={refreshHubcapQuota}
          onCheckLatestRelease={() => checkLatestRelease(true)}
          onInstallOpenSteamTool={() =>
            runAction(
              "install",
              "settings",
              () => installOpenSteamTool(),
              "安装完成。建议重启 Steam 后生效。",
            )
          }
          onRestoreOpenSteamTool={() =>
            runAction("restore", "settings", () => restoreOpenSteamTool(), "已移除组件。")
          }
          onToggleSteamClientLock={toggleSteamClientLock}
        />
      )}
    </AppLayout>
  );
}
