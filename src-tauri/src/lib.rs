use base64::{engine::general_purpose, Engine as _};
use tauri::AppHandle;

mod manifests;
mod models;
mod net;
mod packages;
mod release;
mod search;
mod state;
mod steam;
mod store;

use models::{AppRelease, AppState, HubcapQuota, ManifestStatus, SteamSearchItem};
use state::build_state;
use store::{load_store, save_store};

#[tauri::command]
fn get_initial_state(_app: AppHandle) -> Result<AppState, String> {
    let mut store = load_store()?;
    let mut changed = false;
    if store.settings.steam_path.is_none() {
        store.settings.steam_path = steam::detect_path();
        changed = store.settings.steam_path.is_some();
    }
    changed |= packages::reconcile_with_steam(&mut store)?;
    if changed {
        save_store(&store)?;
    }
    build_state(store)
}

#[tauri::command]
fn detect_steam_path() -> Option<String> {
    steam::detect_path()
}

#[tauri::command]
fn set_steam_path(_app: AppHandle, path: String) -> Result<AppState, String> {
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
    build_state(store)
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
    packages::import_archive(&app, file_name, bytes, None, None, None, None, None, true)
}

#[tauri::command]
fn set_hubcap_api_key(_app: AppHandle, api_key: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let trimmed = api_key.trim();
    store.settings.hubcap_api_key = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    save_store(&store)?;
    build_state(store)
}

#[tauri::command]
fn set_depotbox_api_key(_app: AppHandle, api_key: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let trimmed = api_key.trim();
    store.settings.depotbox_api_key = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    save_store(&store)?;
    build_state(store)
}

#[tauri::command]
async fn check_hubcap_manifest_statuses(app_ids: Vec<u32>) -> Result<Vec<ManifestStatus>, String> {
    let store = load_store()?;
    manifests::check_hubcap_manifest_statuses(&store, app_ids).await
}

#[tauri::command]
async fn check_depotbox_manifest_statuses(
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let store = load_store()?;
    manifests::check_depotbox_manifest_statuses(&store, app_ids).await
}

#[tauri::command]
async fn get_hubcap_quota() -> Result<HubcapQuota, String> {
    let store = load_store()?;
    manifests::get_hubcap_quota(&store).await
}

#[tauri::command]
async fn get_latest_app_release() -> Result<AppRelease, String> {
    release::get_latest_app_release().await
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
    let (bytes, status) = manifests::download_preferred_manifest(&store, app_id).await?;
    let title = packages::normalize_title(&title, app_id);
    let image_url = packages::normalize_optional_text(image_url);
    packages::import_archive(
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

    let (bytes, status) = manifests::download_preferred_manifest(&store, app_id).await?;

    packages::import_archive(
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

#[tauri::command]
fn add_steam_game(app: AppHandle, app_id: u32, title: String) -> Result<AppState, String> {
    packages::add_steam_game(&app, app_id, title)
}

#[tauri::command]
fn set_package_enabled(app: AppHandle, id: String, enabled: bool) -> Result<AppState, String> {
    packages::set_enabled(&app, id, enabled)
}

#[tauri::command]
fn delete_package(app: AppHandle, id: String) -> Result<AppState, String> {
    packages::delete(&app, id)
}

#[tauri::command]
fn install_opensteamtool(_app: AppHandle) -> Result<AppState, String> {
    let store = load_store()?;
    steam::install_opensteamtool(&store)?;
    build_state(store)
}

#[tauri::command]
fn restore_opensteamtool(_app: AppHandle) -> Result<AppState, String> {
    let store = load_store()?;
    steam::restore_opensteamtool(&store)?;
    build_state(store)
}

#[tauri::command]
fn set_steam_client_version_locked(_app: AppHandle, locked: bool) -> Result<AppState, String> {
    let store = load_store()?;
    steam::set_client_version_locked(&store, locked)?;
    build_state(store)
}

#[tauri::command]
async fn search_steam_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    search::search_steam_games(query).await
}

#[tauri::command]
async fn search_steam_suggest_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    search::search_steam_suggest_games(query).await
}

#[tauri::command]
async fn search_cheapshark_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    search::search_cheapshark_games(query).await
}

#[tauri::command]
async fn search_isthereanydeal_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    search::search_isthereanydeal_games(query).await
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
