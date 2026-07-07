use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use zip::ZipArchive;

const DLL_NAMES: [&str; 3] = ["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"];
const STORE_FILE: &str = "state.json";
const DEPOTBOX_DOWNLOAD_POLL_LIMIT: usize = 60;
const HTTP_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15";
const RELEASE_REPOSITORY: &str = "ehgen0ng/wuhu";

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
    #[serde(default)]
    steam_path: Option<String>,
    #[serde(default)]
    hubcap_api_key: Option<String>,
    #[serde(default)]
    depotbox_api_key: Option<String>,
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
    #[serde(default)]
    manifest_updated_at: Option<String>,
    #[serde(default)]
    manifest_file_size: Option<u64>,
    #[serde(default)]
    image_url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SteamSearchItem {
    #[serde(rename = "type", alias = "itemType", default)]
    item_type: String,
    name: String,
    id: u32,
    #[serde(rename = "tiny_image", alias = "tinyImage", default)]
    tiny_image: Option<String>,
    #[serde(default)]
    price: Option<SteamSearchPrice>,
    #[serde(default)]
    platforms: Option<SteamSearchPlatforms>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SteamSearchPrice {
    #[serde(default)]
    currency: String,
    #[serde(default)]
    initial: u32,
    #[serde(rename = "final")]
    #[serde(default)]
    final_price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SteamSearchPlatforms {
    windows: Option<bool>,
    mac: Option<bool>,
    linux: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SteamSearchResponse {
    items: Vec<SteamSearchItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheapSharkGame {
    #[serde(rename = "steamAppID")]
    steam_app_id: Option<String>,
    external: Option<String>,
    thumb: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItadSearchGame {
    slug: String,
    title: String,
    #[serde(default)]
    assets: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    detail: Option<String>,
    error: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestStatus {
    provider: String,
    app_id: u32,
    game_name: Option<String>,
    status: Option<String>,
    available: bool,
    manifest_file_exists: bool,
    update_in_progress: Option<bool>,
    needs_update: Option<bool>,
    file_size: Option<u64>,
    file_modified: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HubcapQuota {
    daily_usage: u64,
    daily_limit: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRelease {
    version: String,
    name: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct DepotBoxBatchAvailabilityRequest {
    appids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DepotBoxDownloadRequest {
    appid: String,
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
    import_package_archive(&app, file_name, bytes, None, None, None, None, None, true)
}

#[tauri::command]
fn set_hubcap_api_key(app: AppHandle, api_key: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let trimmed = api_key.trim();
    store.settings.hubcap_api_key = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    save_store(&store)?;
    build_state(&app, store)
}

#[tauri::command]
fn set_depotbox_api_key(app: AppHandle, api_key: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let trimmed = api_key.trim();
    store.settings.depotbox_api_key = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    save_store(&store)?;
    build_state(&app, store)
}

#[tauri::command]
async fn check_hubcap_manifest_statuses(app_ids: Vec<u32>) -> Result<Vec<ManifestStatus>, String> {
    let store = load_store()?;
    let api_key = hubcap_api_key(&store)?;
    let client = hubcap_client()?;
    let mut statuses = Vec::new();

    for app_id in app_ids.into_iter().filter(|app_id| *app_id > 0).take(24) {
        statuses.push(fetch_hubcap_manifest_status(&client, &api_key, app_id).await?);
    }

    Ok(statuses)
}

#[tauri::command]
async fn check_depotbox_manifest_statuses(
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let store = load_store()?;
    let api_key = depotbox_api_key(&store)?;
    let client = depotbox_client()?;
    fetch_depotbox_manifest_statuses(&client, &api_key, app_ids).await
}

#[tauri::command]
async fn get_hubcap_quota() -> Result<HubcapQuota, String> {
    let store = load_store()?;
    let api_key = hubcap_api_key(&store)?;
    let client = hubcap_client()?;
    fetch_hubcap_quota(&client, &api_key).await
}

#[tauri::command]
async fn get_latest_app_release() -> Result<AppRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建版本检查请求失败：{err}"))?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{RELEASE_REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("检查版本失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("检查版本失败：HTTP {}", response.status()));
    }

    let release = response
        .json::<GithubReleaseResponse>()
        .await
        .map_err(|err| format!("解析版本信息失败：{err}"))?;
    let version = release.tag_name.trim().trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err("版本信息为空".to_string());
    }

    Ok(AppRelease {
        version,
        name: release.name,
        url: release.html_url,
    })
}

#[tauri::command]
async fn add_remote_manifest(
    app: AppHandle,
    app_id: u32,
    title: String,
    image_url: Option<String>,
) -> Result<AppState, String> {
    if app_id == 0 {
        return Err("AppID 无效".to_string());
    }

    let store = load_store()?;
    let (bytes, status) = download_preferred_manifest(&store, app_id).await?;
    let title = normalize_title(&title, app_id);
    let image_url = normalize_optional_text(image_url);
    import_package_archive(
        &app,
        format!("{app_id}.zip"),
        bytes,
        Some(title),
        image_url,
        status.file_modified,
        status.file_size,
        None,
        true,
    )
}

#[tauri::command]
async fn update_remote_manifest(app: AppHandle, id: String) -> Result<AppState, String> {
    let store = load_store()?;
    let package = store
        .packages
        .iter()
        .find(|package| package.id == id)
        .cloned()
        .ok_or_else(|| "没有找到这个清单".to_string())?;
    let app_id = package
        .app_id
        .ok_or_else(|| "这个清单没有可更新的 AppID".to_string())?;

    let (bytes, status) = download_preferred_manifest(&store, app_id).await?;

    import_package_archive(
        &app,
        format!("{app_id}.zip"),
        bytes,
        Some(package.title),
        package.image_url,
        status.file_modified,
        status.file_size,
        Some(package.id),
        package.enabled,
    )
}

fn import_package_archive(
    app: &AppHandle,
    file_name: String,
    bytes: Vec<u8>,
    fallback_title: Option<String>,
    image_url: Option<String>,
    manifest_updated_at: Option<String>,
    manifest_file_size: Option<u64>,
    replace_package_id: Option<String>,
    enabled: bool,
) -> Result<AppState, String> {
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
    let mut metadata = parse_package_metadata(&lua_file_name, &lua_content, zip_stem);
    if let Some(title) = fallback_title {
        metadata.title = title;
    }
    let package_id = sanitize_id(&metadata.id);
    if package_id.is_empty() {
        return Err("无法生成包 ID".to_string());
    }

    let mut store = load_store()?;
    let root = portable_data_dir()?;
    let should_enable = enabled && configured_steam_path(&store).is_some();
    if let Some(replace_id) = replace_package_id
        .as_deref()
        .map(str::trim)
        .filter(|replace_id| !replace_id.is_empty() && *replace_id != package_id.as_str())
    {
        remove_existing_package(&mut store, replace_id)?;
    }
    remove_existing_package(&mut store, &package_id)?;

    let package_dir = root.join("packages").join(&package_id);
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

    let record = PackageItem {
        id: package_id.clone(),
        title: metadata.title,
        app_id: metadata.app_id,
        lua_file_name: format!("wuhu_{package_id}.lua"),
        manifest_files,
        source_zip_name: file_name,
        enabled: should_enable,
        imported_at: now_seconds(),
        manifest_updated_at,
        manifest_file_size,
        image_url,
    };
    let package_to_sync = record.clone();

    store.packages.push(record);
    store
        .packages
        .sort_by(|left, right| left.title.cmp(&right.title));
    save_store(&store)?;
    sync_package_enabled(&store, &package_to_sync)?;
    build_state(app, store)
}

#[tauri::command]
fn add_steam_game(app: AppHandle, app_id: u32, title: String) -> Result<AppState, String> {
    if app_id == 0 {
        return Err("AppID 无效".to_string());
    }

    let title = normalize_title(&title, app_id);
    let package_id = app_id.to_string();
    let lua_content = build_basic_lua(app_id, &title);

    let mut store = load_store()?;
    let root = portable_data_dir()?;
    let should_enable = configured_steam_path(&store).is_some();
    remove_existing_package(&mut store, &package_id)?;

    let package_dir = root.join("packages").join(&package_id);
    fs::create_dir_all(package_dir.join("manifests"))
        .map_err(|err| format!("创建包目录失败：{err}"))?;
    fs::write(package_dir.join("source.lua"), lua_content.as_bytes())
        .map_err(|err| format!("保存 Lua 失败：{err}"))?;

    let record = PackageItem {
        id: package_id.clone(),
        title,
        app_id: Some(app_id),
        lua_file_name: format!("wuhu_{package_id}.lua"),
        manifest_files: Vec::new(),
        source_zip_name: "Steam 搜索".to_string(),
        enabled: should_enable,
        imported_at: now_seconds(),
        manifest_updated_at: None,
        manifest_file_size: None,
        image_url: None,
    };
    let package_to_sync = record.clone();

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
    let next_enabled = enabled && configured_steam_path(&store).is_some();
    let package = store
        .packages
        .iter_mut()
        .find(|package| package.id == id)
        .ok_or_else(|| "没有找到这个清单".to_string())?;
    package.enabled = next_enabled;
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

    remove_existing_package(&mut store, &package.id)?;
    save_store(&store)?;

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

#[tauri::command]
async fn search_steam_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = search_client("创建 Steam 搜索请求失败")?;
    let response = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[("term", query), ("l", "schinese"), ("cc", "cn")])
        .send()
        .await
        .map_err(|err| format!("Steam 搜索请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("Steam 搜索失败：HTTP {}", response.status()));
    }

    let body = response
        .json::<SteamSearchResponse>()
        .await
        .map_err(|err| format!("解析 Steam 搜索结果失败：{err}"))?;
    Ok(body
        .items
        .into_iter()
        .filter(|item| item.item_type == "app" && !item.name.trim().is_empty())
        .take(24)
        .collect())
}

#[tauri::command]
async fn search_steam_suggest_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = search_client("创建 Steam 搜索建议请求失败")?;
    let response = client
        .get("https://store.steampowered.com/search/suggest")
        .query(&[
            ("term", query),
            ("f", "games"),
            ("cc", "cn"),
            ("realm", "1"),
            ("l", "schinese"),
        ])
        .send()
        .await
        .map_err(|err| format!("Steam 搜索建议请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("Steam 搜索建议失败：HTTP {}", response.status()));
    }

    let html = response
        .text()
        .await
        .map_err(|err| format!("读取 Steam 搜索建议失败：{err}"))?;
    Ok(parse_steam_suggest_items(&html))
}

#[tauri::command]
async fn search_cheapshark_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = search_client("创建 CheapShark 搜索请求失败")?;
    let response = client
        .get("https://www.cheapshark.com/api/1.0/games")
        .query(&[("title", query), ("limit", "10")])
        .send()
        .await
        .map_err(|err| format!("CheapShark 搜索请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("CheapShark 搜索失败：HTTP {}", response.status()));
    }

    let games = response
        .json::<Vec<CheapSharkGame>>()
        .await
        .map_err(|err| format!("解析 CheapShark 搜索结果失败：{err}"))?;

    Ok(games
        .into_iter()
        .filter_map(|game| {
            let id = game.steam_app_id?.parse::<u32>().ok()?;
            let name = game.external?.trim().to_string();
            if id == 0 || name.is_empty() {
                return None;
            }

            Some(SteamSearchItem {
                item_type: "app".to_string(),
                name,
                id,
                tiny_image: game.thumb,
                price: None,
                platforms: None,
            })
        })
        .take(24)
        .collect())
}

#[tauri::command]
async fn search_isthereanydeal_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if contains_cjk(query) {
        return Ok(Vec::new());
    }

    let client = search_client("创建 IsThereAnyDeal 搜索请求失败")?;
    let response = send_itad_search_request(&client, query).await?;

    let games = response
        .json::<Vec<ItadSearchGame>>()
        .await
        .map_err(|err| format!("解析 IsThereAnyDeal 搜索结果失败：{err}"))?;

    let mut tasks = Vec::new();
    for (index, game) in games.into_iter().take(10).enumerate() {
        let client = client.clone();
        tasks.push(tauri::async_runtime::spawn(async move {
            (index, resolve_itad_search_game(client, game).await)
        }));
    }

    let mut indexed_items = Vec::new();
    for task in tasks {
        if let Ok((index, Some(item))) = task.await {
            indexed_items.push((index, item));
        }
    }
    indexed_items.sort_by_key(|(index, _)| *index);

    let mut items: Vec<SteamSearchItem> = Vec::new();
    for (_, item) in indexed_items {
        if !items.iter().any(|current| current.id == item.id) {
            items.push(item);
        }
    }
    Ok(items)
}

fn search_client(error_prefix: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("{error_prefix}：{err}"))
}

async fn send_itad_search_request(
    client: &reqwest::Client,
    query: &str,
) -> Result<reqwest::Response, String> {
    let mut last_error = "未知错误".to_string();

    for attempt in 0..3 {
        match client
            .get("https://isthereanydeal.com/search/api/games/")
            .query(&[("q", query)])
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status();
                last_error = format!("HTTP {status}");
                if !is_retryable_http_status(status) {
                    break;
                }
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }

        if attempt < 2 {
            std::thread::sleep(Duration::from_millis(350 * (attempt + 1)));
        }
    }

    Err(format!("IsThereAnyDeal 搜索失败：{last_error}"))
}

fn is_retryable_http_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.as_u16() >= 500
}

async fn resolve_itad_search_game(
    client: reqwest::Client,
    game: ItadSearchGame,
) -> Option<SteamSearchItem> {
    let name = game.title.trim().to_string();
    if name.is_empty() || game.slug.trim().is_empty() {
        return None;
    }

    let response = client
        .get(format!(
            "https://isthereanydeal.com/game/{}/info/",
            game.slug.trim()
        ))
        .header(reqwest::header::ACCEPT, "text/html")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let id = extract_itad_app_id(&html)?;

    Some(SteamSearchItem {
        item_type: "app".to_string(),
        name,
        id,
        tiny_image: itad_asset_url(&game.assets),
        price: None,
        platforms: None,
    })
}

fn parse_steam_suggest_items(html: &str) -> Vec<SteamSearchItem> {
    html.split("<a ")
        .skip(1)
        .filter_map(parse_steam_suggest_anchor)
        .take(24)
        .collect()
}

fn parse_steam_suggest_anchor(anchor: &str) -> Option<SteamSearchItem> {
    let id = find_html_attr(anchor, "data-ds-appid")?
        .parse::<u32>()
        .ok()?;
    let name = extract_between(anchor, "<div class=\"match_name\">", "</div>")
        .map(decode_html_text)
        .map(|value| value.trim().to_string())?;

    if id == 0 || name.is_empty() {
        return None;
    }

    let tiny_image = extract_between(anchor, "<div class=\"match_img\">", "</div>")
        .and_then(|image_html| find_html_attr(image_html, "src"));
    let price = extract_between(anchor, "<div class=\"match_price\">", "</div>")
        .map(decode_html_text)
        .and_then(|text| parse_cny_price(&text));

    Some(SteamSearchItem {
        item_type: "app".to_string(),
        name,
        id,
        tiny_image,
        price,
        platforms: None,
    })
}

fn find_html_attr(html: &str, attr: &str) -> Option<String> {
    let marker = format!("{attr}=\"");
    let start = html.find(&marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    Some(decode_html_text(&rest[..end]))
}

fn extract_itad_app_id(html: &str) -> Option<u32> {
    [
        "https://store.steampowered.com/app/",
        "https://steamdb.info/app/",
        "https://www.protondb.com/app/",
        "/steam/apps/",
    ]
    .into_iter()
    .find_map(|marker| parse_digits_after(html, marker))
}

fn parse_digits_after(text: &str, marker: &str) -> Option<u32> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok().filter(|id| *id > 0)
}

fn itad_asset_url(assets: &Option<serde_json::Value>) -> Option<String> {
    let object = assets.as_ref()?.as_object()?;
    ["boxart", "banner145", "banner300", "banner400"]
        .into_iter()
        .find_map(|key| object.get(key)?.as_str().map(ToString::to_string))
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
        )
    })
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_index = text.find(start)? + start.len();
    let rest = &text[start_index..];
    let end_index = rest.find(end)?;
    Some(&rest[..end_index])
}

