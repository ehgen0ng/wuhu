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

    let mut changed = false;
    for package in &mut store.packages {
        migrate_legacy_active_lua(&steam_root, &lua_root, package)?;
        let actual_enabled = package_is_enabled(&lua_root, package);
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

pub(crate) fn with_suspended_packages_for_app_id<T>(
    store: &AppStore,
    app_id: u32,
    extract: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let Some((steam_root, lua_root)) = package_sync_roots(store) else {
        return extract();
    };
    let mut suspended = SuspendedLua::default();
    let result = (|| {
        suspended.suspend(&steam_root, &lua_root, &store.packages, app_id)?;
        if !suspended.files.is_empty() {
            // OST 的 Lua watcher 有 500ms 防抖，另留出事件投递与处理时间。
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        extract()
    })();
    match (result, suspended.restore()) {
        (result, Ok(())) => result,
        (Ok(_), Err(restore_error)) => Err(restore_error),
        (Err(error), Err(restore_error)) => Err(format!("{error}\n{restore_error}")),
    }
}

#[derive(Default)]
struct SuspendedLua {
    files: Vec<(PathBuf, PathBuf)>,
}

impl SuspendedLua {
    fn suspend(
        &mut self,
        steam_root: &Path,
        lua_root: &Path,
        packages: &[PackageItem],
        app_id: u32,
    ) -> Result<(), String> {
        for package in packages
            .iter()
            .filter(|package| package.app_id == Some(app_id))
        {
            for active in [
                lua_root.join(&package.lua_file_name),
                legacy_active_lua_path(steam_root, package),
            ] {
                if !active.is_file() {
                    continue;
                }
                // 扩展名不再是 .lua，watcher 会卸载旧票据；不删除 depotcache。
                let backup = active.with_extension("lua.wuhu-ticket-backup");
                if backup.exists() {
                    return Err(format!(
                        "临时停用清单失败：备份已存在，请先检查 {}",
                        backup.display()
                    ));
                }
                fs::rename(&active, &backup)
                    .map_err(|err| format!("临时停用清单失败：{}: {err}", active.display()))?;
                self.files.push((active, backup));
            }
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        self.files.retain(|(active, backup)| {
            // 不覆盖提取期间从外部写入的文件；保留备份以便恢复。
            let result = if active.exists() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "目标 Lua 已存在",
                ))
            } else {
                fs::rename(backup, active)
            };
            if let Err(err) = result {
                errors.push(format!(
                    "{} -> {}: {err}",
                    backup.display(),
                    active.display()
                ));
                true
            } else {
                false
            }
        });
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("恢复临时停用的清单失败：{}", errors.join("; ")))
        }
    }
}

impl Drop for SuspendedLua {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("[wuhu] {error}");
        }
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

