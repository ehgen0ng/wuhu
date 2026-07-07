use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::AppHandle;

use crate::{
    models::{AppState, PackageItem},
    state::build_state,
    steam,
    store::{load_store, portable_data_dir, save_store},
};

mod archive;
mod metadata;
mod sync;

pub(crate) use archive::import_archive;
pub(crate) use metadata::{normalize_optional_text, normalize_title};
pub(crate) use sync::reconcile_with_steam;

pub(crate) fn add_steam_game(
    _app: &AppHandle,
    app_id: u32,
    title: String,
) -> Result<AppState, String> {
    if app_id == 0 {
        return Err("AppID 无效".to_string());
    }

    let title = normalize_title(&title, app_id);
    let package_id = app_id.to_string();
    let lua_content = metadata::build_basic_lua(app_id, &title);

    let mut store = load_store()?;
    let root = portable_data_dir()?;
    let should_enable = steam::configured_path(&store).is_some();
    sync::remove_existing_package(&mut store, &package_id)?;

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
    sync::sync_package_enabled(&store, &package_to_sync)?;
    build_state(store)
}

pub(crate) fn set_enabled(_app: &AppHandle, id: String, enabled: bool) -> Result<AppState, String> {
    let mut store = load_store()?;
    let next_enabled = enabled && steam::configured_path(&store).is_some();
    let package = store
        .packages
        .iter_mut()
        .find(|package| package.id == id)
        .ok_or_else(|| "没有找到这个清单".to_string())?;
    package.enabled = next_enabled;
    let package = package.clone();
    save_store(&store)?;
    sync::sync_package_enabled(&store, &package)?;
    build_state(store)
}

pub(crate) fn delete(_app: &AppHandle, id: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let package = store
        .packages
        .iter()
        .find(|package| package.id == id)
        .cloned()
        .ok_or_else(|| "没有找到这个清单".to_string())?;

    sync::remove_existing_package(&mut store, &package.id)?;
    save_store(&store)?;

    build_state(store)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
