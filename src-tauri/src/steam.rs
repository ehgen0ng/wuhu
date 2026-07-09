use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::fs;

use crate::models::{AppStore, InstallStatus, SteamClientStatus};

#[cfg(windows)]
const DLL_NAMES: [&str; 3] = ["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"];

#[cfg(windows)]
struct EmbeddedToolFile {
    name: &'static str,
    bytes: &'static [u8],
}

#[cfg(windows)]
const EMBEDDED_TOOL_FILES: [EmbeddedToolFile; 3] = [
    EmbeddedToolFile {
        name: "dwmapi.dll",
        bytes: include_bytes!("../../resources/opensteamtool/dwmapi.dll"),
    },
    EmbeddedToolFile {
        name: "xinput1_4.dll",
        bytes: include_bytes!("../../resources/opensteamtool/xinput1_4.dll"),
    },
    EmbeddedToolFile {
        name: "OpenSteamTool.dll",
        bytes: include_bytes!("../../resources/opensteamtool/OpenSteamTool.dll"),
    },
];

pub(crate) fn detect_path() -> Option<String> {
    detect_path_candidates()
        .into_iter()
        .find_map(|path| normalize_root_path(&path))
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
    if let Some(root) = default_macos_data_root() {
        candidates.push(root);
    }
    candidates.push(PathBuf::from("/Applications/Steam.app"));
    if let Some(home) = home_dir() {
        candidates.push(home.join("Applications").join("Steam.app"));
    }
    candidates
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect_path_candidates() -> Vec<PathBuf> {
    Vec::new()
}

pub(crate) fn normalize_path(path: &str) -> Option<String> {
    normalize_root_path(&input_path(path)?).and_then(|path| path_to_string(&path))
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
    cfg!(windows)
}

pub(crate) fn package_sync_root(store: &AppStore) -> Option<PathBuf> {
    supports_package_sync()
        .then(|| configured_root(store))
        .flatten()
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
    path.ancestors().any(|ancestor| {
        file_name_eq(ancestor, "Steam.app")
            && ancestor
                .join("Contents")
                .join("MacOS")
                .join("steam_osx")
                .exists()
    })
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

        for file_name in DLL_NAMES {
            let target = steam_root.join(file_name);
            let bytes = embedded_tool_file(file_name)
                .ok_or_else(|| format!("内置资源缺少 {file_name}，请重新构建 wuhu"))?;
            fs::write(&target, bytes).map_err(|err| format!("安装 {file_name} 失败：{err}"))?;
        }

        Ok(())
    }
}

pub(crate) fn restore_opensteamtool(store: &AppStore) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = store;
        return Err("组件恢复目前只支持 Windows Steam 客户端".to_string());
    }

    #[cfg(windows)]
    {
        let steam_root = configured_root(store).ok_or_else(|| "请先设置 Steam 路径".to_string())?;

        let mut errors = Vec::new();
        for file_name in DLL_NAMES {
            if let Err(err) = remove_component_file(&steam_root.join(file_name), file_name) {
                errors.push(err);
            }
        }
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }

        Ok(())
    }
}

pub(crate) fn set_client_version_locked(store: &AppStore, locked: bool) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (store, locked);
        return Err("Steam 客户端版本锁定目前只支持 Windows Steam 客户端".to_string());
    }

    #[cfg(windows)]
    {
        let steam_root = configured_root(store).ok_or_else(|| "请先设置 Steam 路径".to_string())?;

        set_client_lock_file(&steam_root, locked)
    }
}

pub(crate) fn install_status(store: &AppStore) -> InstallStatus {
    #[cfg(not(windows))]
    {
        let _ = store;
        return InstallStatus {
            installed: false,
            supported: false,
        };
    }

    #[cfg(windows)]
    {
        let installed = configured_root(store)
            .map(|path| DLL_NAMES.iter().all(|name| path.join(name).exists()))
            .unwrap_or(false);

        InstallStatus {
            installed,
            supported: true,
        }
    }
}

pub(crate) fn client_status(store: &AppStore) -> SteamClientStatus {
    #[cfg(not(windows))]
    {
        let _ = store;
        return SteamClientStatus {
            version: None,
            client_build_date: None,
            locked: false,
        };
    }

    #[cfg(windows)]
    {
        let Some(steam_root) = configured_root(store) else {
            return SteamClientStatus {
                version: None,
                client_build_date: None,
                locked: false,
            };
        };

        SteamClientStatus {
            version: read_client_version(&steam_root),
            client_build_date: read_client_build_date(&steam_root),
            locked: is_client_locked(&steam_root),
        }
    }
}

#[cfg(windows)]
fn set_client_lock_file(steam_root: &Path, locked: bool) -> Result<(), String> {
    let config_path = steam_root.join("steam.cfg");
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
            "写入 steam.cfg 失败：拒绝访问。请先完全退出 Steam，必要时以管理员身份运行 wuhu。"
                .to_string()
        } else {
            format!("写入 steam.cfg 失败：{err}")
        }
    })
}

#[cfg(windows)]
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

#[cfg(windows)]
fn is_client_locked(steam_root: &Path) -> bool {
    let Ok(content) = fs::read_to_string(steam_root.join("steam.cfg")) else {
        return false;
    };
    has_config_value(&content, "BootStrapperInhibitAll", "enable")
        && has_config_value(&content, "BootStrapperForceSelfUpdate", "disable")
}

#[cfg(windows)]
fn has_config_value(content: &str, key: &str, expected: &str) -> bool {
    content.lines().any(|line| {
        let Some((left, right)) = line.split_once('=') else {
            return false;
        };
        left.trim().eq_ignore_ascii_case(key) && right.trim().eq_ignore_ascii_case(expected)
    })
}

#[cfg(windows)]
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
fn read_package_files(steam_root: &Path) -> [PathBuf; 4] {
    [
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

#[cfg(windows)]
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

#[cfg(windows)]
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
fn embedded_tool_file(file_name: &str) -> Option<&'static [u8]> {
    EMBEDDED_TOOL_FILES
        .iter()
        .find(|file| file.name.eq_ignore_ascii_case(file_name))
        .map(|file| file.bytes)
}
