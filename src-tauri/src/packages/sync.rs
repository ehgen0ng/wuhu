use std::{fs, path::Path};

use crate::{
    models::{AppStore, PackageItem},
    steam,
    store::portable_data_dir,
};

pub(crate) fn reconcile_with_steam(store: &mut AppStore) -> Result<bool, String> {
    let Some(steam_path) = store.settings.steam_path.as_deref() else {
        return Ok(set_all_packages_enabled(store, false));
    };
    let steam_root = Path::new(steam_path);
    if !steam::looks_like_root(steam_root) {
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

pub(super) fn sync_package_enabled(store: &AppStore, package: &PackageItem) -> Result<(), String> {
    let Some(steam_path) = steam::configured_path(store) else {
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
        if let Some(steam_path) = steam::configured_path(store) {
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