fn decode_html_text(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn parse_cny_price(text: &str) -> Option<SteamSearchPrice> {
    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    if normalized.is_empty() {
        return None;
    }

    let value = normalized.parse::<f64>().ok()?;
    Some(SteamSearchPrice {
        currency: "CNY".to_string(),
        initial: (value * 100.0).round() as u32,
        final_price: (value * 100.0).round() as u32,
    })
}

fn hubcap_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建清单请求失败：{err}"))
}

fn depotbox_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建清单请求失败：{err}"))
}

fn hubcap_api_key(store: &AppStore) -> Result<String, String> {
    trimmed_api_key(store.settings.hubcap_api_key.as_deref())
        .ok_or_else(|| "请先在设置里保存 Key".to_string())
}

fn depotbox_api_key(store: &AppStore) -> Result<String, String> {
    trimmed_api_key(store.settings.depotbox_api_key.as_deref())
        .ok_or_else(|| "请先在设置里保存 Key".to_string())
}

fn trimmed_api_key(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
}

async fn fetch_hubcap_manifest_status(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<ManifestStatus, String> {
    let response = client
        .get(format!("https://hubcapmanifest.com/api/v1/status/{app_id}"))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("检查清单失败：{err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("读取清单状态失败：{err}"))?;

    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        let detail = parse_hubcap_error_detail(&text);
        return Err(format!("Key 无效或无权限{}", detail_prefix(&detail)));
    }

    if !status_code.is_success() {
        return Ok(ManifestStatus {
            provider: "hubcap".to_string(),
            app_id,
            game_name: None,
            status: Some(status_code.to_string()),
            available: false,
            manifest_file_exists: false,
            update_in_progress: None,
            needs_update: None,
            file_size: None,
            file_modified: None,
            error: parse_hubcap_error_detail(&text),
        });
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析清单状态失败：{err}"))?;
    let manifest_file_exists = value_as_bool(&json, "manifest_file_exists").unwrap_or(false);
    let status = value_as_string(&json, "status");
    let update_in_progress = value_as_bool(&json, "update_in_progress");

    Ok(ManifestStatus {
        provider: "hubcap".to_string(),
        app_id: value_as_u32(&json, "app_id").unwrap_or(app_id),
        game_name: value_as_string(&json, "game_name")
            .or_else(|| value_as_string(&json, "app_name")),
        available: manifest_file_exists
            && status
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("available"))
                .unwrap_or(false)
            && !update_in_progress.unwrap_or(false),
        status,
        manifest_file_exists,
        update_in_progress,
        needs_update: value_as_bool(&json, "needs_update"),
        file_size: value_as_u64(&json, "file_size"),
        file_modified: value_as_string(&json, "file_modified")
            .or_else(|| value_as_string(&json, "updated_at"))
            .or_else(|| value_as_string(&json, "last_modified")),
        error: None,
    })
}

async fn download_hubcap_manifest(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let status = fetch_hubcap_manifest_status(client, api_key, app_id).await?;
    if !status.available || status.update_in_progress.unwrap_or(false) {
        return Err("当前没有可用清单".to_string());
    }

    let response = client
        .get(format!(
            "https://hubcapmanifest.com/api/v1/manifest/{app_id}"
        ))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = hubcap_error_detail(response).await;
        return Err(format!(
            "下载清单失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取清单失败：{err}"))?
        .to_vec();

    Ok((bytes, status))
}

async fn fetch_depotbox_manifest_statuses(
    client: &reqwest::Client,
    api_key: &str,
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let unique_app_ids: Vec<u32> = app_ids
        .into_iter()
        .filter(|app_id| *app_id > 0)
        .take(100)
        .collect();
    if unique_app_ids.is_empty() {
        return Ok(Vec::new());
    }

    let body = DepotBoxBatchAvailabilityRequest {
        appids: unique_app_ids.iter().map(ToString::to_string).collect(),
    };
    let response = client
        .post("https://depotbox.org/api/games/batch-availability")
        .header("X-API-Key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("检查清单失败：{err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("读取清单状态失败：{err}"))?;

    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        let detail = parse_api_error_detail(&text);
        return Err(format!("Key 无效或无权限{}", detail_prefix(&detail)));
    }

    if status_code == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let detail = parse_api_error_detail(&text);
        return Err(format!("请求过快，请稍后再试{}", detail_prefix(&detail)));
    }

    if !status_code.is_success() {
        let detail = parse_api_error_detail(&text);
        return Err(format!(
            "检查清单失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析清单状态失败：{err}"))?;
    let results = json
        .get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut by_app_id = std::collections::HashMap::new();
    for value in results {
        let Some(app_id) = value_as_u32(&value, "appid") else {
            continue;
        };
        by_app_id.insert(app_id, depotbox_status_from_value(app_id, &value));
    }

    Ok(unique_app_ids
        .into_iter()
        .map(|app_id| {
            by_app_id.remove(&app_id).unwrap_or_else(|| ManifestStatus {
                provider: "depotbox".to_string(),
                app_id,
                game_name: None,
                status: Some("missing".to_string()),
                available: false,
                manifest_file_exists: false,
                update_in_progress: None,
                needs_update: None,
                file_size: None,
                file_modified: None,
                error: Some("没有返回这个 AppID 的清单状态。".to_string()),
            })
        })
        .collect())
}

fn depotbox_status_from_value(app_id: u32, value: &serde_json::Value) -> ManifestStatus {
    let source_available = value
        .get("sources")
        .and_then(serde_json::Value::as_object)
        .map(|sources| {
            sources
                .values()
                .any(|value| value.as_bool().unwrap_or(false))
        })
        .unwrap_or(false);
    let available = value_as_bool(value, "available").unwrap_or(source_available);

    ManifestStatus {
        provider: "depotbox".to_string(),
        app_id,
        game_name: value_as_string(value, "name").or_else(|| value_as_string(value, "game_name")),
        status: Some(
            if available {
                "available"
            } else {
                "unavailable"
            }
            .to_string(),
        ),
        available,
        manifest_file_exists: available,
        update_in_progress: None,
        needs_update: None,
        file_size: None,
        file_modified: None,
        error: None,
    }
}

async fn download_depotbox_manifest(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let mut statuses = fetch_depotbox_manifest_statuses(client, api_key, vec![app_id]).await?;
    let mut status = statuses
        .pop()
        .ok_or_else(|| "没有返回清单状态。".to_string())?;
    if !status.available {
        return Err(status
            .error
            .clone()
            .unwrap_or_else(|| "当前没有可用清单".to_string()));
    }

    let body = DepotBoxDownloadRequest {
        appid: app_id.to_string(),
    };
    let response = client
        .post("https://depotbox.org/api/download")
        .header("X-API-Key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = api_error_detail(response).await;
        return Err(download_error_message(status_code, detail));
    }

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| format!("解析下载任务失败：{err}"))?;
    let token = value_as_string(&json, "token")
        .ok_or_else(|| "下载任务没有返回 token，请稍后重试。".to_string())?;
    let download_link = poll_depotbox_download(client, api_key, &token).await?;
    let bytes = fetch_depotbox_download(client, api_key, &download_link).await?;
    status.file_size = Some(bytes.len() as u64);

    Ok((bytes, status))
}

