import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import appPackage from "../../package.json";
import { PackagesPage } from "../features/packages/PackagesPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { arrayBufferToBase64 } from "../lib/file";
import { call } from "../lib/tauri";
import { canAddManifest } from "../lib/hubcap";
import { wait, waitForNextPaint } from "../lib/render";
import { enrichPackageMetadata, searchSteamStore } from "../lib/steam";
import type { AppState, HubcapManifestStatus, HubcapQuota, Notice, Page, PackageItem, SteamSearchResult } from "../types";
import { AppLayout } from "./AppLayout";

const APP_VERSION = appPackage.version;

function isTauriRuntime() {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

function waitForTauriRuntime(timeoutMs = 2500) {
  return new Promise<boolean>((resolve) => {
    if (isTauriRuntime()) {
      resolve(true);
      return;
    }

    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      if (isTauriRuntime()) {
        window.clearInterval(timer);
        resolve(true);
        return;
      }

      if (Date.now() - startedAt >= timeoutMs) {
        window.clearInterval(timer);
        resolve(false);
      }
    }, 50);
  });
}

export default function App() {
  const [page, setPage] = useState<Page>("packages");
  const [state, setState] = useState<AppState | null>(null);
  const [steamPathInput, setSteamPathInput] = useState("");
  const [hubcapKeyInput, setHubcapKeyInput] = useState("");
  const [hubcapQuota, setHubcapQuota] = useState<HubcapQuota | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [searchResults, setSearchResults] = useState<SteamSearchResult[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const busyRef = useRef<string | null>(null);
  const stateApplyVersion = useRef(0);

  const packages = state?.packages ?? [];
  const hasLoadedState = state !== null;

  async function applyAppState(nextState: AppState) {
    const applyVersion = stateApplyVersion.current + 1;
    stateApplyVersion.current = applyVersion;
    setState(nextState);
    setSteamPathInput(nextState.settings.steamPath ?? "");
    setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");

    const enrichedState = await enrichPackageMetadata(nextState);
    if (stateApplyVersion.current !== applyVersion) return;
    setState(enrichedState);
    setSteamPathInput(enrichedState.settings.steamPath ?? "");
    setHubcapKeyInput(enrichedState.settings.hubcapApiKey ?? "");
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
        const nextState = await call<AppState>("get_initial_state");
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

  function switchPage(nextPage: Page) {
    setPage(nextPage);
    setNotice(null);
  }

  function clearSearchState() {
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
      const [nextState] = await Promise.all([call<AppState>("get_initial_state"), wait(320)]);
      await applyAppState(nextState);
      clearSearchState();
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
    success?: string,
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
        setNotice({ page: noticePage, text: success, kind: "success" });
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
        return call<AppState>("import_package_from_bytes", {
          fileName: file.name,
          dataBase64,
        });
      },
      "已导入清单。",
    );
  }

  async function searchSteamGames(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const label = "steam-search";
    if (!beginAction(label, true)) return;
    const query = searchTerm.trim();
    if (!query) {
      endAction(label, true);
      setNotice({ page: "packages", text: "请输入游戏名称。", kind: "warning" });
      return;
    }

    try {
      setNotice(null);
      setHasSearched(true);
      const results = await searchSteamStore(query);
      const resultsWithStatuses = await attachHubcapStatuses(results);
      setSearchResults(resultsWithStatuses);
      if (!results.length) {
        setNotice({ page: "packages", text: "没有搜索结果。", kind: "info" });
      }
    } catch (error) {
      setSearchResults([]);
      setNotice({ page: "packages", text: String(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function attachHubcapStatuses(results: SteamSearchResult[]) {
    if (!results.length) return results;

    const apiKey = state?.settings.hubcapApiKey?.trim() || hubcapKeyInput.trim();
    if (!apiKey) {
      setNotice({ page: "packages", text: "请先在设置里保存 Key，才能检查清单。", kind: "warning" });
      return results.map((item) => ({ ...item, manifestChecked: false, manifestStatus: null }));
    }

    const statuses = await call<HubcapManifestStatus[]>("check_hubcap_manifest_statuses", {
      appIds: results.map((item) => item.id),
    });
    const byAppId = new Map(statuses.map((status) => [status.appId, status]));
    return results.map((item) => ({
      ...item,
      manifestChecked: true,
      manifestStatus: byAppId.get(item.id) ?? null,
    }));
  }

  async function addSearchResult(item: SteamSearchResult) {
    const label = `add-hubcap-${item.id}`;
    if (!beginAction(label, true)) return;
    try {
      setNotice({ page: "packages", text: `正在添加 ${item.name}，请稍候。`, kind: "info" });

      if (!canAddManifest(item)) {
        throw new Error("当前没有可用清单。");
      }

      await waitForNextPaint();

      const nextState = await call<AppState>("add_hubcap_manifest", {
        appId: item.id,
        title: item.name,
        imageUrl: item.tinyImage,
      });
      await applyAppState(nextState);

      setNotice({ page: "packages", text: `已添加 ${item.name}。`, kind: "success" });
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

    await runAction(`toggle-${pkg.id}`, "packages", () => call<AppState>("set_package_enabled", { id: pkg.id, enabled }));
  }

  async function deletePackage(pkg: PackageItem) {
    await runAction(`delete-${pkg.id}`, "packages", () => call<AppState>("delete_package", { id: pkg.id }), "已删除清单。");
  }

  async function saveSteamPath() {
    await runAction(
      "steam-path",
      "settings",
      () => call<AppState>("set_steam_path", { path: steamPathInput.trim() }),
      "Steam 路径已保存，请重新启用需要的清单。",
    );
  }

  async function saveHubcapKey() {
    const label = "hubcap-key";
    if (!beginAction(label)) return;
    try {
      setNotice(null);

      const nextState = await call<AppState>("set_hubcap_api_key", { apiKey: hubcapKeyInput.trim() });
      await applyAppState(nextState);

      if (hubcapKeyInput.trim()) {
        setHubcapQuota(await call<HubcapQuota>("get_hubcap_quota"));
      } else {
        setHubcapQuota(null);
      }

      setNotice({ page: "settings", text: "Hubcap Key 已保存。", kind: "success" });
    } catch (error) {
      setHubcapQuota(null);
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
      setHubcapQuota(await call<HubcapQuota>("get_hubcap_quota"));
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
      const path = await call<string | null>("detect_steam_path");
      if (!path) throw new Error("没有自动检测到 Steam 路径，可以手动填写 Steam 根目录。");

      setSteamPathInput(path);
      if (path === state?.settings.steamPath) {
        setNotice({ page: "settings", text: "Steam 路径已是最新。", kind: "success" });
        return;
      }

      const nextState = await call<AppState>("set_steam_path", { path });
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
        () => call<AppState>("set_steam_path", { path: selectedPath }),
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
      () => call<AppState>("set_steam_client_version_locked", { locked }),
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
          searchResults={searchResults}
          searchTerm={searchTerm}
          hasSearched={hasSearched}
          hasLoadedState={hasLoadedState}
          busy={busy}
          onRefresh={refreshState}
          onImportFile={handleImportFile}
          onSearch={searchSteamGames}
          onSearchTermChange={setSearchTerm}
          onAddSearchResult={addSearchResult}
          onTogglePackage={togglePackage}
          onDeletePackage={deletePackage}
        />
      ) : (
        <SettingsPage
          appVersion={APP_VERSION}
          notice={notice}
          state={state}
          steamPathInput={steamPathInput}
          hubcapKeyInput={hubcapKeyInput}
          hubcapQuota={hubcapQuota}
          busy={busy}
          onSteamPathChange={setSteamPathInput}
          onHubcapKeyChange={(value) => {
            setHubcapKeyInput(value);
            setHubcapQuota(null);
          }}
          onSaveSteamPath={saveSteamPath}
          onDetectSteamPath={detectSteamPath}
          onChooseSteamPath={chooseSteamPath}
          onSaveHubcapKey={saveHubcapKey}
          onRefreshHubcapQuota={refreshHubcapQuota}
          onInstallOpenSteamTool={() =>
            runAction(
              "install",
              "settings",
              () => call<AppState>("install_opensteamtool"),
              "安装完成。建议重启 Steam 后生效。",
            )
          }
          onRestoreOpenSteamTool={() =>
            runAction("restore", "settings", () => call<AppState>("restore_opensteamtool"), "已移除组件。")
          }
          onToggleSteamClientLock={toggleSteamClientLock}
        />
      )}
    </AppLayout>
  );
}
