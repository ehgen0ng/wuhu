use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(any(windows, target_os = "macos"))]
use std::fs;

#[cfg(target_os = "macos")]
use std::process::Command;

use crate::models::{AppStore, InstallStatus, SteamClientStatus};

#[cfg(windows)]
const PROXY_DLL_NAMES: [&str; 2] = ["dwmapi.dll", "xinput1_4.dll"];

#[cfg(windows)]
const OPENSTEAMTOOL_DLL_NAME: &str = "OpenSteamTool.dll";

#[cfg(windows)]
const CLOUD_REDIRECT_DLL_NAME: &str = "cloud_redirect.dll";

#[cfg(target_os = "macos")]
const OPENSTEAMTOOL_DYLIB_NAME: &str = "libOpenSteamTool.dylib";

#[cfg(target_os = "macos")]
const CLOUD_REDIRECT_DYLIB_NAME: &str = "cloud_redirect.dylib";

#[cfg(all(target_os = "macos", debug_assertions))]
const EMBEDDED_OPENSTEAMTOOL_DYLIB: &[u8] =
    include_bytes!("../../resources/opensteamtool/macos/debug/libOpenSteamTool.dylib");

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const EMBEDDED_OPENSTEAMTOOL_DYLIB: &[u8] =
    include_bytes!("../../resources/opensteamtool/macos/release/libOpenSteamTool.dylib");

#[cfg(target_os = "macos")]
const EMBEDDED_CLOUD_REDIRECT_DYLIB: &[u8] =
    include_bytes!("../../resources/cloudredirect/macos/cloud_redirect.dylib");

#[cfg(windows)]
struct EmbeddedToolFile {
    name: &'static str,
    bytes: &'static [u8],
}

#[cfg(all(windows, debug_assertions))]
const EMBEDDED_DWMAPI_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/debug/dwmapi.dll");

#[cfg(all(windows, not(debug_assertions)))]
const EMBEDDED_DWMAPI_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/release/dwmapi.dll");

#[cfg(all(windows, debug_assertions))]
const EMBEDDED_XINPUT_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/debug/xinput1_4.dll");

#[cfg(all(windows, not(debug_assertions)))]
const EMBEDDED_XINPUT_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/release/xinput1_4.dll");

#[cfg(all(windows, debug_assertions))]
const EMBEDDED_OPENSTEAMTOOL_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/debug/OpenSteamTool.dll");

#[cfg(all(windows, not(debug_assertions)))]
const EMBEDDED_OPENSTEAMTOOL_DLL: &[u8] =
    include_bytes!("../../resources/opensteamtool/windows/release/OpenSteamTool.dll");

#[cfg(windows)]
const EMBEDDED_CLOUD_REDIRECT_DLL: &[u8] =
    include_bytes!("../../resources/cloudredirect/windows/cloud_redirect.dll");

#[cfg(windows)]
const EMBEDDED_TOOL_FILES: [EmbeddedToolFile; 3] = [
    EmbeddedToolFile {
        name: "dwmapi.dll",
        bytes: EMBEDDED_DWMAPI_DLL,
    },
    EmbeddedToolFile {
        name: "xinput1_4.dll",
        bytes: EMBEDDED_XINPUT_DLL,
    },
    EmbeddedToolFile {
        name: "OpenSteamTool.dll",
        bytes: EMBEDDED_OPENSTEAMTOOL_DLL,
    },
];

pub(crate) fn detect_path() -> Option<String> {
    detect_path_candidates()
        .into_iter()
        .find_map(|path| normalize_configured_path(&path))
        .and_then(|path| path_to_string(&path))
}

#[cfg(windows)]
fn detect_path_candidates() -> Vec<PathBuf> {
    let candidates = [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"];

    candidates.iter().map(PathBuf::from).collect()
}

#[cfg(target_os = "macos")]
fn detect_path_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from("/Applications/Steam.app"));
    if let Some(home) = home_dir() {
        candidates.push(home.join("Applications").join("Steam.app"));
    }
    if let Some(root) = default_macos_data_root() {
        candidates.push(root);
    }
    candidates
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect_path_candidates() -> Vec<PathBuf> {
    Vec::new()
}

pub(crate) fn normalize_path(path: &str) -> Option<String> {
    normalize_configured_path(&input_path(path)?).and_then(|path| path_to_string(&path))
}

#[cfg(target_os = "macos")]
fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
    macos_app_bundle_path(path).or_else(|| normalize_root_path(path))
}

#[cfg(not(target_os = "macos"))]
fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
    normalize_root_path(path)
}

pub(crate) fn configured_root(store: &AppStore) -> Option<PathBuf> {
    store
        .settings
        .steam_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(|path| normalize_root_path(&input_path(path)?))
}

pub(crate) fn supports_package_sync() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

pub(crate) fn supports_client_version_lock() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

pub(crate) fn package_sync_root(store: &AppStore) -> Option<PathBuf> {
    if !supports_package_sync() || package_lua_root().is_none() {
        return None;
    }

    configured_root(store)
}

pub(crate) fn package_lua_root() -> Option<PathBuf> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        opensteamtool_data_root().map(|root| root.join("lua"))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

fn input_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().map(|home| home.join(rest));
    }

    Some(PathBuf::from(trimmed))
}

fn path_to_string(path: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = home_relative_path(path) {
            return Some(path);
        }
    }

    path.to_str().map(ToString::to_string)
}

