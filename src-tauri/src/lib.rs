use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use zip::ZipArchive;

const DLL_NAMES: [&str; 3] = ["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"];
const STORE_FILE: &str = "state.json";

struct EmbeddedToolFile {
    name: &'static str,
    bytes: &'static [u8],
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppStore {
    settings: AppSettings,
    packages: Vec<PackageItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    steam_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageItem {
    id: String,
    title: String,
    app_id: Option<u32>,
    lua_file_name: String,
    manifest_files: Vec<String>,
    source_zip_name: String,
    enabled: bool,
    imported_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    settings: AppSettings,
    packages: Vec<PackageItem>,
    install_status: InstallStatus,
    steam_client: SteamClientStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallStatus {
    installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamClientStatus {
    version: Option<String>,
    client_build_date: Option<u64>,
    locked: bool,
}

#[tauri::command]
fn get_initial_state(app: AppHandle) -> Result<AppState, String> {
    let mut store = load_store()?;
    let mut changed = false;
    if store.settings.steam_path.is_none() {
        store.settings.steam_path = detect_steam_path_internal();
        changed = store.settings.steam_path.is_some();
    }
    changed |= reconcile_packages_with_steam(&mut store)?;
    if changed {
        save_store(&store)?;
    }
    build_state(&app, store)
}

#[tauri::command]
fn detect_steam_path() -> Option<String> {
    detect_steam_path_internal()
}

#[tauri::command]
fn set_steam_path(app: AppHandle, path: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let previous_path = store.settings.steam_path.clone();
    let trimmed = path.trim();
    let next_path = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    let path_changed = previous_path != next_path;
    store.settings.steam_path = next_path;

    if path_changed {
        for package in &mut store.packages {
            package.enabled = false;
        }
    }

    save_store(&store)?;
    build_state(&app, store)
}

#[tauri::command]
fn import_package_from_bytes(
    app: AppHandle,
    file_name: String,
    data_base64: String,
) -> Result<AppState, String> {
    let bytes = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("zip 数据解码失败：{err}"))?;
    let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice()))
        .map_err(|err| format!("zip 读取失败：{err}"))?;

    let lua_index = find_entry_index(&mut archive, ".lua")?
        .ok_or_else(|| "压缩包里没有找到 .lua 文件".to_string())?;
    let lua_entry_name = archive
        .by_index(lua_index)
        .map_err(|err| format!("读取 Lua 条目失败：{err}"))?
        .name()
        .to_string();
    let lua_file_name = file_name_only(&lua_entry_name)?;
    let mut lua_content = String::new();
    archive
        .by_index(lua_index)
        .map_err(|err| format!("读取 Lua 文件失败：{err}"))?
        .read_to_string(&mut lua_content)
        .map_err(|err| format!("Lua 文件不是有效文本：{err}"))?;

    let zip_stem = Path::new(&file_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("package");
    let metadata = parse_package_metadata(&lua_file_name, &lua_content, zip_stem);
    let package_id = sanitize_id(&metadata.id);
    if package_id.is_empty() {
        return Err("无法生成包 ID".to_string());
    }

    let root = portable_data_dir()?;
    let package_dir = root.join("packages").join(&package_id);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|err| format!("清理旧包失败：{err}"))?;
    }
    let manifest_dir = package_dir.join("manifests");
    fs::create_dir_all(&manifest_dir).map_err(|err| format!("创建包目录失败：{err}"))?;
    fs::write(package_dir.join("source.lua"), lua_content.as_bytes())
        .map_err(|err| format!("保存 Lua 失败：{err}"))?;

