use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::models::AppStore;

const STORE_FILE: &str = "state.json";

pub(crate) fn portable_data_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|err| format!("获取程序路径失败：{err}"))?;
    let base_dir = exe_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "获取程序目录失败".to_string())?;
    let path = base_dir.join("data");
    fs::create_dir_all(&path).map_err(|err| format!("创建应用数据目录失败：{err}"))?;
    Ok(path)
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