#[cfg(target_os = "macos")]
fn home_relative_path(path: &Path) -> Option<String> {
    let home = home_dir()?;
    if path == home.as_path() {
        return Some("~".to_string());
    }

    let rest = path.strip_prefix(&home).ok()?;
    let rest = rest.to_str()?;
    if rest.is_empty() {
        Some("~".to_string())
    } else {
        Some(format!("~/{rest}"))
    }
}

#[cfg(windows)]
fn normalize_root_path(path: &Path) -> Option<PathBuf> {
    if windows_looks_like_root(path) {
        Some(path.to_path_buf())
    } else {
        None
    }
}

#[cfg(windows)]
fn windows_looks_like_root(path: &Path) -> bool {
    path.join("steam.exe").exists() || path.join("Steam.exe").exists()
}

#[cfg(target_os = "macos")]
fn normalize_root_path(path: &Path) -> Option<PathBuf> {
    if macos_looks_like_data_root(path) {
        return Some(path.to_path_buf());
    }

    if let Some(root) = macos_data_root_from_app_bundle_path(path) {
        return Some(root);
    }

    if macos_looks_like_launcher_path(path) {
        return default_macos_data_root().filter(|root| macos_looks_like_data_root(root));
    }

    None
}

#[cfg(target_os = "macos")]
fn macos_looks_like_data_root(path: &Path) -> bool {
    path.join("Steam.AppBundle")
        .join("Steam")
        .join("Contents")
        .join("MacOS")
        .join("steamclient.dylib")
        .exists()
}

