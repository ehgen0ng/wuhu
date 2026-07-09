use std::{
    ffi::OsStr,
    fs,
    io::{Cursor, Read, Seek},
    path::Path,
};

use tauri::AppHandle;
use zip::ZipArchive;

use crate::{
    models::{AppState, PackageItem},
    state::build_state,
    steam,
    store::{load_store, portable_data_dir, save_store},
};

use super::{metadata, now_seconds, sync};

pub(crate) fn import_archive(
    _app: &AppHandle,
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
    let mut package_metadata =
        metadata::parse_package_metadata(&lua_file_name, &lua_content, zip_stem);
    if let Some(title) = fallback_title {
        package_metadata.title = title;
    }
    let package_id = metadata::sanitize_id(&package_metadata.id);
    if package_id.is_empty() {
        return Err("无法生成包 ID".to_string());
    }

    let mut store = load_store()?;
    let root = portable_data_dir()?;
    let should_enable = enabled && steam::package_sync_root(&store).is_some();
    if let Some(replace_id) = replace_package_id
        .as_deref()
        .map(str::trim)
        .filter(|replace_id| !replace_id.is_empty() && *replace_id != package_id.as_str())
    {
        sync::remove_existing_package(&mut store, replace_id)?;
    }
    sync::remove_existing_package(&mut store, &package_id)?;

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
        title: package_metadata.title,
        app_id: package_metadata.app_id,
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
    sync::sync_package_enabled(&store, &package_to_sync)?;
    build_state(store)
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