    let mut manifest_files = Vec::new();
    for index in 0..archive.len() {
        let entry_name = {
            let file = archive
                .by_index(index)
                .map_err(|err| format!("读取 zip 条目失败：{err}"))?;
            file.name().to_string()
        };
        if !entry_name.to_ascii_lowercase().ends_with(".manifest") {
            continue;
        }

        let safe_name = file_name_only(&entry_name)?;
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("读取 manifest 失败：{err}"))?;
        let mut manifest = Vec::new();
        file.read_to_end(&mut manifest)
            .map_err(|err| format!("读取 manifest 内容失败：{err}"))?;
        fs::write(manifest_dir.join(&safe_name), manifest)
            .map_err(|err| format!("保存 manifest 失败：{err}"))?;
        manifest_files.push(safe_name);
    }
    manifest_files.sort();

    let mut store = load_store()?;
    let record = PackageItem {
        id: package_id.clone(),
        title: metadata.title,
        app_id: metadata.app_id,
        lua_file_name: format!("wuhu_{package_id}.lua"),
        manifest_files,
        source_zip_name: file_name,
        enabled: true,
        imported_at: now_seconds(),
    };
    let package_to_sync = record.clone();

    store.packages.retain(|package| package.id != package_id);
    store.packages.push(record);
    store
        .packages
        .sort_by(|left, right| left.title.cmp(&right.title));
    save_store(&store)?;
    sync_package_enabled(&store, &package_to_sync)?;
    build_state(&app, store)
}

#[tauri::command]
fn set_package_enabled(app: AppHandle, id: String, enabled: bool) -> Result<AppState, String> {
    let mut store = load_store()?;
    let package = store
        .packages
        .iter_mut()
        .find(|package| package.id == id)
        .ok_or_else(|| "没有找到这个清单".to_string())?;
    package.enabled = enabled;
    let package = package.clone();
    save_store(&store)?;
    sync_package_enabled(&store, &package)?;
    build_state(&app, store)
}

#[tauri::command]
fn delete_package(app: AppHandle, id: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let package = store
        .packages
        .iter()
        .find(|package| package.id == id)
        .cloned()
        .ok_or_else(|| "没有找到这个清单".to_string())?;

    if let Some(steam_path) = store.settings.steam_path.as_deref() {
        remove_active_lua(Path::new(steam_path), &package)?;
    }

    store.packages.retain(|item| item.id != id);
    save_store(&store)?;

    let package_dir = portable_data_dir()?.join("packages").join(&package.id);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|err| format!("删除本地包失败：{err}"))?;
    }

    build_state(&app, store)
}

#[tauri::command]
fn install_opensteamtool(app: AppHandle) -> Result<AppState, String> {
    let store = load_store()?;
    let steam_path = store
        .settings
        .steam_path
        .clone()
        .ok_or_else(|| "请先设置 Steam 路径".to_string())?;
    let steam_root = PathBuf::from(&steam_path);
    if !looks_like_steam_root(&steam_root) {
        return Err("Steam 路径不像 Steam 根目录，请检查后再安装".to_string());
    }

    for file_name in DLL_NAMES {
        let target = steam_root.join(file_name);
        let bytes = embedded_tool_file(file_name)
            .ok_or_else(|| format!("内置资源缺少 {file_name}，请重新构建 wuhu"))?;
        fs::write(&target, bytes).map_err(|err| format!("安装 {file_name} 失败：{err}"))?;
    }

    build_state(&app, store)
}