#[cfg(target_os = "macos")]
fn macos_data_root_from_app_bundle_path(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if !file_name_eq(ancestor, "Steam.AppBundle") {
            continue;
        }

        let root = ancestor.parent()?.to_path_buf();
        if macos_looks_like_data_root(&root) {
            return Some(root);
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn macos_looks_like_launcher_path(path: &Path) -> bool {
    macos_app_bundle_path(path).is_some()
}

#[cfg(target_os = "macos")]
fn macos_app_bundle_path(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| {
            file_name_eq(ancestor, "Steam.app")
                && ancestor
                    .join("Contents")
                    .join("MacOS")
                    .join("steam_osx")
                    .is_file()
        })
        .map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
fn default_macos_data_root() -> Option<PathBuf> {
    home_dir().map(|home| {
        home.join("Library")
            .join("Application Support")
            .join("Steam")
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn normalize_root_path(_path: &Path) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub(crate) fn install_opensteamtool(store: &AppStore) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = store;
        return Err("组件安装目前只支持 Windows Steam 客户端".to_string());
    }

    #[cfg(windows)]
    {
        let steam_root = configured_root(store).ok_or_else(|| "请先设置 Steam 路径".to_string())?;
        let core_path = opensteamtool_binary_path().ok_or_else(|| {
            "无法确定 OpenSteamTool 数据目录：环境变量 OST_DATA_DIR 和 LOCALAPPDATA 均未设置"
                .to_string()
        })?;
        let cloud_redirect_path = cloud_redirect_plugin_path().ok_or_else(|| {
            "无法确定 CloudRedirect 插件目录：环境变量 OST_DATA_DIR 和 LOCALAPPDATA 均未设置"
                .to_string()
        })?;

        if let Some(binary_dir) = core_path.parent() {
            fs::create_dir_all(binary_dir)
                .map_err(|err| format!("创建 OpenSteamTool bin 目录失败：{err}"))?;
        }
        if let Some(plugin_dir) = cloud_redirect_path.parent() {
            fs::create_dir_all(plugin_dir)
                .map_err(|err| format!("创建 OpenSteamTool plugins 目录失败：{err}"))?;
        }

        write_embedded_tool_file(&core_path, OPENSTEAMTOOL_DLL_NAME)?;
        write_cloud_redirect_if_missing(&cloud_redirect_path)?;

        for file_name in PROXY_DLL_NAMES {
            write_embedded_tool_file(&steam_root.join(file_name), file_name)?;
        }

        enable_opensteamtool_cloud()?;
        configure_cloud_redirect_local_folder()?;

        let legacy_core_path = steam_root.join(OPENSTEAMTOOL_DLL_NAME);
        if legacy_core_path.exists() {
            remove_component_file(&legacy_core_path, OPENSTEAMTOOL_DLL_NAME)?;
        }

        Ok(())
    }
}

pub(crate) fn ensure_opensteamtool_aligned(store: &AppStore) {
    #[cfg(not(windows))]
    {
        let _ = store;
    }

    #[cfg(windows)]
    {
        let Some(steam_root) = configured_root(store) else {
            return;
        };
        let Some(core_path) = opensteamtool_binary_path() else {
            return;
        };

        if !windows_components_present(&steam_root, &core_path) {
            return;
        }

        if let Some(binary_dir) = core_path.parent() {
            let _ = fs::create_dir_all(binary_dir);
        }

        let _ = write_embedded_tool_file(&core_path, OPENSTEAMTOOL_DLL_NAME);
        for file_name in PROXY_DLL_NAMES {
            let _ = write_embedded_tool_file(&steam_root.join(file_name), file_name);
        }

        let legacy_core_path = steam_root.join(OPENSTEAMTOOL_DLL_NAME);
        if legacy_core_path.exists() {
            let _ = remove_component_file(&legacy_core_path, OPENSTEAMTOOL_DLL_NAME);
        }
    }
}

pub(crate) fn launch_steam_with_opensteamtool(store: &AppStore) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = store;
        return Err("通过 wuhu 启动 Steam 目前只支持 macOS".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let steam_executable = macos_steam_executable(store)
            .ok_or_else(|| "没有找到设置中的 Steam.app，请检查 Steam 路径".to_string())?;
        let running_pids = macos_steam_process_ids()?;
        if !running_pids.is_empty() {
            let message = macos_launch_marker_pid()
                .filter(|pid| running_pids.contains(pid))
                .map(|_| "Steam 已经由 wuhu 启动".to_string())
                .unwrap_or_else(|| "Steam 正在运行，请先完全退出 Steam 后再启动".to_string());
            return Err(message);
        }

        let dylib_path = opensteamtool_binary_path().ok_or_else(|| {
            "无法确定 OpenSteamTool 数据目录：环境变量 OST_DATA_DIR 和 HOME 均未设置".to_string()
        })?;
        let cloud_redirect_path = cloud_redirect_plugin_path().ok_or_else(|| {
            "无法确定 CloudRedirect 插件目录：环境变量 OST_DATA_DIR 和 HOME 均未设置".to_string()
        })?;
        let binary_dir = dylib_path
            .parent()
            .ok_or_else(|| "OpenSteamTool bin 目录无效".to_string())?;
        fs::create_dir_all(binary_dir)
            .map_err(|err| format!("创建 OpenSteamTool bin 目录失败：{err}"))?;
        let plugin_dir = cloud_redirect_path
            .parent()
            .ok_or_else(|| "OpenSteamTool plugins 目录无效".to_string())?;
        fs::create_dir_all(plugin_dir)
            .map_err(|err| format!("创建 OpenSteamTool plugins 目录失败：{err}"))?;

        write_macos_component(
            &dylib_path,
            OPENSTEAMTOOL_DYLIB_NAME,
            EMBEDDED_OPENSTEAMTOOL_DYLIB,
        )?;
        write_macos_component(
            &cloud_redirect_path,
            CLOUD_REDIRECT_DYLIB_NAME,
            EMBEDDED_CLOUD_REDIRECT_DYLIB,
        )?;
        enable_opensteamtool_cloud()?;
        configure_cloud_redirect_local_folder()?;

        let backup_path = macos_steam_backup_path(&steam_executable);
        if !backup_path.exists() {
            macos_copy_preserving(&steam_executable, &backup_path, "备份 steam_osx")?;
        } else if !macos_is_ad_hoc_signed(&steam_executable)? {
            macos_copy_preserving(&steam_executable, &backup_path, "更新 steam_osx 备份")?;
        }

        macos_codesign(&steam_executable, "重签 steam_osx")?;
        macos_codesign(&dylib_path, "签名 libOpenSteamTool.dylib")?;
        macos_codesign(&cloud_redirect_path, "签名 cloud_redirect.dylib")?;

        let child = Command::new(&steam_executable)
            .env("DYLD_INSERT_LIBRARIES", &dylib_path)
            .spawn()
            .map_err(|err| format!("启动 Steam 失败：{err}"))?;
        let marker_path = macos_launch_marker_path()
            .ok_or_else(|| "无法确定 wuhu 启动状态文件位置".to_string())?;
        fs::write(&marker_path, child.id().to_string())
            .map_err(|err| format!("Steam 已启动，但记录 wuhu 启动状态失败：{err}"))?;

        Ok(())
    }
}

pub(crate) fn restore_opensteamtool(store: &AppStore) -> Result<(), String> {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = store;
        return Err("组件恢复目前只支持 Windows 和 macOS Steam 客户端".to_string());
    }

    #[cfg(windows)]
    {
        let steam_root = configured_root(store).ok_or_else(|| "请先设置 Steam 路径".to_string())?;

        let mut errors = Vec::new();
        for file_name in PROXY_DLL_NAMES {
            if let Err(err) = remove_component_file(&steam_root.join(file_name), file_name) {
                errors.push(err);
            }
        }

        if let Err(err) = remove_component_file(
            &steam_root.join(OPENSTEAMTOOL_DLL_NAME),
            OPENSTEAMTOOL_DLL_NAME,
        ) {
            errors.push(err);
        }

        match opensteamtool_binary_path() {
            Some(core_path) => {
                if let Err(err) = remove_component_file(&core_path, OPENSTEAMTOOL_DLL_NAME) {
                    errors.push(err);
                }
            }
            None => errors.push(
                "无法确定 OpenSteamTool 数据目录：环境变量 OST_DATA_DIR 和 LOCALAPPDATA 均未设置"
                    .to_string(),
            ),
        }
        if let Some(plugin_path) = cloud_redirect_plugin_path() {
            if let Err(err) = remove_component_file(&plugin_path, CLOUD_REDIRECT_DLL_NAME) {
                errors.push(err);
            }
        }

        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        if !macos_steam_process_ids()?.is_empty() {
            return Err("Steam 正在运行，请先完全退出 Steam 后再恢复".to_string());
        }

        let steam_executable = macos_steam_executable(store)
            .ok_or_else(|| "没有找到设置中的 Steam.app，请检查 Steam 路径".to_string())?;
        let backup_path = macos_steam_backup_path(&steam_executable);
        if backup_path.exists() {
            if macos_is_ad_hoc_signed(&steam_executable)? {
                macos_copy_preserving(&backup_path, &steam_executable, "恢复 steam_osx")?;
            }
            fs::remove_file(&backup_path)
                .map_err(|err| format!("删除 steam_osx 备份失败：{err}"))?;
        }

        if let Some(dylib_path) = opensteamtool_binary_path() {
            remove_macos_file_if_exists(&dylib_path, OPENSTEAMTOOL_DYLIB_NAME)?;
        }
        if let Some(plugin_path) = cloud_redirect_plugin_path() {
            remove_macos_file_if_exists(&plugin_path, CLOUD_REDIRECT_DYLIB_NAME)?;
        }
        if let Some(marker_path) = macos_launch_marker_path() {
            remove_macos_file_if_exists(&marker_path, "wuhu 启动状态")?;
        }

        Ok(())
    }
}

pub(crate) fn set_client_version_locked(store: &AppStore, locked: bool) -> Result<(), String> {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (store, locked);
        return Err("Steam 客户端版本锁定目前只支持 Windows 和 macOS".to_string());
    }

    #[cfg(any(windows, target_os = "macos"))]
    {
        let steam_root = configured_root(store).ok_or_else(|| "请先设置 Steam 路径".to_string())?;

        set_client_lock_file(&client_config_root(&steam_root), locked)
    }
}