async fn poll_depotbox_download(
    client: &reqwest::Client,
    api_key: &str,
    token: &str,
) -> Result<String, String> {
    for _ in 0..DEPOTBOX_DOWNLOAD_POLL_LIMIT {
        let response = client
            .get(format!("https://depotbox.org/api/status/{token}"))
            .header("X-API-Key", api_key)
            .send()
            .await
            .map_err(|err| format!("检查下载进度失败：{err}"))?;

        let status_code = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| format!("读取下载进度失败：{err}"))?;
        if !status_code.is_success() {
            return Err(download_error_message(
                status_code,
                parse_api_error_detail(&text),
            ));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| format!("解析下载进度失败：{err}"))?;
        match value_as_string(&json, "status").as_deref() {
            Some("completed") => {
                return value_as_string(&json, "download_link")
                    .ok_or_else(|| "下载完成但没有返回链接，请稍后重试。".to_string());
            }
            Some("failed") => return Err(depotbox_failed_status_message(&json)),
            _ => tokio::time::sleep(Duration::from_millis(1500)).await,
        }
    }

    Err("清单打包超时，请稍后重试。".to_string())
}

async fn fetch_depotbox_download(
    client: &reqwest::Client,
    api_key: &str,
    download_link: &str,
) -> Result<Vec<u8>, String> {
    let url = if download_link.starts_with("http://") || download_link.starts_with("https://") {
        download_link.to_string()
    } else {
        format!("https://depotbox.org{download_link}")
    };
    let response = client
        .get(url)
        .header("X-API-Key", api_key)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = api_error_detail(response).await;
        return Err(download_error_message(status_code, detail));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("读取清单失败：{err}"))
}

