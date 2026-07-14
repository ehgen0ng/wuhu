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
    let Some((steam_root, lua_root)) = package_sync_roots(store) else {
        return Ok(set_all_packages_enabled(store, false));
    };

    let app_root = portable_data_dir()?;
    let tickets = store.tickets.clone();
    let mut changed = false;
    for package in &mut store.packages {
        migrate_legacy_active_lua(&steam_root, &lua_root, package)?;
        let actual_enabled =
            package_matches_steam(&app_root, &steam_root, &lua_root, &tickets, package);
        if package.enabled != actual_enabled {
            package.enabled = actual_enabled;
            changed = true;
        }
    }

    Ok(changed)
}

pub(super) fn sync_package_enabled(store: &AppStore, package: &PackageItem) -> Result<(), String> {
    let Some((steam_root, lua_root)) = package_sync_roots(store) else {
        return Ok(());
    };
    let root = portable_data_dir()?;

    if package.enabled {
        apply_package(&root, &steam_root, &lua_root, package, &store.tickets)
    } else {
        remove_active_package(&steam_root, &lua_root, package)
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
        if let Some((steam_root, lua_root)) = package_sync_roots(store) {
            remove_active_package(&steam_root, &lua_root, &existing)?;
        }
    }

    store.packages.retain(|package| package.id != package_id);

    let package_dir = portable_data_dir()?.join("packages").join(package_id);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|err| format!("清理旧包失败：{err}"))?;
    }

    Ok(())
}