pub(crate) fn install_status(store: &AppStore) -> InstallStatus {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = store;
        return InstallStatus {
            installed: false,
            supported: false,
            launch_required: false,
            launched_via_wuhu: false,
            update_available: false,
        };
    }

    #[cfg(windows)]
    {
        let (installed, update_available) = configured_root(store)
            .zip(opensteamtool_binary_path())
            .map(|(steam_root, core_path)| {
                let installed = windows_components_present(&steam_root, &core_path);
                let update_available = installed
                    && windows_component_targets(&steam_root, &core_path)
                        .into_iter()
                        .any(|(path, name)| !tool_file_matches_embedded(&path, name));
                (installed, update_available)
            })
            .unwrap_or((false, false));

        InstallStatus {
            installed,
            supported: true,
            launch_required: false,
            launched_via_wuhu: false,
            update_available,
        }
    }

    #[cfg(target_os = "macos")]
    {
        let installed = macos_steam_executable(store)
            .map(|path| macos_steam_backup_path(&path).exists())
            .unwrap_or(false);

        InstallStatus {
            installed,
            supported: true,
            launch_required: true,
            launched_via_wuhu: macos_launched_via_wuhu(),
            update_available: false,
        }
    }
}

pub(crate) fn client_status(store: &AppStore) -> SteamClientStatus {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = store;
        return SteamClientStatus {
            version: None,
            client_build_date: None,
            locked: false,
            lock_supported: false,
        };
    }

    #[cfg(any(windows, target_os = "macos"))]
    {
        let Some(steam_root) = configured_root(store) else {
            return SteamClientStatus {
                version: None,
                client_build_date: None,
                locked: false,
                lock_supported: true,
            };
        };

        SteamClientStatus {
            version: read_client_version(&steam_root),
            client_build_date: read_client_build_date(&steam_root),
            locked: is_client_locked(&client_config_root(&steam_root)),
            lock_supported: true,
        }
    }
}

#[cfg(windows)]
fn client_config_root(steam_root: &Path) -> PathBuf {
    steam_root.to_path_buf()
}

#[cfg(target_os = "macos")]
fn client_config_root(steam_root: &Path) -> PathBuf {
    steam_root
        .join("Steam.AppBundle")
        .join("Steam")
        .join("Contents")
        .join("MacOS")
}

#[cfg(any(windows, target_os = "macos"))]
fn set_client_lock_file(config_root: &Path, locked: bool) -> Result<(), String> {
    let config_path = config_root.join("steam.cfg");
    let existing = if config_path.exists() {
        fs::read_to_string(&config_path).map_err(|err| format!("读取 steam.cfg 失败：{err}"))?
    } else {
        String::new()
    };
    let mut lines = remove_client_lock_lines(&existing);

    if locked {
        lines.push("BootStrapperInhibitAll=enable".to_string());
        lines.push("BootStrapperForceSelfUpdate=disable".to_string());
    }

    if !locked && lines.is_empty() {
        if config_path.exists() {
            fs::remove_file(&config_path).map_err(|err| format!("移除 steam.cfg 失败：{err}"))?;
        }
        return Ok(());
    }

    let mut next = lines.join("\n");
    next.push('\n');
    fs::write(&config_path, next).map_err(|err| {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            "写入 steam.cfg 失败：拒绝访问。请先完全退出 Steam，并检查 Steam 目录写入权限。"
                .to_string()
        } else {
            format!("写入 steam.cfg 失败：{err}")
        }
    })
}

#[cfg(any(windows, target_os = "macos"))]
fn remove_client_lock_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| {
            let key = line
                .split_once('=')
                .map(|(left, _)| left.trim().to_ascii_lowercase());
            !matches!(
                key.as_deref(),
                Some("bootstrapperinhibitall") | Some("bootstrapperforceselfupdate")
            )
        })
        .map(ToString::to_string)
        .collect()
}

#[cfg(any(windows, target_os = "macos"))]
fn is_client_locked(config_root: &Path) -> bool {
    let Ok(content) = fs::read_to_string(config_root.join("steam.cfg")) else {
        return false;
    };
    has_config_value(&content, "BootStrapperInhibitAll", "enable")
        && has_config_value(&content, "BootStrapperForceSelfUpdate", "disable")
}