#[tauri::command]
fn restore_opensteamtool(app: AppHandle) -> Result<AppState, String> {
    let store = load_store()?;
    let steam_path = store
        .settings
        .steam_path
        .clone()
        .ok_or_else(|| "请先设置 Steam 路径".to_string())?;
    let steam_root = PathBuf::from(&steam_path);
    if !looks_like_steam_root(&steam_root) {
        return Err("Steam 路径不像 Steam 根目录，请检查后再恢复".to_string());
    }

    let mut errors = Vec::new();
    for file_name in DLL_NAMES {
        if let Err(err) = remove_component_file(&steam_root.join(file_name), file_name) {
            errors.push(err);
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    build_state(&app, store)
}

#[tauri::command]
fn set_steam_client_version_locked(app: AppHandle, locked: bool) -> Result<AppState, String> {
    let store = load_store()?;
    let steam_path = store
        .settings
        .steam_path
        .clone()
        .ok_or_else(|| "请先设置 Steam 路径".to_string())?;
    let steam_root = PathBuf::from(&steam_path);
    if !looks_like_steam_root(&steam_root) {
        return Err("Steam 路径不像 Steam 根目录，请检查后再操作".to_string());
    }

    set_steam_client_lock_file(&steam_root, locked)?;
    build_state(&app, store)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            detect_steam_path,
            set_steam_path,
            import_package_from_bytes,
            set_package_enabled,
            delete_package,
            install_opensteamtool,
            restore_opensteamtool,
            set_steam_client_version_locked
        ])
        .run(tauri::generate_context!())
        .expect("error while running wuhu");
}

#[derive(Debug)]
struct PackageMetadata {
    id: String,
    title: String,
    app_id: Option<u32>,
}

fn build_state(app: &AppHandle, store: AppStore) -> Result<AppState, String> {
    Ok(AppState {
        install_status: install_status(app, &store),
        steam_client: steam_client_status(&store),
        settings: store.settings,
        packages: store.packages,
    })
}

fn portable_data_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|err| format!("获取程序路径失败：{err}"))?;
    let base_dir = exe_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "获取程序目录失败".to_string())?;
    let path = base_dir.join("data");
    fs::create_dir_all(&path).map_err(|err| format!("创建应用数据目录失败：{err}"))?;
    Ok(path)
}

fn load_store() -> Result<AppStore, String> {
    let path = portable_data_dir()?.join(STORE_FILE);
    if !path.exists() {
        return Ok(AppStore::default());
    }
    let data = fs::read_to_string(&path).map_err(|err| format!("读取状态文件失败：{err}"))?;
    serde_json::from_str(&data).map_err(|err| format!("解析状态文件失败：{err}"))
}

fn save_store(store: &AppStore) -> Result<(), String> {
    let path = portable_data_dir()?.join(STORE_FILE);
    let data =
        serde_json::to_string_pretty(store).map_err(|err| format!("序列化状态失败：{err}"))?;
    fs::write(path, data).map_err(|err| format!("保存状态文件失败：{err}"))
}

fn reconcile_packages_with_steam(store: &mut AppStore) -> Result<bool, String> {
    let Some(steam_path) = store.settings.steam_path.as_deref() else {
        return Ok(set_all_packages_enabled(store, false));
    };
    let steam_root = Path::new(steam_path);
    if !looks_like_steam_root(steam_root) {
        return Ok(set_all_packages_enabled(store, false));
    }

    let app_root = portable_data_dir()?;
    let mut changed = false;
    for package in &mut store.packages {
        let actual_enabled = package_matches_steam(&app_root, steam_root, package);
        if package.enabled != actual_enabled {
            package.enabled = actual_enabled;
            changed = true;
        }
    }

    Ok(changed)
}

fn set_all_packages_enabled(store: &mut AppStore, enabled: bool) -> bool {
    let mut changed = false;
    for package in &mut store.packages {
        if package.enabled != enabled {
            package.enabled = enabled;
            changed = true;
        }
    }
    changed
}

fn package_matches_steam(app_root: &Path, steam_root: &Path, package: &PackageItem) -> bool {
    let package_dir = app_root.join("packages").join(&package.id);
    let local_lua = package_dir.join("source.lua");
    let steam_lua = steam_root
        .join("config")
        .join("lua")
        .join(&package.lua_file_name);
    if !files_match(&local_lua, &steam_lua) {
        return false;
    }

    for manifest_name in &package.manifest_files {
        let local_manifest = package_dir.join("manifests").join(manifest_name);
        let steam_manifest = steam_root.join("depotcache").join(manifest_name);
        if !files_match(&local_manifest, &steam_manifest) {
            return false;
        }
    }

    true
}

