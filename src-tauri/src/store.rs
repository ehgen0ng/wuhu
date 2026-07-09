use std::{
    env,
    fs,
    path::PathBuf,
};

use crate::models::AppStore;

#[cfg(target_os = "macos")]
const MACOS_DATA_DIR_NAME: &str = "wuhu";
const STORE_FILE: &str = "state.json";

pub(crate) fn portable_data_dir() -> Result<PathBuf, String> {
    let path = data_root_dir()?;
    fs::create_dir_all(&path).map_err(|err| format!("创建应用数据目录失败：{err}"))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn data_root_dir() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or_else(|| "获取用户主目录失败".to_string())?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join(MACOS_DATA_DIR_NAME))
}

#[cfg(not(target_os = "macos"))]
fn data_root_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|err| format!("获取程序路径失败：{err}"))?;
    let base_dir = exe_path
        .parent()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| "获取程序目录失败".to_string())?;
    Ok(base_dir.join("data"))
}

pub(crate) fn load_store() -> Result<AppStore, String> {
    let path = portable_data_dir()?.join(STORE_FILE);
    if !path.exists() {
        return Ok(AppStore::default());
    }
    let data = fs::read_to_string(&path).map_err(|err| format!("读取状态文件失败：{err}"))?;
    serde_json::from_str(&data).map_err(|err| format!("解析状态文件失败：{err}"))
}

pub(crate) fn save_store(store: &AppStore) -> Result<(), String> {
    let path = portable_data_dir()?.join(STORE_FILE);
    let data =
        serde_json::to_string_pretty(store).map_err(|err| format!("序列化状态失败：{err}"))?;
    fs::write(path, data).map_err(|err| format!("保存状态文件失败：{err}"))
}