#[cfg(any(windows, target_os = "macos"))]
fn has_config_value(content: &str, key: &str, expected: &str) -> bool {
    content.lines().any(|line| {
        let Some((left, right)) = line.split_once('=') else {
            return false;
        };
        left.trim().eq_ignore_ascii_case(key) && right.trim().eq_ignore_ascii_case(expected)
    })
}

#[cfg(any(windows, target_os = "macos"))]
fn read_client_version(steam_root: &Path) -> Option<String> {
    read_package_files(steam_root)
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|content| parse_vdf_field(&content, "version"))
        .max_by_key(|value| value.parse::<u64>().ok())
}

#[cfg(windows)]
fn read_client_build_date(steam_root: &Path) -> Option<u64> {
    read_pe_timestamp(&steam_root.join("steamui.dll"))
        .or_else(|| read_pe_timestamp(&steam_root.join("steamclient64.dll")))
        .or_else(|| read_package_build_timestamp(steam_root))
}

#[cfg(windows)]
fn read_package_files(steam_root: &Path) -> Vec<PathBuf> {
    vec![
        steam_root
            .join("package")
            .join("steam_client_win64.installed"),
        steam_root
            .join("package")
            .join("steam_client_win32.installed"),
        steam_root
            .join("package")
            .join("steam_client_win64.manifest"),
        steam_root
            .join("package")
            .join("steam_client_win32.manifest"),
    ]
}

#[cfg(target_os = "macos")]
fn read_package_files(steam_root: &Path) -> Vec<PathBuf> {
    let package_root = client_config_root(steam_root).join("package");
    vec![
        package_root.join("steam_client_osx.manifest"),
        package_root.join("steam_client_signed_osx.manifest"),
        package_root.join("steam_client_signed-2_osx.manifest"),
    ]
}

#[cfg(target_os = "macos")]
fn read_client_build_date(steam_root: &Path) -> Option<u64> {
    read_client_version(steam_root)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| is_timestamp_like_value(*value))
}

#[cfg(windows)]
fn read_package_build_timestamp(steam_root: &Path) -> Option<u64> {
    read_package_files(steam_root)
        .iter()
        .filter_map(|path| {
            let content = fs::read_to_string(path).ok()?;
            parse_vdf_field(&content, "buildtime")
                .or_else(|| parse_vdf_field(&content, "build_time"))
                .or_else(|| parse_vdf_field(&content, "build date"))
                .or_else(|| parse_vdf_field(&content, "build_date"))
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| is_timestamp_like_value(*value))
        })
        .max()
}

#[cfg(any(windows, target_os = "macos"))]
fn parse_vdf_field(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('"') {
            continue;
        }
        let quoted: Vec<&str> = trimmed.split('"').skip(1).step_by(2).collect();
        if quoted.len() < 2 || !quoted[0].eq_ignore_ascii_case(key) {
            continue;
        }
        let value = quoted[1].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

#[cfg(windows)]
fn read_pe_timestamp(path: &Path) -> Option<u64> {
    let data = fs::read(path).ok()?;
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return None;
    }
    let pe_offset = u32::from_le_bytes(data[0x3c..0x40].try_into().ok()?) as usize;
    if data.len() < pe_offset + 12 || &data[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return None;
    }
    let timestamp = u32::from_le_bytes(data[pe_offset + 8..pe_offset + 12].try_into().ok()?) as u64;
    if is_timestamp_like_value(timestamp) {
        Some(timestamp)
    } else {
        None
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn is_timestamp_like_value(timestamp: u64) -> bool {
    (1_262_304_000..=4_102_444_800).contains(&timestamp)
}

#[cfg(windows)]
fn remove_component_file(target: &Path, file_name: &str) -> Result<(), String> {
    if !target.exists() {
        return Ok(());
    }

    fs::remove_file(target).map_err(|err| {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "移除 {file_name} 失败：拒绝访问。请完全退出 Steam 后重试；如果仍失败，以管理员身份运行 wuhu。"
            )
        } else {
            format!("移除 {file_name} 失败：{err}")
        }
    })
}

#[cfg(windows)]
fn opensteamtool_data_root() -> Option<PathBuf> {
    env::var_os("OST_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("LOCALAPPDATA")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .map(|path| path.join("OpenSteamTool"))
        })
}

#[cfg(target_os = "macos")]
fn opensteamtool_data_root() -> Option<PathBuf> {
    env::var_os("OST_DATA_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home_dir().map(|home| {
                home.join("Library")
                    .join("Application Support")
                    .join("OpenSteamTool")
            })
        })
}

#[cfg(windows)]
fn opensteamtool_binary_path() -> Option<PathBuf> {
    let data_root = opensteamtool_data_root()?;

    Some(data_root.join("bin").join(OPENSTEAMTOOL_DLL_NAME))
}

#[cfg(windows)]
fn cloud_redirect_plugin_path() -> Option<PathBuf> {
    Some(
        opensteamtool_data_root()?
            .join("plugins")
            .join(CLOUD_REDIRECT_DLL_NAME),
    )
}

#[cfg(windows)]
fn opensteamtool_config_path() -> Option<PathBuf> {
    Some(opensteamtool_data_root()?.join("opensteamtool.toml"))
}

#[cfg(windows)]
fn cloud_redirect_config_path() -> Option<PathBuf> {
    env::var_os("APPDATA")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join("CloudRedirect").join("config.json"))
}

#[cfg(target_os = "macos")]
fn opensteamtool_binary_path() -> Option<PathBuf> {
    Some(
        opensteamtool_data_root()?
            .join("bin")
            .join(OPENSTEAMTOOL_DYLIB_NAME),
    )
}

