import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  CheckCircle2,
  FolderCog,
  FolderOpen,
  LockKeyhole,
  PackagePlus,
  RefreshCcw,
  Settings,
  Trash2,
  Upload,
  Wrench,
} from "lucide-react";
import { ChangeEvent, useEffect, useRef, useState } from "react";
import appIcon from "./assets/icon.png";
import { Switch } from "./Switch";
import type { AppState, PackageItem } from "./types";

type Page = "packages" | "settings";
type Notice = {
  page: Page;
  text: string;
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
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);

  const packages = state?.packages ?? [];

  useEffect(() => {
    call<AppState>("get_initial_state")
      .then((nextState) => {
        setState(nextState);
        setSteamPathInput(nextState.settings.steamPath ?? "");
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