fn package_sync_roots(store: &AppStore) -> Option<(PathBuf, PathBuf)> {
    Some((steam::package_sync_root(store)?, steam::package_lua_root()?))
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
    lua_root: &Path,
    tickets: &[TicketItem],
    package: &PackageItem,
) -> bool {
    let package_dir = app_root.join("packages").join(&package.id);
    let active_lua = lua_root.join(&package.lua_file_name);
    let Ok(expected_lua) = render_package_lua(app_root, package, tickets) else {
        return false;
    };
    if !file_matches_bytes(&expected_lua, &active_lua) {
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
    lua_root: &Path,
    package: &PackageItem,
    tickets: &[TicketItem],
) -> Result<(), String> {
    let depotcache_dir = steam_root.join("depotcache");
    fs::create_dir_all(lua_root).map_err(|err| format!("创建 Lua 目录失败：{err}"))?;
    fs::create_dir_all(&depotcache_dir)
        .map_err(|err| format!("创建 depotcache 目录失败：{err}"))?;

    let lua = render_package_lua(app_root, package, tickets)?;
    fs::write(lua_root.join(&package.lua_file_name), lua)
        .map_err(|err| format!("写入启用 Lua 失败：{err}"))?;
    remove_legacy_active_lua(steam_root, package)?;

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

fn legacy_active_lua_path(steam_root: &Path, package: &PackageItem) -> PathBuf {
    steam_root
        .join("config")
        .join("lua")
        .join(&package.lua_file_name)
}

fn remove_legacy_active_lua(steam_root: &Path, package: &PackageItem) -> Result<(), String> {
    let legacy_lua = legacy_active_lua_path(steam_root, package);
    if legacy_lua.exists() {
        fs::remove_file(&legacy_lua).map_err(|err| format!("移除旧版启用 Lua 失败：{err}"))?;
    }
    Ok(())
}

fn migrate_legacy_active_lua(
    steam_root: &Path,
    lua_root: &Path,
    package: &PackageItem,
) -> Result<(), String> {
    let legacy_lua = legacy_active_lua_path(steam_root, package);
    if !legacy_lua.is_file() {
        return Ok(());
    }

    let active_lua = lua_root.join(&package.lua_file_name);
    if active_lua.exists() {
        if files_match(&legacy_lua, &active_lua) {
            remove_legacy_active_lua(steam_root, package)?;
        }
        return Ok(());
    }

    fs::create_dir_all(lua_root).map_err(|err| format!("创建 Lua 目录失败：{err}"))?;
    fs::copy(&legacy_lua, &active_lua).map_err(|err| format!("迁移旧版启用 Lua 失败：{err}"))?;
    if !files_match(&legacy_lua, &active_lua) {
        let _ = fs::remove_file(&active_lua);
        return Err("迁移旧版启用 Lua 后校验失败".to_string());
    }
    remove_legacy_active_lua(steam_root, package)?;

    Ok(())
}

fn remove_active_lua(
    steam_root: &Path,
    lua_root: &Path,
    package: &PackageItem,
) -> Result<(), String> {
    let active_lua = lua_root.join(&package.lua_file_name);
    if active_lua.exists() {
        fs::remove_file(&active_lua).map_err(|err| format!("移除启用 Lua 失败：{err}"))?;
    }
    remove_legacy_active_lua(steam_root, package)
}

fn remove_active_package(
    steam_root: &Path,
    lua_root: &Path,
    package: &PackageItem,
) -> Result<(), String> {
    remove_active_lua(steam_root, lua_root, package)?;

    for manifest_name in &package.manifest_files {
        let manifest_path = steam_root.join("depotcache").join(manifest_name);
        if manifest_path.exists() {
            fs::remove_file(&manifest_path)
                .map_err(|err| format!("移除 manifest {manifest_name} 失败：{err}"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{legacy_active_lua_path, migrate_legacy_active_lua};
    use crate::models::PackageItem;

    fn test_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("wuhu-{label}-{}-{unique}", std::process::id()))
    }

    fn test_package() -> PackageItem {
        PackageItem {
            id: "3265700".to_string(),
            title: "Test package".to_string(),
            app_id: Some(3_265_700),
            lua_file_name: "3265700.lua".to_string(),
            manifest_files: Vec::new(),
            source_zip_name: "test.zip".to_string(),
            enabled: true,
            imported_at: 0,
            manifest_updated_at: None,
            manifest_file_size: None,
            image_url: None,
        }
    }

    #[test]
    fn migrates_matching_legacy_lua_to_opensteamtool_data_root() {
        let root = test_root("migrate-lua");
        let steam_root = root.join("Steam");
        let lua_root = root.join("OpenSteamTool").join("lua");
        let package = test_package();
        let legacy_lua = legacy_active_lua_path(&steam_root, &package);
        std::fs::create_dir_all(
            legacy_lua
                .parent()
                .expect("legacy Lua should have a parent"),
        )
        .expect("legacy Lua directory should be created");
        std::fs::write(&legacy_lua, b"addappid(3265700)\n").expect("legacy Lua should be created");

        migrate_legacy_active_lua(&steam_root, &lua_root, &package)
            .expect("legacy Lua should migrate");

        assert!(!legacy_lua.exists());
        assert_eq!(
            std::fs::read(lua_root.join(&package.lua_file_name))
                .expect("migrated Lua should be readable"),
            b"addappid(3265700)\n"
        );
        std::fs::remove_dir_all(root).expect("test directory should be removed");
    }

    #[test]
    fn preserves_conflicting_legacy_and_current_lua_files() {
        let root = test_root("conflicting-lua");
        let steam_root = root.join("Steam");
        let lua_root = root.join("OpenSteamTool").join("lua");
        let package = test_package();
        let legacy_lua = legacy_active_lua_path(&steam_root, &package);
        let active_lua = lua_root.join(&package.lua_file_name);
        std::fs::create_dir_all(
            legacy_lua
                .parent()
                .expect("legacy Lua should have a parent"),
        )
        .expect("legacy Lua directory should be created");
        std::fs::create_dir_all(&lua_root).expect("current Lua directory should be created");
        std::fs::write(&legacy_lua, b"legacy").expect("legacy Lua should be created");
        std::fs::write(&active_lua, b"current").expect("current Lua should be created");

        migrate_legacy_active_lua(&steam_root, &lua_root, &package)
            .expect("a conflict should be preserved without failing");

        assert_eq!(
            std::fs::read(&legacy_lua).expect("legacy Lua should remain"),
            b"legacy"
        );
        assert_eq!(
            std::fs::read(&active_lua).expect("current Lua should remain"),
            b"current"
        );
        std::fs::remove_dir_all(root).expect("test directory should be removed");
    }
}