#[cfg(target_os = "macos")]
fn cloud_redirect_plugin_path() -> Option<PathBuf> {
    Some(
        opensteamtool_data_root()?
            .join("plugins")
            .join(CLOUD_REDIRECT_DYLIB_NAME),
    )
}

#[cfg(target_os = "macos")]
fn opensteamtool_config_path() -> Option<PathBuf> {
    Some(opensteamtool_data_root()?.join("opensteamtool.toml"))
}

#[cfg(target_os = "macos")]
fn cloud_redirect_config_path() -> Option<PathBuf> {
    home_dir().map(|home| {
        home.join("Library")
            .join("Application Support")
            .join("CloudRedirect")
            .join("config.json")
    })
}

#[cfg(target_os = "macos")]
fn macos_steam_executable(store: &AppStore) -> Option<PathBuf> {
    let configured = store
        .settings
        .steam_path
        .as_deref()
        .and_then(input_path)
        .and_then(|path| macos_app_bundle_path(&path))
        .map(|app| app.join("Contents").join("MacOS").join("steam_osx"));
    if configured.as_ref().is_some_and(|path| path.is_file()) {
        return configured;
    }

    let mut candidates = vec![PathBuf::from(
        "/Applications/Steam.app/Contents/MacOS/steam_osx",
    )];
    if let Some(home) = home_dir() {
        candidates.push(
            home.join("Applications")
                .join("Steam.app")
                .join("Contents")
                .join("MacOS")
                .join("steam_osx"),
        );
    }

    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(target_os = "macos")]
fn macos_steam_backup_path(steam_executable: &Path) -> PathBuf {
    let mut backup = steam_executable.as_os_str().to_os_string();
    backup.push(".ostbak");
    PathBuf::from(backup)
}

#[cfg(target_os = "macos")]
fn macos_launch_marker_path() -> Option<PathBuf> {
    Some(opensteamtool_data_root()?.join("wuhu-steam.pid"))
}