// 开关表示对应 Lua 是否位于启用目录；缓存、票据或本地副本的差异不代表已禁用。
fn package_is_enabled(lua_root: &Path, package: &PackageItem) -> bool {
    lua_root.join(&package.lua_file_name).is_file()
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
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        apply_package, legacy_active_lua_path, migrate_legacy_active_lua, package_is_enabled,
        remove_active_lua, SuspendedLua,
    };
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
    fn extraction_suspends_only_target_lua_and_restores_exact_contents() {
        let root = test_root("ticket-suspend");
        let (steam_root, lua_root, package) = apply_test_package(&root);
        let active = lua_root.join(&package.lua_file_name);
        let original = b"addappid(3265700)\n-- existing ticket and custom settings\n";
        std::fs::write(&active, original).unwrap();
        let legacy = legacy_active_lua_path(&steam_root, &package);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, b"legacy settings").unwrap();
        let mut disabled = package.clone();
        disabled.id = "disabled".into();
        disabled.enabled = false;
        disabled.lua_file_name = "disabled.lua".into();
        let mut unrelated = package.clone();
        unrelated.app_id = Some(123);
        unrelated.lua_file_name = "123.lua".into();
        let unrelated_path = lua_root.join(&unrelated.lua_file_name);
        std::fs::write(&unrelated_path, b"addappid(123)").unwrap();

        let mut suspended = SuspendedLua::default();
        suspended
            .suspend(
                &steam_root,
                &lua_root,
                &[package.clone(), disabled.clone(), unrelated],
                3_265_700,
            )
            .unwrap();
        assert!(!active.exists());
        assert!(!legacy.exists());
        assert!(unrelated_path.is_file());
        assert!(steam_root
            .join("depotcache")
            .join(&package.manifest_files[0])
            .is_file());
        suspended.restore().unwrap();
        assert_eq!(std::fs::read(&active).unwrap(), original);
        assert_eq!(std::fs::read(&legacy).unwrap(), b"legacy settings");
        assert!(!lua_root.join(disabled.lua_file_name).exists());
        assert!(!active.with_extension("lua.wuhu-ticket-backup").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_partial_suspension_restores_previously_moved_lua() {
        let root = test_root("ticket-suspend-error");
        let (steam_root, lua_root, package) = apply_test_package(&root);
        let mut second = package.clone();
        second.lua_file_name = "second.lua".into();
        let active = lua_root.join(&package.lua_file_name);
        let second_active = lua_root.join(&second.lua_file_name);
        let backup = second_active.with_extension("lua.wuhu-ticket-backup");
        std::fs::write(&second_active, b"second original").unwrap();
        std::fs::write(&backup, b"previous backup").unwrap();
        {
            let mut suspended = SuspendedLua::default();
            assert!(suspended
                .suspend(&steam_root, &lua_root, &[package, second], 3_265_700)
                .is_err());
            assert!(!active.exists());
        }
        assert!(active.is_file());
        assert_eq!(std::fs::read(backup).unwrap(), b"previous backup");
        assert_eq!(std::fs::read(second_active).unwrap(), b"second original");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extraction_error_restores_lua_on_scope_exit() {
        let root = test_root("ticket-extract-error");
        let (steam_root, lua_root, package) = apply_test_package(&root);
        let active = lua_root.join(&package.lua_file_name);
        let result: Result<(), String> = (|| {
            let mut suspended = SuspendedLua::default();
            suspended.suspend(&steam_root, &lua_root, &[package], 3_265_700)?;
            Err("ConnectToGlobalUser failed".into())
        })();
        assert!(result.is_err());
        assert!(active.is_file());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_conflict_preserves_backup_and_reports_failure() {
        let root = test_root("ticket-restore-conflict");
        let (steam_root, lua_root, package) = apply_test_package(&root);
        let active = lua_root.join(&package.lua_file_name);
        let mut suspended = SuspendedLua::default();
        suspended
            .suspend(&steam_root, &lua_root, &[package], 3_265_700)
            .unwrap();
        std::fs::write(&active, b"external update").unwrap();
        assert!(suspended.restore().is_err());
        assert_eq!(std::fs::read(&active).unwrap(), b"external update");
        assert!(active.with_extension("lua.wuhu-ticket-backup").is_file());
        std::fs::remove_file(&active).unwrap();
        suspended.restore().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    // 在临时目录中执行真实启用流程，避免测试读写用户的 Steam 数据。
    fn apply_test_package(root: &Path) -> (PathBuf, PathBuf, PackageItem) {
        let steam_root = root.join("Steam");
        let lua_root = root.join("OpenSteamTool").join("lua");
        let mut package = test_package();
        package.manifest_files = vec!["3265701_123.manifest".to_string()];
        let package_dir = root.join("packages").join(&package.id);
        std::fs::create_dir_all(package_dir.join("manifests")).unwrap();
        std::fs::write(package_dir.join("source.lua"), b"addappid(3265700)\n").unwrap();
        std::fs::write(
            package_dir.join("manifests").join(&package.manifest_files[0]),
            b"original manifest",
        )
        .unwrap();
        apply_package(root, &steam_root, &lua_root, &package, &[]).unwrap();
        (steam_root, lua_root, package)
    }

    #[test]
    fn active_lua_stays_enabled_when_manifest_cache_changes_or_disappears() {
        let root = test_root("manifest-cache-state");
        let (steam_root, lua_root, mut package) = apply_test_package(&root);
        // 模拟旧版本已把保存状态误判为关闭，读取应以启用目录为准。
        package.enabled = false;
        let cached_manifest = steam_root
            .join("depotcache")
            .join(&package.manifest_files[0]);

        std::fs::write(&cached_manifest, b"updated manifest").unwrap();
        let enabled_after_change = package_is_enabled(&lua_root, &package);
        std::fs::remove_file(&cached_manifest).unwrap();
        let enabled_after_removal = package_is_enabled(&lua_root, &package);

        std::fs::remove_dir_all(root).unwrap();
        assert!(
            enabled_after_change,
            "cache changes must not disable an active Lua"
        );
        assert!(
            enabled_after_removal,
            "cache removal must not disable an active Lua"
        );
    }

    #[test]
    fn active_lua_stays_enabled_when_its_text_or_local_copy_changes() {
        let root = test_root("lua-copy-state");
        let (_, lua_root, package) = apply_test_package(&root);
        std::fs::write(
            lua_root.join(&package.lua_file_name),
            b"-- updated comment\r\naddappid(3265700)\r\n",
        )
        .unwrap();
        let enabled_after_edit = package_is_enabled(&lua_root, &package);
        std::fs::remove_dir_all(root.join("packages").join(&package.id)).unwrap();
        let enabled_without_local_copy = package_is_enabled(&lua_root, &package);

        std::fs::remove_dir_all(root).unwrap();
        assert!(
            enabled_after_edit,
            "Lua text differences must not change the switch"
        );
        assert!(
            enabled_without_local_copy,
            "local copies must not determine the switch"
        );
    }

    #[test]
    fn manifest_cache_without_an_active_lua_file_stays_disabled() {
        let root = test_root("disabled-lua-state");
        let (steam_root, lua_root, package) = apply_test_package(&root);
        assert!(package_is_enabled(&lua_root, &package));
        remove_active_lua(&steam_root, &lua_root, &package).unwrap();
        assert!(steam_root
            .join("depotcache")
            .join(&package.manifest_files[0])
            .is_file());
        assert!(!package_is_enabled(&lua_root, &package));

        std::fs::create_dir_all(lua_root.join(&package.lua_file_name)).unwrap();
        assert!(!package_is_enabled(&lua_root, &package));
        std::fs::remove_dir_all(root).unwrap();
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
