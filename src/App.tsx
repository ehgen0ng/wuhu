import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  CheckCircle2,
  FolderCog,
  FolderOpen,
  KeyRound,
  LockKeyhole,
  PackagePlus,
  RefreshCcw,
  Search,
  Settings,
  Trash2,
  Upload,
  Wrench,
} from "lucide-react";
import { ChangeEvent, FormEvent, useEffect, useRef, useState } from "react";
import appIcon from "./assets/icon.png";
import { Switch } from "./Switch";
import type {
  AppState,
  HubcapManifestStatus,
  PackageItem,
  SteamSearchPlatforms,
  SteamSearchPrice,
  SteamSearchResult,
} from "./types";

type Page = "packages" | "settings";
type Notice = {
  page: Page;
  text: string;
};

type RawSteamSearchItem = {
  type?: string;
  itemType?: string;
  name?: string;
  id?: number;
  tiny_image?: string;
  tinyImage?: string | null;
  price?: SteamSearchPrice | null;
  platforms?: SteamSearchPlatforms | null;
};

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

function arrayBufferToBase64(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";

  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }

  return btoa(binary);
}

function packageSubtitle(pkg: PackageItem) {
  const app = pkg.appId ? `AppID ${pkg.appId}` : "未识别 AppID";
  if (!pkg.manifestFiles.length) return app;
  return `${app} · ${pkg.manifestFiles.length} 个 manifest`;
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function normalizeSteamSearchItem(item: RawSteamSearchItem): SteamSearchResult | null {
  const itemType = item.itemType ?? item.type ?? "";
  const name = item.name?.trim() ?? "";
  if (!name || typeof item.id !== "number") return null;

  return {
    itemType,
    name,
    id: item.id,
    tinyImage: item.tinyImage ?? item.tiny_image ?? null,
    price: item.price ?? null,
    platforms: item.platforms ?? null,
  };
}

async function searchSteamStore(query: string): Promise<SteamSearchResult[]> {
  if (isTauriRuntime()) {
    const items = await call<RawSteamSearchItem[]>("search_steam_games", { query });
    return items.map(normalizeSteamSearchItem).filter((item): item is SteamSearchResult => Boolean(item));
  }

  const url = new URL("/steam-api/api/storesearch/", window.location.origin);
  url.searchParams.set("term", query);
  url.searchParams.set("l", "schinese");
  url.searchParams.set("cc", "cn");

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Steam 搜索失败：HTTP ${response.status}`);
  }

  const data = (await response.json()) as { items?: RawSteamSearchItem[] };
  return (data.items ?? [])
    .map(normalizeSteamSearchItem)
    .filter((item): item is SteamSearchResult => Boolean(item));
}

function formatSteamPrice(price: SteamSearchPrice | null) {
  if (!price) return null;
  if (price.final === 0) return "免费";
  const value = (price.final / 100).toFixed(2);
  if (price.currency === "CNY") return `¥ ${value}`;
  return `${price.currency} ${value}`;
}

function searchResultSubtitle(item: SteamSearchResult) {
  const price = formatSteamPrice(item.price);
  return price ? `AppID ${item.id} · ${price}` : `AppID ${item.id}`;
}

function searchResultBadge(item: SteamSearchResult) {
  if (item.platforms?.windows) return "Windows";
  return "Steam";
}

function formatManifestTime(value: string | null | undefined) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFileSize(value: number | null | undefined) {
  if (!value) return null;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function manifestStatusText(item: SteamSearchResult) {
  if (!item.manifestChecked) return null;
  const status = item.manifestStatus;
  if (!status) return null;
  if (status.updateInProgress) return null;
  if (!canAddManifest(item)) return null;
  const size = formatFileSize(status.fileSize);
  return `清单更新：${formatManifestTime(status.fileModified)}${size ? ` · ${size}` : ""}`;
}

function canAddManifest(item: SteamSearchResult) {
  const status = item.manifestStatus;
  return Boolean(
    item.manifestChecked &&
      status?.available &&
      status.manifestFileExists &&
      !status.updateInProgress &&
      status.status?.toLowerCase() === "available",
  );
}

function normalizeHubcapStatus(appId: number, raw: Record<string, unknown>): HubcapManifestStatus {
  const status = typeof raw.status === "string" ? raw.status : null;
  const updateInProgress = typeof raw.update_in_progress === "boolean" ? raw.update_in_progress : null;
  const manifestFileExists = raw.manifest_file_exists === true;
  const available =
    manifestFileExists && status?.toLowerCase() === "available" && updateInProgress !== true;

  return {
    appId: Number(raw.app_id ?? appId),
    gameName: typeof raw.game_name === "string" ? raw.game_name : null,
    status,
    available,
    manifestFileExists,
    updateInProgress,
    needsUpdate: typeof raw.needs_update === "boolean" ? raw.needs_update : null,
    fileSize: typeof raw.file_size === "number" ? raw.file_size : null,
    fileModified: typeof raw.file_modified === "string" ? raw.file_modified : null,
    error: typeof raw.detail === "string" ? raw.detail : null,
  };
}

async function fetchHubcapStatusInBrowser(appId: number, apiKey: string): Promise<HubcapManifestStatus> {
  const response = await fetch(`/hubcap-api/api/v1/status/${appId}`, {
    headers: { Authorization: `Bearer ${apiKey}` },
  });
  const text = await response.text();
  const raw = text ? (JSON.parse(text) as Record<string, unknown>) : {};

  if (!response.ok) {
    return {
      appId,
      gameName: null,
      status: String(response.status),
      available: false,
      manifestFileExists: false,
      updateInProgress: null,
      needsUpdate: null,
      fileSize: null,
      fileModified: null,
      error: typeof raw.detail === "string" ? raw.detail : null,
    };
  }

  return normalizeHubcapStatus(appId, raw);
}

function fallbackState(previous: AppState | null): AppState {
  return (
    previous ?? {
      settings: { steamPath: null, hubcapApiKey: null },
      packages: [],
      installStatus: { installed: false },
      steamClient: { version: null, clientBuildDate: null, locked: false },
    }
  );
}

function packageFromSearchResult(item: SteamSearchResult): PackageItem {
  return {
    id: item.id.toString(),
    title: item.name,
    appId: item.id,
    luaFileName: `wuhu_${item.id}.lua`,
    manifestFiles: [],
    sourceZipName: "Steam 搜索",
    enabled: true,
    importedAt: Math.floor(Date.now() / 1000),
    manifestUpdatedAt: null,
    manifestFileSize: null,
  };
}

function formatSteamVersion(version: string | null | undefined) {
  if (!version) return "未识别";
  return version;
}

function formatSteamBuildDate(seconds: number | null | undefined) {
  if (!seconds) return "未识别";
  const utcMinusEight = new Date((seconds - 8 * 60 * 60) * 1000);
  if (Number.isNaN(utcMinusEight.getTime())) return "未识别";

  const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
  const year = utcMinusEight.getUTCFullYear();
  const month = utcMinusEight.getUTCMonth() + 1;
  const day = utcMinusEight.getUTCDate();
  const weekday = weekdays[utcMinusEight.getUTCDay()];
  const hour = utcMinusEight.getUTCHours();
  const minute = String(utcMinusEight.getUTCMinutes()).padStart(2, "0");

  return `${year}年${month}月${day}日${weekday} ${hour}:${minute} UTC-08:00`;
}

export default function App() {
  const [page, setPage] = useState<Page>("packages");
  const [state, setState] = useState<AppState | null>(null);
  const [steamPathInput, setSteamPathInput] = useState("");
  const [hubcapKeyInput, setHubcapKeyInput] = useState("");
  const [searchTerm, setSearchTerm] = useState("");
  const [searchResults, setSearchResults] = useState<SteamSearchResult[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);

  const packages = state?.packages ?? [];

  useEffect(() => {
    if (!isTauriRuntime()) {
      const nextState = fallbackState(null);
      setState(nextState);
      setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
      return;
    }

    call<AppState>("get_initial_state")
      .then((nextState) => {
        setState(nextState);
        setSteamPathInput(nextState.settings.steamPath ?? "");
        setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
      })
      .catch((error) => setNotice({ page: "packages", text: String(error) }));
  }, []);

  function switchPage(nextPage: Page) {
    setPage(nextPage);
    setNotice(null);
  }

  async function refreshState() {
    try {
      setBusy("refresh");
      setNotice(null);
      const nextState = await call<AppState>("get_initial_state");
      setState(nextState);
      setSteamPathInput(nextState.settings.steamPath ?? "");
      setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
    } catch (error) {
      setNotice({ page: "packages", text: String(error) });
    } finally {
      setBusy(null);
    }
  }

  async function runAction(
    label: string,
    noticePage: Page,
    action: () => Promise<AppState | void>,
    success?: string,
  ) {
    try {
      setBusy(label);
      setNotice(null);
      const nextState = await action();
      if (nextState) {
        setState(nextState);
        setSteamPathInput(nextState.settings.steamPath ?? "");
        setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
      }
      if (success) {
        setNotice({ page: noticePage, text: success });
      }
    } catch (error) {
      setNotice({ page: noticePage, text: String(error) });
    } finally {
      setBusy(null);
    }
  }

  async function handleImport(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
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

  async function searchSteamGames(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = searchTerm.trim();
    if (!query) {
      setNotice({ page: "packages", text: "请输入游戏名称。" });
      return;
    }

    try {
      setBusy("steam-search");
      setNotice(null);
      setSearchResults([]);
      setHasSearched(true);
      const results = await searchSteamStore(query);
      const resultsWithStatuses = await attachHubcapStatuses(results);
      setSearchResults(resultsWithStatuses);
      if (!results.length) {
        setNotice({ page: "packages", text: "没有搜索结果。" });
      }
    } catch (error) {
      setSearchResults([]);
      setNotice({ page: "packages", text: String(error) });
    } finally {
      setBusy(null);
    }
  }

  async function attachHubcapStatuses(results: SteamSearchResult[]) {
    if (!results.length) return results;

    const apiKey = state?.settings.hubcapApiKey?.trim() || hubcapKeyInput.trim();
    if (!apiKey) {
      setNotice({ page: "packages", text: "请先在设置里保存 Key，才能检查清单。" });
      return results.map((item) => ({ ...item, manifestChecked: false, manifestStatus: null }));
    }

    if (!isTauriRuntime()) {
      const statuses = await Promise.all(results.map((item) => fetchHubcapStatusInBrowser(item.id, apiKey)));
      const byAppId = new Map(statuses.map((status) => [status.appId, status]));
      return results.map((item) => ({
        ...item,
        manifestChecked: true,
        manifestStatus: byAppId.get(item.id) ?? null,
      }));
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
    try {
      setBusy(label);
      setNotice(null);

      if (!canAddManifest(item)) {
        throw new Error("当前没有可用清单。");
      }

      if (isTauriRuntime()) {
        const nextState = await call<AppState>("add_hubcap_manifest", {
          appId: item.id,
          title: item.name,
        });
        setState(nextState);
        setSteamPathInput(nextState.settings.steamPath ?? "");
      } else {
        setState((previous) => {
          const base = fallbackState(previous);
          const nextPackage = packageFromSearchResult(item);
          const packages = base.packages
            .filter((pkg) => pkg.id !== nextPackage.id)
            .concat(nextPackage)
            .sort((left, right) => left.title.localeCompare(right.title, "zh-Hans-CN"));

          return { ...base, packages };
        });
      }

      setNotice({ page: "packages", text: `已添加 ${item.name}。` });
    } catch (error) {
      setNotice({ page: "packages", text: String(error) });
    } finally {
      setBusy(null);
    }
  }

  async function togglePackage(pkg: PackageItem, enabled: boolean) {
    await runAction(
      `toggle-${pkg.id}`,
      "packages",
      () => call<AppState>("set_package_enabled", { id: pkg.id, enabled }),
    );
  }

  async function deletePackage(pkg: PackageItem) {
    await runAction(
      `delete-${pkg.id}`,
      "packages",
      () => call<AppState>("delete_package", { id: pkg.id }),
      "已删除清单。",
    );
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
    if (!isTauriRuntime()) {
      const apiKey = hubcapKeyInput.trim();
      const base = fallbackState(state);
      setState({
        ...base,
        settings: { ...base.settings, hubcapApiKey: apiKey || null },
      });
      setNotice({ page: "settings", text: "Hubcap Key 已保存。" });
      return;
    }

    await runAction(
      "hubcap-key",
      "settings",
      () => call<AppState>("set_hubcap_api_key", { apiKey: hubcapKeyInput.trim() }),
      "Hubcap Key 已保存。",
    );
  }

  async function detectSteamPath() {
    await runAction(
      "detect-steam",
      "settings",
      async () => {
        const path = await call<string | null>("detect_steam_path");
        if (!path) throw new Error("没有自动检测到 Steam 路径，可以手动填写 Steam 根目录。");
        setSteamPathInput(path);
        return call<AppState>("set_steam_path", { path });
      },
      "已自动检测并保存 Steam 路径，请重新启用需要的清单。",
    );
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
      setNotice({ page: "settings", text: String(error) });
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

  const hasSteamPath = Boolean(state?.settings.steamPath);
  const isRefreshing = busy === "refresh";
  const isSearching = busy === "steam-search";

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brand-mark" src={appIcon} alt="" />
          <div>
            <div className="brand-title">wuhu</div>
          </div>
        </div>

        <nav className="nav-group" aria-label="主导航">
          <button
            className={page === "packages" ? "nav-item active" : "nav-item"}
            onClick={() => switchPage("packages")}
          >
            <Archive size={19} />
            清单管理
          </button>
          <button
            className={page === "settings" ? "nav-item active" : "nav-item"}
            onClick={() => switchPage("settings")}
          >
            <Settings size={19} />
            设置
          </button>
        </nav>

        <div className="sidebar-footer">
          <div className={state?.installStatus.installed ? "status-dot good" : "status-dot"} />
          <span>{state?.installStatus.installed ? "组件已安装" : "等待安装组件"}</span>
        </div>
      </aside>

      <main className="content">
        {page === "packages" ? (
          <section className="page">
            <header className="page-header">
              <div>
                <h1>清单管理</h1>
              </div>
              <div className="header-actions">
                <button className="ghost-button" onClick={refreshState} disabled={Boolean(busy)}>
                  <RefreshCcw className={isRefreshing ? "spin" : undefined} size={17} />
                  刷新
                </button>
                <button
                  className="primary-button"
                  onClick={() => fileInput.current?.click()}
                  disabled={Boolean(busy)}
                >
                  <PackagePlus size={18} />
                  添加游戏
                </button>
                <input ref={fileInput} type="file" accept=".zip" hidden onChange={handleImport} />
              </div>
            </header>

            {notice?.page === "packages" && <div className="notice">{notice.text}</div>}

            <form className="search-panel" onSubmit={searchSteamGames}>
              <div className="search-row">
                <input
                  value={searchTerm}
                  onChange={(event) => setSearchTerm(event.target.value)}
                  placeholder="搜索 Steam 游戏名"
                  disabled={Boolean(busy)}
                />
                <button className="primary-button" type="submit" disabled={Boolean(busy) || !searchTerm.trim()}>
                  <Search className={isSearching ? "spin" : undefined} size={17} />
                  搜索
                </button>
              </div>
            </form>

            {hasSearched && searchResults.length > 0 && (
              <section className="result-section">
                <div className="section-heading">
                  <h2>搜索结果</h2>
                  <span>{searchResults.length} 个结果</span>
                </div>
                <div className="package-grid">
                  {searchResults.map((item, index) => {
                    const existingPackage = packages.find(
                      (pkg) => pkg.appId === item.id || pkg.id === item.id.toString(),
                    );
                    const canAdd = canAddManifest(item);
                    const manifestText = manifestStatusText(item);
                    return (
                      <article className="package-card search-card" key={item.id}>
                        <div className={`card-art card-art-${index % 4}`}>
                          {item.tinyImage && <img src={item.tinyImage} alt="" />}
                          <span>{searchResultBadge(item)}</span>
                        </div>
                        <div className="card-body">
                          <div className="card-main">
                            <h2>{item.name}</h2>
                            <p>{searchResultSubtitle(item)}</p>
                            {manifestText && <div className="manifest-meta good">{manifestText}</div>}
                            {existingPackage?.manifestUpdatedAt && (
                              <div className="manifest-meta">
                                已添加：{formatManifestTime(existingPackage.manifestUpdatedAt)}
                              </div>
                            )}
                          </div>

                          <div className="card-actions">
                            {canAdd && (
                              <button
                                className="ghost-button add-card-button"
                                onClick={() => addSearchResult(item)}
                                disabled={Boolean(busy)}
                              >
                                <PackagePlus size={17} />
                                {existingPackage ? "重新添加" : "添加"}
                              </button>
                            )}
                          </div>
                        </div>
                      </article>
                    );
                  })}
                </div>
              </section>
            )}

            <div className="package-grid">
              {packages.map((pkg, index) => (
                <article className="package-card" key={pkg.id}>
                  <div className={`card-art card-art-${index % 4}`}>
                    <span>{pkg.enabled ? "已启用" : "已禁用"}</span>
                  </div>
                  <div className="card-body">
                    <div className="card-main">
                      <h2>{pkg.title}</h2>
                      <p>{packageSubtitle(pkg)}</p>
                      {pkg.manifestUpdatedAt && (
                        <div className="manifest-meta">
                          清单更新：{formatManifestTime(pkg.manifestUpdatedAt)}
                        </div>
                      )}
                    </div>

                    <div className="card-actions">
                      <Switch
                        checked={pkg.enabled}
                        disabled={Boolean(busy)}
                        title={pkg.enabled ? "禁用" : "启用"}
                        ariaLabel={`${pkg.enabled ? "禁用" : "启用"} ${pkg.title}`}
                        onChange={(enabled) => togglePackage(pkg, enabled)}
                      />
                      <button
                        className="icon-button danger"
                        aria-label={`删除 ${pkg.title}`}
                        title="删除"
                        onClick={() => deletePackage(pkg)}
                        disabled={Boolean(busy)}
                      >
                        <Trash2 size={18} />
                      </button>
                    </div>
                  </div>
                </article>
              ))}
            </div>

            {!packages.length && (
              <div className="empty-state">
                <Upload size={34} />
                <h2>还没有清单</h2>
                <button className="primary-button" onClick={() => fileInput.current?.click()}>
                  <PackagePlus size={18} />
                  添加游戏
                </button>
              </div>
            )}
          </section>
        ) : (
          <section className="page settings-page">
            <header className="page-header">
              <div>
                <h1>设置</h1>
              </div>
            </header>

            {notice?.page === "settings" && <div className="notice">{notice.text}</div>}

            <div className="settings-panel">
              <div className="panel-title">
                <FolderCog size={20} />
                <div>
                  <h2>Steam 路径</h2>
                </div>
              </div>
              <div className="path-row">
                <input
                  value={steamPathInput}
                  onChange={(event) => setSteamPathInput(event.target.value)}
                  placeholder="Steam 根目录"
                />
                <button className="ghost-button" onClick={detectSteamPath} disabled={Boolean(busy)}>
                  <RefreshCcw size={17} />
                  自动读取
                </button>
                <button className="ghost-button" onClick={chooseSteamPath} disabled={Boolean(busy)}>
                  <FolderOpen size={17} />
                  选择目录
                </button>
                <button className="primary-button" onClick={saveSteamPath} disabled={Boolean(busy)}>
                  保存
                </button>
              </div>
            </div>

            <div className="settings-panel">
              <div className="panel-title">
                <KeyRound size={20} />
                <div>
                  <h2>Hubcap Key</h2>
                </div>
              </div>
              <div className="path-row key-row">
                <input
                  type="password"
                  value={hubcapKeyInput}
                  onChange={(event) => setHubcapKeyInput(event.target.value)}
                  placeholder="Hubcap Key"
                  autoComplete="off"
                />
                <button className="primary-button" onClick={saveHubcapKey} disabled={Boolean(busy)}>
                  保存 Key
                </button>
              </div>
            </div>

            <div className="settings-panel">
              <div className="panel-title">
                <Wrench size={20} />
                <div>
                  <h2>组件安装</h2>
                </div>
              </div>

              <div className="choice-row">
                <span>实现方式</span>
                <button className="choice-button active" disabled>
                  <CheckCircle2 size={17} />
                  OpenSteamTool
                </button>
              </div>

              <div className="install-grid">
                <div className="install-card">
                  <span>当前状态</span>
                  <strong>{state?.installStatus.installed ? "已安装" : "未安装"}</strong>
                </div>
              </div>

              <div className="button-row">
                <button
                  className="primary-button"
                  onClick={() =>
                    runAction(
                      "install",
                      "settings",
                      () => call<AppState>("install_opensteamtool"),
                      "安装完成。建议重启 Steam 后生效。",
                    )
                  }
                  disabled={Boolean(busy) || !hasSteamPath}
                >
                  <CheckCircle2 size={18} />
                  安装
                </button>
                <button
                  className="ghost-button danger-text"
                  onClick={() =>
                    runAction(
                      "restore",
                      "settings",
                      () => call<AppState>("restore_opensteamtool"),
                      "已移除组件。",
                    )
                  }
                  disabled={Boolean(busy) || !state?.installStatus.installed}
                >
                  恢复
                </button>
              </div>
            </div>

            <div className="settings-panel">
              <div className="panel-title">
                <LockKeyhole size={20} />
                <div>
                  <h2>Steam 客户端版本</h2>
                </div>
              </div>

              <div className="install-grid client-grid">
                <div className="install-card">
                  <span>Steam 版本</span>
                  <strong>{formatSteamVersion(state?.steamClient.version)}</strong>
                  <small className="version-detail">
                    客户端生成日期：{formatSteamBuildDate(state?.steamClient.clientBuildDate)}
                  </small>
                </div>
                <div className="install-card client-lock-card">
                  <div>
                    <span>锁定版本</span>
                    <strong>{state?.steamClient.locked ? "已锁定" : "未锁定"}</strong>
                  </div>
                  <Switch
                    checked={Boolean(state?.steamClient.locked)}
                    disabled={Boolean(busy) || !hasSteamPath}
                    title={state?.steamClient.locked ? "取消锁定" : "锁定"}
                    ariaLabel={state?.steamClient.locked ? "取消锁定 Steam 客户端版本" : "锁定 Steam 客户端版本"}
                    onChange={toggleSteamClientLock}
                  />
                </div>
              </div>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}