fn files_match(expected: &Path, actual: &Path) -> bool {
    let Ok(expected_meta) = fs::metadata(expected) else {
        return false;
    };
    let Ok(actual_meta) = fs::metadata(actual) else {
        return false;
    };
    if expected_meta.len() != actual_meta.len() {
        return false;
    }

    match (fs::read(expected), fs::read(actual)) {
        (Ok(expected_bytes), Ok(actual_bytes)) => expected_bytes == actual_bytes,
        _ => false,
    }
}

fn sync_package_enabled(store: &AppStore, package: &PackageItem) -> Result<(), String> {
    let Some(steam_path) = store.settings.steam_path.as_deref() else {
        return Ok(());
    };
    let steam_root = Path::new(steam_path);
    let root = portable_data_dir()?;

    if package.enabled {
        apply_package(&root, steam_root, package)
    } else {
        remove_active_lua(steam_root, package)
    }
}

fn apply_package(app_root: &Path, steam_root: &Path, package: &PackageItem) -> Result<(), String> {
    let lua_dir = steam_root.join("config").join("lua");
    let depotcache_dir = steam_root.join("depotcache");
    fs::create_dir_all(&lua_dir).map_err(|err| format!("创建 Lua 目录失败：{err}"))?;
    fs::create_dir_all(&depotcache_dir)
        .map_err(|err| format!("创建 depotcache 目录失败：{err}"))?;

    let package_dir = app_root.join("packages").join(&package.id);
    let lua = fs::read(package_dir.join("source.lua"))
        .map_err(|err| format!("读取 {} 的 Lua 失败：{err}", package.title))?;
    fs::write(lua_dir.join(&package.lua_file_name), lua)
        .map_err(|err| format!("写入启用 Lua 失败：{err}"))?;

    for manifest_name in &package.manifest_files {
        let source = package_dir.join("manifests").join(manifest_name);
        let target = depotcache_dir.join(manifest_name);
        fs::copy(&source, &target)
            .map_err(|err| format!("复制 manifest {manifest_name} 失败：{err}"))?;
    }

    Ok(())
}

fn remove_active_lua(steam_root: &Path, package: &PackageItem) -> Result<(), String> {
    let active_lua = steam_root
        .join("config")
        .join("lua")
        .join(&package.lua_file_name);
    if active_lua.exists() {
        fs::remove_file(&active_lua).map_err(|err| format!("移除启用 Lua 失败：{err}"))?;
    }
    Ok(())
}

fn install_status(_app: &AppHandle, store: &AppStore) -> InstallStatus {
    let installed = store
        .settings
        .steam_path
        .as_deref()
        .map(|path| {
            DLL_NAMES
                .iter()
                .all(|name| Path::new(path).join(name).exists())
        })
        .unwrap_or(false);

    InstallStatus { installed }
}

fn steam_client_status(store: &AppStore) -> SteamClientStatus {
    let Some(steam_path) = store.settings.steam_path.as_deref() else {
        return SteamClientStatus {
            version: None,
            client_build_date: None,
            locked: false,
        };
    };
    let steam_root = Path::new(steam_path);

    SteamClientStatus {
        version: read_steam_client_version(steam_root),
        client_build_date: read_steam_client_build_date(steam_root),
        locked: is_steam_client_locked(steam_root),
    }
}