fn depotbox_failed_status_message(json: &serde_json::Value) -> String {
    let reason = value_as_string(json, "failureReason").unwrap_or_default();
    let message = value_as_string(json, "message").unwrap_or_default();
    let log_text = json
        .get("logs")
        .and_then(serde_json::Value::as_array)
        .map(|logs| {
            logs.iter()
                .filter_map(|log| value_as_string(log, "message"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let lower = format!("{reason}\n{message}\n{log_text}").to_ascii_lowercase();

    if lower.contains("manifest_not_found")
        || lower.contains("no manifest")
        || lower.contains("main depot key")
        || lower.contains("app access token")
    {
        return "当前没有可下载清单。".to_string();
    }

    "清单下载失败，请稍后重试。".to_string()
}

fn download_error_message(status_code: reqwest::StatusCode, detail: Option<String>) -> String {
    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        return format!("Key 无效或无权限{}", detail_prefix(&detail));
    }
    if status_code == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return format!("请求过快，请稍后再试{}", detail_prefix(&detail));
    }

    let lower = detail.as_deref().unwrap_or_default().to_ascii_lowercase();
    if status_code == reqwest::StatusCode::NOT_FOUND
        || lower.contains("manifest_not_found")
        || lower.contains("manifest for the requested appid could not be found")
    {
        return "当前没有可下载清单。".to_string();
    }

    format!("下载清单失败：HTTP {status_code}{}", detail_prefix(&detail))
}

async fn download_preferred_manifest(
    store: &AppStore,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let hubcap_key = trimmed_api_key(store.settings.hubcap_api_key.as_deref());
    let depotbox_key = trimmed_api_key(store.settings.depotbox_api_key.as_deref());

    if hubcap_key.is_none() && depotbox_key.is_none() {
        return Err("请先在设置里保存 Key".to_string());
    }

    let mut hubcap_error = None;
    if let Some(api_key) = hubcap_key.as_deref() {
        let client = hubcap_client()?;
        match download_hubcap_manifest(&client, api_key, app_id).await {
            Ok(result) => return Ok(result),
            Err(err) => hubcap_error = Some(err),
        }
    }

    if let Some(api_key) = depotbox_key.as_deref() {
        let client = depotbox_client()?;
        return download_depotbox_manifest(&client, api_key, app_id).await;
    }

    Err(hubcap_error.unwrap_or_else(|| "当前没有可用清单".to_string()))
}

async fn fetch_hubcap_quota(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<HubcapQuota, String> {
    let response = client
        .get("https://hubcapmanifest.com/api/v1/user/stats")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("读取额度失败：{err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("读取额度失败：{err}"))?;

    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        let detail = parse_hubcap_error_detail(&text);
        return Err(format!("Key 无效或无权限{}", detail_prefix(&detail)));
    }

    if !status_code.is_success() {
        let detail = parse_hubcap_error_detail(&text);
        return Err(format!(
            "读取额度失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析额度失败：{err}"))?;

    Ok(HubcapQuota {
        daily_usage: value_as_u64(&json, "daily_usage").unwrap_or(0),
        daily_limit: value_as_u64(&json, "daily_limit").unwrap_or(0),
    })
}

async fn hubcap_error_detail(response: reqwest::Response) -> Option<String> {
    api_error_detail(response).await
}

async fn api_error_detail(response: reqwest::Response) -> Option<String> {
    response
        .text()
        .await
        .ok()
        .and_then(|text| parse_api_error_detail(&text))
}

fn parse_hubcap_error_detail(text: &str) -> Option<String> {
    parse_api_error_detail(text)
}

fn parse_api_error_detail(text: &str) -> Option<String> {
    serde_json::from_str::<ApiErrorResponse>(text)
        .ok()
        .and_then(|body| body.detail.or(body.error).or(body.message))
        .filter(|message| !message.trim().is_empty())
        .or_else(|| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.chars().take(180).collect())
            }
        })
}

fn detail_prefix(detail: &Option<String>) -> String {
    detail
        .as_deref()
        .map(|message| format!("：{message}"))
        .unwrap_or_default()
}

fn value_as_string(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn value_as_bool(json: &serde_json::Value, key: &str) -> Option<bool> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

fn value_as_u32(json: &serde_json::Value, key: &str) -> Option<u32> {
    value_as_u64(json, key).and_then(|value| u32::try_from(value).ok())
}

fn value_as_u64(json: &serde_json::Value, key: &str) -> Option<u64> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            detect_steam_path,
            set_steam_path,
            import_package_from_bytes,
            set_hubcap_api_key,
            set_depotbox_api_key,
            check_hubcap_manifest_statuses,
            check_depotbox_manifest_statuses,
            get_hubcap_quota,
            get_latest_app_release,
            add_remote_manifest,
            update_remote_manifest,
            set_package_enabled,
            delete_package,
            install_opensteamtool,
            restore_opensteamtool,
            set_steam_client_version_locked,
            search_steam_games,
            search_steam_suggest_games,
            search_cheapshark_games,
            search_isthereanydeal_games,
            add_steam_game
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
    let Some(steam_path) = configured_steam_path(store) else {
        return Ok(());
    };
    let steam_root = Path::new(steam_path);
    let root = portable_data_dir()?;

    if package.enabled {
        apply_package(&root, steam_root, package)
    } else {
        remove_active_package(steam_root, package)
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

fn remove_active_package(steam_root: &Path, package: &PackageItem) -> Result<(), String> {
    remove_active_lua(steam_root, package)?;

    for manifest_name in &package.manifest_files {
        let manifest_path = steam_root.join("depotcache").join(manifest_name);
        if manifest_path.exists() {
            fs::remove_file(&manifest_path)
                .map_err(|err| format!("移除 manifest {manifest_name} 失败：{err}"))?;
        }
    }

    Ok(())
}

fn remove_existing_package(store: &mut AppStore, package_id: &str) -> Result<(), String> {
    if let Some(existing) = store
        .packages
        .iter()
        .find(|package| package.id == package_id)
        .cloned()
    {
        if let Some(steam_path) = configured_steam_path(store) {
            remove_active_package(Path::new(steam_path), &existing)?;
        }
    }

    store.packages.retain(|package| package.id != package_id);

    let package_dir = portable_data_dir()?.join("packages").join(package_id);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|err| format!("清理旧包失败：{err}"))?;
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

fn configured_steam_path(store: &AppStore) -> Option<&str> {
    store
        .settings
        .steam_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
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

fn normalize_title(title: &str, app_id: u32) -> String {
    let title = title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if title.is_empty() {
        app_id.to_string()
    } else {
        title
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    })
}

fn build_basic_lua(app_id: u32, title: &str) -> String {
    format!("-- {title}\naddappid({app_id})\n")
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
