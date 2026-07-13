use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    models::{AppStore, PackageItem, TicketItem},
    steam,
    store::portable_data_dir,
    tickets,
};

pub(crate) fn reconcile_with_steam(store: &mut AppStore) -> Result<bool, String> {
    let Some(steam_root) = steam::package_sync_root(store) else {
        return Ok(set_all_packages_enabled(store, false));
    };

    let app_root = portable_data_dir()?;
    let tickets = store.tickets.clone();
    let mut changed = false;
    for package in &mut store.packages {
        let actual_enabled = package_matches_steam(&app_root, &steam_root, &tickets, package);
        if package.enabled != actual_enabled {
            package.enabled = actual_enabled;
            changed = true;
        }
    }

    Ok(changed)
}

pub(super) fn sync_package_enabled(store: &AppStore, package: &PackageItem) -> Result<(), String> {
    let Some(steam_root) = steam::package_sync_root(store) else {
        return Ok(());
    };
    let root = portable_data_dir()?;

    if package.enabled {
        apply_package(&root, &steam_root, package, &store.tickets)
    } else {
        remove_active_package(&steam_root, package)
    }
}

pub(crate) fn sync_enabled_packages_for_app_id(
    store: &AppStore,
    app_id: u32,
) -> Result<(), String> {
    for package in store
        .packages
        .iter()
        .filter(|package| package.enabled && package.app_id == Some(app_id))
    {
        sync_package_enabled(store, package)?;
    }
    Ok(())
}

pub(super) fn remove_existing_package(
    store: &mut AppStore,
    package_id: &str,
) -> Result<(), String> {
    if let Some(existing) = store
        .packages
        .iter()
        .find(|package| package.id == package_id)
        .cloned()
    {
        if let Some(steam_root) = steam::package_sync_root(store) {
            remove_active_package(&steam_root, &existing)?;
        }
    }

    store.packages.retain(|package| package.id != package_id);

    let package_dir = portable_data_dir()?.join("packages").join(package_id);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|err| format!("清理旧包失败：{err}"))?;
    }

    Ok(())
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

fn package_matches_steam(
    app_root: &Path,
    steam_root: &Path,
    tickets: &[TicketItem],
    package: &PackageItem,
) -> bool {
    let package_dir = app_root.join("packages").join(&package.id);
    let steam_lua = steam_root
        .join("config")
        .join("lua")
        .join(&package.lua_file_name);
    let Ok(expected_lua) = render_package_lua(app_root, package, tickets) else {
        return false;
    };
    if !file_matches_bytes(&expected_lua, &steam_lua) {
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

fn file_matches_bytes(expected: &[u8], actual: &Path) -> bool {
    let Ok(actual_meta) = fs::metadata(actual) else {
        return false;
    };
    if actual_meta.len() != expected.len() as u64 {
        return false;
    }

    matches!(fs::read(actual), Ok(actual_bytes) if actual_bytes == expected)
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

fn apply_package(
    app_root: &Path,
    steam_root: &Path,
    package: &PackageItem,
    tickets: &[TicketItem],
) -> Result<(), String> {
    let lua_dir = steam_root.join("config").join("lua");
    let depotcache_dir = steam_root.join("depotcache");
    fs::create_dir_all(&lua_dir).map_err(|err| format!("创建 Lua 目录失败：{err}"))?;
    fs::create_dir_all(&depotcache_dir)
        .map_err(|err| format!("创建 depotcache 目录失败：{err}"))?;

    let lua = render_package_lua(app_root, package, tickets)?;
    fs::write(lua_dir.join(&package.lua_file_name), lua)
        .map_err(|err| format!("写入启用 Lua 失败：{err}"))?;

    let package_dir = app_root.join("packages").join(&package.id);
    for manifest_name in &package.manifest_files {
        let source = package_dir.join("manifests").join(manifest_name);
        let target = depotcache_dir.join(manifest_name);
        fs::copy(&source, &target)
            .map_err(|err| format!("复制 manifest {manifest_name} 失败：{err}"))?;
    }

    Ok(())
}

fn render_package_lua(
    app_root: &Path,
    package: &PackageItem,
    ticket_items: &[TicketItem],
) -> Result<Vec<u8>, String> {
    let package_dir = app_root.join("packages").join(&package.id);
    let mut lua = fs::read_to_string(stored_lua_path(&package_dir, &package.lua_file_name))
        .map_err(|err| format!("读取 {} 的 Lua 失败：{err}", package.title))?;

    if let Some(app_id) = package.app_id {
        if ticket_items.iter().any(|ticket| ticket.app_id == app_id) {
            if let Some(ticket_lua) = tickets::lua_for_app_id(app_id)? {
                if !lua.ends_with('\n') {
                    lua.push('\n');
                }
                if !lua.ends_with("\n\n") {
                    lua.push('\n');
                }
                lua.push_str(&ticket_lua);
                lua.push('\n');
            }
        }
    }

    Ok(lua.into_bytes())
}

fn stored_lua_path(package_dir: &Path, lua_file_name: &str) -> PathBuf {
    let original_name_path = package_dir.join(lua_file_name);
    if original_name_path.is_file() {
        original_name_path
    } else {
        package_dir.join("source.lua")
    }
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