#[cfg(target_os = "macos")]
fn macos_launch_marker_pid() -> Option<u32> {
    fs::read_to_string(macos_launch_marker_path()?)
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn parse_macos_process_ids(output: &str) -> Vec<u32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

#[cfg(target_os = "macos")]
fn macos_steam_process_ids() -> Result<Vec<u32>, String> {
    let output = Command::new("/usr/bin/pgrep")
        .args(["-x", "steam_osx"])
        .output()
        .map_err(|err| format!("检查 Steam 运行状态失败：{err}"))?;

    if output.status.success() {
        return Ok(parse_macos_process_ids(&String::from_utf8_lossy(
            &output.stdout,
        )));
    }
    if output.status.code() == Some(1) {
        return Ok(Vec::new());
    }

    Err(format!(
        "检查 Steam 运行状态失败：{}",
        command_error_detail(&output)
    ))
}

#[cfg(target_os = "macos")]
fn macos_launched_via_wuhu() -> bool {
    let Some(marker_path) = macos_launch_marker_path() else {
        return false;
    };
    let Some(pid) = macos_launch_marker_pid() else {
        let _ = fs::remove_file(marker_path);
        return false;
    };

    match macos_steam_process_ids() {
        Ok(pids) if pids.contains(&pid) => true,
        Ok(_) => {
            let _ = fs::remove_file(marker_path);
            false
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
fn macos_copy_preserving(source: &Path, destination: &Path, action: &str) -> Result<(), String> {
    let output = Command::new("/bin/cp")
        .arg("-p")
        .arg(source)
        .arg(destination)
        .output()
        .map_err(|err| format!("{action}失败：{err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{action}失败：{}", command_error_detail(&output)))
    }
}

#[cfg(target_os = "macos")]
fn macos_codesign(path: &Path, action: &str) -> Result<(), String> {
    let output = Command::new("/usr/bin/codesign")
        .args(["-f", "-s", "-"])
        .arg(path)
        .output()
        .map_err(|err| format!("{action}失败：{err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{action}失败：{}", command_error_detail(&output)))
    }
}

#[cfg(target_os = "macos")]
fn macos_is_ad_hoc_signed(path: &Path) -> Result<bool, String> {
    let output = Command::new("/usr/bin/codesign")
        .arg("-dvvv")
        .arg(path)
        .output()
        .map_err(|err| format!("检查 steam_osx 签名失败：{err}"))?;

    if !output.status.success() {
        return Err(format!(
            "检查 steam_osx 签名失败：{}",
            command_error_detail(&output)
        ));
    }

    Ok(parse_macos_ad_hoc_signature(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

#[cfg(target_os = "macos")]
fn parse_macos_ad_hoc_signature(output: &str) -> bool {
    output
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("Signature=adhoc"))
}

#[cfg(target_os = "macos")]
fn command_error_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    if detail.is_empty() {
        output.status.to_string()
    } else {
        detail.to_string()
    }
}

#[cfg(target_os = "macos")]
fn remove_macos_file_if_exists(path: &Path, label: &str) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(path).map_err(|err| format!("移除 {label} 失败：{err}"))
}

#[cfg(target_os = "macos")]
fn macos_component_matches(path: &Path, embedded: &[u8]) -> bool {
    fs::read(path)
        .map(|bytes| bytes.as_slice() == embedded)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn write_macos_component(target: &Path, name: &str, embedded: &[u8]) -> Result<(), String> {
    if macos_component_matches(target, embedded) {
        return Ok(());
    }

    fs::write(target, embedded).map_err(|err| format!("部署 {name} 失败：{err}"))
}

#[cfg(windows)]
fn windows_component_targets(steam_root: &Path, core_path: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut targets = vec![(core_path.to_path_buf(), OPENSTEAMTOOL_DLL_NAME)];
    for file_name in PROXY_DLL_NAMES {
        targets.push((steam_root.join(file_name), file_name));
    }
    targets
}

#[cfg(windows)]
fn windows_components_present(steam_root: &Path, core_path: &Path) -> bool {
    windows_component_targets(steam_root, core_path)
        .into_iter()
        .all(|(path, _)| path.exists())
        && cloud_redirect_plugin_path().is_some_and(|path| path.is_file())
}

#[cfg(windows)]
fn tool_file_matches_embedded(path: &Path, file_name: &str) -> bool {
    let Some(embedded) = embedded_tool_file(file_name) else {
        return false;
    };
    fs::read(path)
        .map(|bytes| bytes.as_slice() == embedded)
        .unwrap_or(false)
}

#[cfg(windows)]
fn write_embedded_tool_file(target: &Path, file_name: &str) -> Result<(), String> {
    let bytes = embedded_tool_file(file_name)
        .ok_or_else(|| format!("内置资源缺少 {file_name}，请重新构建 wuhu"))?;
    if tool_file_matches_embedded(target, file_name) {
        return Ok(());
    }
    fs::write(target, bytes).map_err(|err| format!("安装 {file_name} 失败：{err}"))
}

#[cfg(windows)]
fn write_cloud_redirect_if_missing(target: &Path) -> Result<(), String> {
    if target.is_file() {
        return Ok(());
    }

    fs::write(target, EMBEDDED_CLOUD_REDIRECT_DLL)
        .map_err(|err| format!("安装 {CLOUD_REDIRECT_DLL_NAME} 失败：{err}"))
}

#[cfg(any(windows, target_os = "macos"))]
fn enable_opensteamtool_cloud() -> Result<(), String> {
    let config_path =
        opensteamtool_config_path().ok_or_else(|| "无法确定 OpenSteamTool 配置目录".to_string())?;
    let existing = if config_path.exists() {
        fs::read_to_string(&config_path)
            .map_err(|err| format!("读取 opensteamtool.toml 失败：{err}"))?
    } else {
        String::new()
    };
    let updated = set_cloud_enabled(&existing);
    if updated == existing {
        return Ok(());
    }

    fs::write(&config_path, updated).map_err(|err| format!("写入 opensteamtool.toml 失败：{err}"))
}

#[cfg(any(windows, target_os = "macos", test))]
fn set_cloud_enabled(content: &str) -> String {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines = content
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();

    let cloud_start = lines
        .iter()
        .position(|line| line.split('#').next().unwrap_or("").trim() == "[cloud]");

    if let Some(start) = cloud_start {
        let end = lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, line)| {
                let value = line.split('#').next().unwrap_or("").trim();
                value.starts_with('[') && value.ends_with(']')
            })
            .map(|(index, _)| index)
            .unwrap_or(lines.len());

        let enabled_line = (start + 1..end).find(|&index| {
            lines[index]
                .split('#')
                .next()
                .and_then(|line| line.split_once('='))
                .is_some_and(|(key, _)| key.trim() == "enabled")
        });

        if let Some(index) = enabled_line {
            let indentation = lines[index]
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            let comment = lines[index]
                .find('#')
                .map(|offset| format!(" {}", &lines[index][offset..]))
                .unwrap_or_default();
            lines[index] = format!("{indentation}enabled = true{comment}");
        } else {
            lines.insert(start + 1, "enabled = true".to_string());
        }
    } else {
        if lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[cloud]".to_string());
        lines.push("enabled = true".to_string());
    }

    let mut updated = lines.join(newline);
    updated.push_str(newline);
    updated
}

#[cfg(any(windows, target_os = "macos"))]
fn configure_cloud_redirect_local_folder() -> Result<(), String> {
    let config_path = cloud_redirect_config_path()
        .ok_or_else(|| "无法确定 CloudRedirect 配置目录".to_string())?;
    let sync_path = crate::store::portable_data_dir()?.join("cloudredirect");
    fs::create_dir_all(&sync_path)
        .map_err(|err| format!("创建 CloudRedirect 本地同步目录失败：{err}"))?;
    let sync_path = sync_path
        .to_str()
        .ok_or_else(|| "CloudRedirect 本地同步目录不是有效的 Unicode 路径".to_string())?;

    let mut config = if config_path.exists() {
        let existing = fs::read_to_string(&config_path)
            .map_err(|err| format!("读取 CloudRedirect config.json 失败：{err}"))?;
        serde_json::from_str(&existing)
            .map_err(|err| format!("解析 CloudRedirect config.json 失败：{err}"))?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };
    set_cloud_redirect_local_config(&mut config, sync_path, cfg!(windows))?;

    if let Some(config_dir) = config_path.parent() {
        fs::create_dir_all(config_dir)
            .map_err(|err| format!("创建 CloudRedirect 配置目录失败：{err}"))?;
    }
    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|err| format!("序列化 CloudRedirect config.json 失败：{err}"))?;
    fs::write(&config_path, format!("{serialized}\n"))
        .map_err(|err| format!("写入 CloudRedirect config.json 失败：{err}"))
}

