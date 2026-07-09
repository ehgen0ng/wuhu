use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose, Engine as _};
use tauri::AppHandle;

use crate::{
    models::{AppState, AppStore, TicketItem},
    packages,
    state::build_state,
    steam,
    store::{load_store, portable_data_dir, save_store},
};

mod parser;
mod steam_client;

use parser::{build_lua_lines, build_tickets_txt, parse_tickets_txt, TicketData};

const TICKET_VALID_SECONDS: u64 = 30 * 60;

pub(crate) fn extract_ticket(
    _app: &AppHandle,
    app_id: u32,
    title: String,
) -> Result<AppState, String> {
    if app_id == 0 {
        return Err("AppID 无效".to_string());
    }

    let mut store = load_store()?;
    let steam_path = steam::configured_path(&store)
        .ok_or_else(|| "请先在设置里配置 Steam 路径".to_string())?
        .to_string();
    let steam_root = Path::new(&steam_path);
    if !steam::looks_like_root(steam_root) {
        return Err("Steam 路径不像 Steam 根目录，请检查设置".to_string());
    }

    let extracted = steam_client::extract(steam_root, app_id)?;
    if extracted.app_ticket.is_none() && extracted.e_ticket.is_none() {
        return Err(
            "没有提取到 AppTicket 或 ETicket，请确认 Steam 已运行且账号拥有该游戏".to_string(),
        );
    }

    let ticket_data = TicketData {
        app_id,
        app_ticket: extracted.app_ticket,
        e_ticket: extracted.e_ticket,
    };
    let file_name = format!("{app_id}.tickets.txt");
    upsert_ticket(
        &mut store,
        ticket_data,
        title,
        Some(file_name),
        now_seconds(),
    )?;
    save_store(&store)?;
    packages::sync_enabled_packages_for_app_id(&store, app_id)?;
    build_state(store)
}

pub(crate) fn import_tickets_txt(
    _app: &AppHandle,
    file_name: String,
    data_base64: String,
) -> Result<AppState, String> {
    let bytes = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("tickets.txt 数据解码失败：{err}"))?;
    let text =
        String::from_utf8(bytes).map_err(|err| format!("tickets.txt 不是有效文本：{err}"))?;
    let ticket_data = parse_tickets_txt(&text)?;
    if ticket_data.app_ticket.is_none() && ticket_data.e_ticket.is_none() {
        return Err("tickets.txt 里没有可用的 AppTicket 或 ETicket".to_string());
    }

    let mut store = load_store()?;
    let title = store
        .packages
        .iter()
        .find(|package| package.app_id == Some(ticket_data.app_id))
        .map(|package| package.title.clone())
        .or_else(|| {
            store
                .tickets
                .iter()
                .find(|ticket| ticket.app_id == ticket_data.app_id)
                .map(|ticket| ticket.title.clone())
        })
        .unwrap_or_else(|| ticket_data.app_id.to_string());
    let app_id = ticket_data.app_id;
    upsert_ticket(
        &mut store,
        ticket_data,
        title,
        Some(file_name),
        now_seconds(),
    )?;
    save_store(&store)?;
    packages::sync_enabled_packages_for_app_id(&store, app_id)?;
    build_state(store)
}

pub(crate) fn export_tickets_txt(
    _app: &AppHandle,
    app_id: u32,
    path: String,
) -> Result<(), String> {
    let data = read_ticket_data(app_id)?;
    let text = build_tickets_txt(&data);
    fs::write(path, text).map_err(|err| format!("导出 tickets.txt 失败：{err}"))
}

pub(crate) fn delete_ticket(_app: &AppHandle, app_id: u32) -> Result<AppState, String> {
    let mut store = load_store()?;
    let before = store.tickets.len();
    store.tickets.retain(|ticket| ticket.app_id != app_id);
    if before == store.tickets.len() {
        return Err("没有找到这个 ticket".to_string());
    }

    let dir = ticket_dir(app_id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| format!("删除 ticket 文件失败：{err}"))?;
    }

    save_store(&store)?;
    packages::sync_enabled_packages_for_app_id(&store, app_id)?;
    build_state(store)
}

pub(crate) fn lua_for_app_id(app_id: u32) -> Result<Option<String>, String> {
    let data = read_ticket_data(app_id)?;
    Ok(build_lua_lines(&data))
}

fn upsert_ticket(
    store: &mut AppStore,
    data: TicketData,
    title: String,
    source_file_name: Option<String>,
    extracted_at: u64,
) -> Result<(), String> {
    write_ticket_files(&data)?;
    let item = TicketItem {
        app_id: data.app_id,
        title: normalize_title(&title, data.app_id),
        has_app_ticket: data.app_ticket.is_some(),
        has_e_ticket: data.e_ticket.is_some(),
        extracted_at,
        expires_at: data
            .e_ticket
            .as_ref()
            .map(|_| extracted_at.saturating_add(TICKET_VALID_SECONDS)),
        source_file_name,
    };

    if let Some(existing) = store
        .tickets
        .iter_mut()
        .find(|ticket| ticket.app_id == item.app_id)
    {
        *existing = item;
    } else {
        store.tickets.push(item);
    }
    store.tickets.sort_by(|left, right| {
        left.title
            .cmp(&right.title)
            .then(left.app_id.cmp(&right.app_id))
    });
    Ok(())
}

fn write_ticket_files(data: &TicketData) -> Result<(), String> {
    let dir = ticket_dir(data.app_id)?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建 ticket 目录失败：{err}"))?;
    if let Some(app_ticket) = data.app_ticket.as_deref() {
        fs::write(dir.join("appticket.bin"), app_ticket)
            .map_err(|err| format!("保存 AppTicket 失败：{err}"))?;
    } else {
        remove_if_exists(&dir.join("appticket.bin"))?;
    }
    if let Some(e_ticket) = data.e_ticket.as_deref() {
        fs::write(dir.join("eticket.bin"), e_ticket)
            .map_err(|err| format!("保存 ETicket 失败：{err}"))?;
    } else {
        remove_if_exists(&dir.join("eticket.bin"))?;
    }
    fs::write(dir.join("tickets.txt"), build_tickets_txt(data))
        .map_err(|err| format!("保存 tickets.txt 失败：{err}"))?;
    Ok(())
}

fn read_ticket_data(app_id: u32) -> Result<TicketData, String> {
    let path = ticket_dir(app_id)?.join("tickets.txt");
    let text = fs::read_to_string(&path).map_err(|err| format!("读取 tickets.txt 失败：{err}"))?;
    let data = parse_tickets_txt(&text)?;
    if data.app_id != app_id {
        return Err("tickets.txt 的 AppID 与请求不一致".to_string());
    }
    Ok(data)
}

fn ticket_dir(app_id: u32) -> Result<std::path::PathBuf, String> {
    Ok(portable_data_dir()?
        .join("tickets")
        .join(app_id.to_string()))
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|err| format!("删除旧 ticket 文件失败：{err}"))?;
    }
    Ok(())
}

fn normalize_title(title: &str, app_id: u32) -> String {
    let title = title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if title.is_empty() {
        app_id.to_string()
    } else {
        title
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