fn set_steam_client_lock_file(steam_root: &Path, locked: bool) -> Result<(), String> {
    let config_path = steam_root.join("steam.cfg");
    let existing = if config_path.exists() {
        fs::read_to_string(&config_path).map_err(|err| format!("读取 steam.cfg 失败：{err}"))?
    } else {
        String::new()
    };
    let mut lines = remove_steam_client_lock_lines(&existing);

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

fn remove_steam_client_lock_lines(content: &str) -> Vec<String> {
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

fn is_steam_client_locked(steam_root: &Path) -> bool {
    let Ok(content) = fs::read_to_string(steam_root.join("steam.cfg")) else {
        return false;
    };
    has_config_value(&content, "BootStrapperInhibitAll", "enable")
        && has_config_value(&content, "BootStrapperForceSelfUpdate", "disable")
}

fn has_config_value(content: &str, key: &str, expected: &str) -> bool {
    content.lines().any(|line| {
        let Some((left, right)) = line.split_once('=') else {
            return false;
        };
        left.trim().eq_ignore_ascii_case(key) && right.trim().eq_ignore_ascii_case(expected)
    })
}

fn read_steam_client_version(steam_root: &Path) -> Option<String> {
    read_package_files(steam_root)
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|content| parse_vdf_field(&content, "version"))
        .max_by_key(|value| value.parse::<u64>().ok())
}

fn read_steam_client_build_date(steam_root: &Path) -> Option<u64> {
    read_pe_timestamp(&steam_root.join("steamui.dll"))
        .or_else(|| read_pe_timestamp(&steam_root.join("steamclient64.dll")))
        .or_else(|| read_package_build_timestamp(steam_root))
}

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

fn is_timestamp_like_value(timestamp: u64) -> bool {
    (1_262_304_000..=4_102_444_800).contains(&timestamp)
}

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

fn embedded_tool_file(file_name: &str) -> Option<&'static [u8]> {
    EMBEDDED_TOOL_FILES
        .iter()
        .find(|file| file.name.eq_ignore_ascii_case(file_name))
        .map(|file| file.bytes)
}

fn detect_steam_path_internal() -> Option<String> {
    let candidates = [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"];

    candidates
        .iter()
        .find(|path| looks_like_steam_root(Path::new(path)))
        .map(|path| path.to_string())
}

fn looks_like_steam_root(path: &Path) -> bool {
    path.join("steam.exe").exists() || path.join("Steam.exe").exists()
}

fn find_entry_index<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    extension: &str,
) -> Result<Option<usize>, String> {
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|err| format!("读取 zip 条目失败：{err}"))?;
        if file.name().to_ascii_lowercase().ends_with(extension) {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn file_name_only(path: &str) -> Result<String, String> {
    Path::new(path)
        .file_name()
        .and_then(OsStr::to_str)
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("zip 内文件名不安全：{path}"))
}

fn parse_package_metadata(
    lua_file_name: &str,
    lua_content: &str,
    zip_stem: &str,
) -> PackageMetadata {
    let app_id = parse_app_id_from_text(lua_file_name)
        .or_else(|| parse_first_addappid(lua_content))
        .or_else(|| parse_app_id_from_text(zip_stem));
    let id = app_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| zip_stem.to_string());
    let title = parse_title(lua_content).unwrap_or_else(|| id.clone());

    PackageMetadata { id, title, app_id }
}

fn parse_first_addappid(text: &str) -> Option<u32> {
    let marker = "addappid";
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)?;
    let rest = &text[start + marker.len()..];
    let open = rest.find('(')?;
    let after_open = &rest[open + 1..];
    let digits: String = after_open
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn parse_app_id_from_text(text: &str) -> Option<u32> {
    let digits: String = text.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_title(lua_content: &str) -> Option<String> {
    for line in lua_content.lines() {
        let trimmed = line.trim_start();
        let Some(comment) = trimmed.strip_prefix("--") else {
            continue;
        };
        let title = comment.trim();
        if title.is_empty() || is_metadata_comment(title) {
            continue;
        }
        return Some(title.to_string());
    }
    None
}

fn is_metadata_comment(comment: &str) -> bool {
    let lower = comment.to_ascii_lowercase();
    lower.contains("lua and manifest")
        || lower.starts_with("created")
        || lower.starts_with("website")
        || lower.starts_with("total")
        || lower.starts_with("main")
        || lower.contains("depot")
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