#[cfg(any(windows, target_os = "macos", test))]
fn set_cloud_redirect_local_config(
    config: &mut serde_json::Value,
    sync_path: &str,
    enable_auto_update: bool,
) -> Result<(), String> {
    let object = config
        .as_object_mut()
        .ok_or_else(|| "CloudRedirect config.json 的根节点必须是对象".to_string())?;
    object.insert("provider".to_string(), serde_json::json!("folder"));
    object.insert("sync_path".to_string(), serde_json::json!(sync_path));
    if enable_auto_update {
        object.insert("auto_update_dll".to_string(), serde_json::json!(true));
    }
    Ok(())
}

#[cfg(windows)]
fn embedded_tool_file(file_name: &str) -> Option<&'static [u8]> {
    EMBEDDED_TOOL_FILES
        .iter()
        .find(|file| file.name.eq_ignore_ascii_case(file_name))
        .map(|file| file.bytes)
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod tests {
    use super::{
        has_config_value, remove_client_lock_lines, set_cloud_enabled,
        set_cloud_redirect_local_config,
    };

    #[test]
    fn client_lock_config_is_case_insensitive_and_preserves_other_settings() {
        let content =
            "Universe=Public\nbootstrapperinhibitall=ENABLE\nBootStrapperForceSelfUpdate=disable\n";

        assert!(has_config_value(
            content,
            "BootStrapperInhibitAll",
            "enable"
        ));
        assert_eq!(remove_client_lock_lines(content), vec!["Universe=Public"]);
    }

    #[test]
    fn cloud_config_is_created_when_opensteamtool_config_is_empty() {
        assert_eq!(set_cloud_enabled(""), "[cloud]\nenabled = true\n");
    }

    #[test]
    fn cloud_config_is_appended_without_changing_other_sections() {
        let content = "[log]\nlevel = \"info\"\n";

        assert_eq!(
            set_cloud_enabled(content),
            "[log]\nlevel = \"info\"\n\n[cloud]\nenabled = true\n"
        );
    }

    #[test]
    fn cloud_config_enables_existing_section_and_preserves_comment_style() {
        let content = "[cloud]\r\n  enabled = false # user setting\r\n\r\n[remote]\r\n";

        assert_eq!(
            set_cloud_enabled(content),
            "[cloud]\r\n  enabled = true # user setting\r\n\r\n[remote]\r\n"
        );
    }

    #[test]
    fn cloud_redirect_local_config_preserves_unmanaged_fields() {
        let mut config = serde_json::json!({
            "provider": "gdrive",
            "token_paths": { "gdrive": "tokens.json" },
            "sync_achievements": true
        });

        set_cloud_redirect_local_config(&mut config, r"C:\Apps\wuhu\data\cloudredirect", true)
            .expect("CloudRedirect config should be updated");

        assert_eq!(config["provider"], "folder");
        assert_eq!(config["sync_path"], r"C:\Apps\wuhu\data\cloudredirect");
        assert_eq!(config["auto_update_dll"], true);
        assert_eq!(config["token_paths"]["gdrive"], "tokens.json");
        assert_eq!(config["sync_achievements"], true);
    }

    #[test]
    fn cloud_redirect_macos_config_does_not_enable_windows_updater() {
        let mut config = serde_json::json!({});

        set_cloud_redirect_local_config(
            &mut config,
            "/Users/test/Library/Application Support/wuhu/cloudredirect",
            false,
        )
        .expect("CloudRedirect config should be updated");

        assert_eq!(config["provider"], "folder");
        assert_eq!(
            config["sync_path"],
            "/Users/test/Library/Application Support/wuhu/cloudredirect"
        );
        assert!(config.get("auto_update_dll").is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_client_config_uses_the_active_app_bundle() {
        let config_root = super::client_config_root(std::path::Path::new("/Steam"));

        assert_eq!(
            config_root,
            std::path::Path::new("/Steam/Steam.AppBundle/Steam/Contents/MacOS")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_steam_executable_prefers_the_configured_app_bundle() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let test_root =
            std::env::temp_dir().join(format!("wuhu-steam-path-{}-{unique}", std::process::id()));
        let app_path = test_root.join("Custom").join("Steam.app");
        let executable = app_path.join("Contents").join("MacOS").join("steam_osx");
        std::fs::create_dir_all(
            executable
                .parent()
                .expect("executable should have a parent"),
        )
        .expect("test Steam.app directory should be created");
        std::fs::write(&executable, b"test").expect("test steam_osx should be created");

        let mut store = crate::models::AppStore::default();
        store.settings.steam_path = Some(app_path.to_string_lossy().into_owned());

        assert_eq!(super::macos_steam_executable(&store), Some(executable));
        std::fs::remove_dir_all(test_root).expect("test directory should be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_process_ids_ignore_empty_and_invalid_lines() {
        assert_eq!(
            super::parse_macos_process_ids("123\n\ninvalid\n456\n"),
            vec![123, 456]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_signature_parser_distinguishes_ad_hoc_and_valve_signatures() {
        assert!(super::parse_macos_ad_hoc_signature(
            "Identifier=com.valvesoftware.steam\nSignature=adhoc\nTeamIdentifier=not set\n"
        ));
        assert!(!super::parse_macos_ad_hoc_signature(
            "Authority=Developer ID Application: Valve Corporation (MXGJJ98X76)\n\
             TeamIdentifier=MXGJJ98X76\n"
        ));
    }
}
