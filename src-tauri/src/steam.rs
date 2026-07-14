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

#[cfg(target_os = "macos")]
const OPENSTEAMTOOL_DYLIB_NAME: &str = "libOpenSteamTool.dylib";

#[cfg(all(target_os = "macos", debug_assertions))]
const EMBEDDED_OPENSTEAMTOOL_DYLIB: &[u8] =
    include_bytes!("../../resources/opensteamtool/macos/debug/libOpenSteamTool.dylib");

#[cfg(all(target_os = "macos", not(debug_assertions)))]
const EMBEDDED_OPENSTEAMTOOL_DYLIB: &[u8] =
    include_bytes!("../../resources/opensteamtool/macos/release/libOpenSteamTool.dylib");

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

        if let Some(binary_dir) = core_path.parent() {
            fs::create_dir_all(binary_dir)
                .map_err(|err| format!("创建 OpenSteamTool bin 目录失败：{err}"))?;
        }

        write_embedded_tool_file(&core_path, OPENSTEAMTOOL_DLL_NAME)?;

        for file_name in PROXY_DLL_NAMES {
            write_embedded_tool_file(&steam_root.join(file_name), file_name)?;
        }

        let legacy_core_path = steam_root.join(OPENSTEAMTOOL_DLL_NAME);
        if legacy_core_path.exists() {
            remove_component_file(&legacy_core_path, OPENSTEAMTOOL_DLL_NAME)?;
        }

        Ok(())
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
        let binary_dir = dylib_path
            .parent()
            .ok_or_else(|| "OpenSteamTool bin 目录无效".to_string())?;
        fs::create_dir_all(binary_dir)
            .map_err(|err| format!("创建 OpenSteamTool bin 目录失败：{err}"))?;

        let needs_deploy = fs::read(&dylib_path)
            .map(|bytes| bytes != EMBEDDED_OPENSTEAMTOOL_DYLIB)
            .unwrap_or(true);
        if needs_deploy {
            fs::write(&dylib_path, EMBEDDED_OPENSTEAMTOOL_DYLIB)
                .map_err(|err| format!("部署 {OPENSTEAMTOOL_DYLIB_NAME} 失败：{err}"))?;
        }

        let backup_path = macos_steam_backup_path(&steam_executable);
        if !backup_path.exists() {
            macos_copy_preserving(&steam_executable, &backup_path, "备份 steam_osx")?;
        } else if !macos_is_ad_hoc_signed(&steam_executable)? {
            macos_copy_preserving(&steam_executable, &backup_path, "更新 steam_osx 备份")?;
        }

        macos_codesign(&steam_executable, "重签 steam_osx")?;
        macos_codesign(&dylib_path, "签名 libOpenSteamTool.dylib")?;

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
        };
    }

    #[cfg(windows)]
    {
        let installed = configured_root(store)
            .zip(opensteamtool_binary_path())
            .map(|(steam_root, core_path)| {
                core_path.exists()
                    && PROXY_DLL_NAMES
                        .iter()
                        .all(|name| steam_root.join(name).exists())
            })
            .unwrap_or(false);

        InstallStatus {
            installed,
            supported: true,
            launch_required: false,
            launched_via_wuhu: false,
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

#[cfg(target_os = "macos")]
fn opensteamtool_binary_path() -> Option<PathBuf> {
    Some(
        opensteamtool_data_root()?
            .join("bin")
            .join(OPENSTEAMTOOL_DYLIB_NAME),
    )
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

#[cfg(windows)]
fn write_embedded_tool_file(target: &Path, file_name: &str) -> Result<(), String> {
    let bytes = embedded_tool_file(file_name)
        .ok_or_else(|| format!("内置资源缺少 {file_name}，请重新构建 wuhu"))?;
    fs::write(target, bytes).map_err(|err| format!("安装 {file_name} 失败：{err}"))
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
    use super::{has_config_value, remove_client_lock_lines};

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
