use std::{
    ffi::OsStr,
    fs,
    io::{Cursor, Read, Seek, Write},
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
    let bytes = normalize_archive(&file_name, bytes)?;
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
    fs::write(package_dir.join(&lua_file_name), lua_content.as_bytes())
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
        lua_file_name,
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

// Some LuaTools sources return a Lua script directly. Keep one import path for both formats.
pub(crate) fn normalize_archive(file_name: &str, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.starts_with(b"PK") {
        let mut archive =
            ZipArchive::new(Cursor::new(&bytes)).map_err(|err| format!("zip 读取失败：{err}"))?;
        if find_entry_index(&mut archive, ".lua")?.is_none() {
            return Err("压缩包里没有找到 .lua 文件".to_string());
        }
        return Ok(bytes);
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "清单不是有效的 ZIP 或 Lua 文件".to_string())?;
    if !text
        .trim_start_matches('\u{feff}')
        .lines()
        .any(|line| line.trim_start().starts_with("addappid("))
    {
        return Err("清单不是有效的 ZIP 或 Lua 文件".to_string());
    }
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("package");
    let name = format!("{stem}.lua");
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file(name, zip::write::SimpleFileOptions::default())
        .map_err(|err| format!("保存 Lua 失败：{err}"))?;
    archive
        .write_all(&bytes)
        .map_err(|err| format!("保存 Lua 失败：{err}"))?;
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|err| format!("保存 Lua 失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_lua_becomes_a_valid_archive_without_changing_its_contents() {
        let lua = b"-- Generated with Luie\naddappid(1086940)\nsetManifestid(1419660, \"123\")\n"
            .to_vec();
        let bytes = normalize_archive("1086940.lua", lua.clone()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
        assert_eq!(archive.len(), 1);
        let mut entry = archive.by_name("1086940.lua").unwrap();
        let mut output = Vec::new();
        entry.read_to_end(&mut output).unwrap();
        assert_eq!(output, lua);
        assert_eq!(
            normalize_archive("1086940.zip", bytes.clone()).unwrap(),
            bytes
        );
    }

    #[test]
    fn invalid_downloads_are_rejected_before_package_replacement() {
        for bytes in [
            b"<html>Access denied</html>".to_vec(),
            b"{\"error\":\"no manifest\"}".to_vec(),
            b"PK broken zip".to_vec(),
            vec![255],
        ] {
            assert!(normalize_archive("1086940.lua", bytes).is_err());
        }
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("file.manifest", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"manifest").unwrap();
        assert!(normalize_archive("1086940.zip", archive.finish().unwrap().into_inner()).is_err());
    }
}
