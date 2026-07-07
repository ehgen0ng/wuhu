use std::time::Duration;

use serde::Serialize;

use crate::{
    manifests::shared::{
        api_error_detail, detail_prefix, parse_api_error_detail, value_as_bool, value_as_string,
        value_as_u32,
    },
    models::ManifestStatus,
    net::HTTP_USER_AGENT,
};

const DOWNLOAD_POLL_LIMIT: usize = 60;

#[derive(Debug, Serialize)]
struct BatchAvailabilityRequest {
    appids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DownloadRequest {
    appid: String,
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建清单请求失败：{err}"))
}

pub(crate) async fn fetch_statuses(
    client: &reqwest::Client,
    api_key: &str,
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let unique_app_ids: Vec<u32> = app_ids
        .into_iter()
        .filter(|app_id| *app_id > 0)
        .take(100)
        .collect();
    if unique_app_ids.is_empty() {
        return Ok(Vec::new());
    }

    let body = BatchAvailabilityRequest {
        appids: unique_app_ids.iter().map(ToString::to_string).collect(),
    };
    let response = client
        .post("https://depotbox.org/api/games/batch-availability")
        .header("X-API-Key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("检查清单失败：{err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("读取清单状态失败：{err}"))?;

    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        let detail = parse_api_error_detail(&text);
        return Err(format!("Key 无效或无权限{}", detail_prefix(&detail)));
    }

    if status_code == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let detail = parse_api_error_detail(&text);
        return Err(format!("请求过快，请稍后再试{}", detail_prefix(&detail)));
    }

    if !status_code.is_success() {
        let detail = parse_api_error_detail(&text);
        return Err(format!(
            "检查清单失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析清单状态失败：{err}"))?;
    let results = json
        .get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut by_app_id = std::collections::HashMap::new();
    for value in results {
        let Some(app_id) = value_as_u32(&value, "appid") else {
            continue;
        };
        by_app_id.insert(app_id, status_from_value(app_id, &value));
    }

    Ok(unique_app_ids
        .into_iter()
        .map(|app_id| {
            by_app_id.remove(&app_id).unwrap_or_else(|| ManifestStatus {
                provider: "depotbox".to_string(),
                app_id,
                game_name: None,
                status: Some("missing".to_string()),
                available: false,
                manifest_file_exists: false,
                update_in_progress: None,
                needs_update: None,
                file_size: None,
                file_modified: None,
                error: Some("没有返回这个 AppID 的清单状态。".to_string()),
            })
        })
        .collect())
}

pub(crate) async fn download(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let mut statuses = fetch_statuses(client, api_key, vec![app_id]).await?;
    let mut status = statuses
        .pop()
        .ok_or_else(|| "没有返回清单状态。".to_string())?;
    if !status.available {
        return Err(status
            .error
            .clone()
            .unwrap_or_else(|| "当前没有可用清单".to_string()));
    }

    let body = DownloadRequest {
        appid: app_id.to_string(),
    };
    let response = client
        .post("https://depotbox.org/api/download")
        .header("X-API-Key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = api_error_detail(response).await;
        return Err(download_error_message(status_code, detail));
    }

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| format!("解析下载任务失败：{err}"))?;
    let token = value_as_string(&json, "token")
        .ok_or_else(|| "下载任务没有返回 token，请稍后重试。".to_string())?;
    let download_link = poll_download(client, api_key, &token).await?;
    let bytes = fetch_download(client, api_key, &download_link).await?;
    status.file_size = Some(bytes.len() as u64);

    Ok((bytes, status))
}

fn status_from_value(app_id: u32, value: &serde_json::Value) -> ManifestStatus {
    let source_available = value
        .get("sources")
        .and_then(serde_json::Value::as_object)
        .map(|sources| {
            sources
                .values()
                .any(|value| value.as_bool().unwrap_or(false))
        })
        .unwrap_or(false);
    let available = value_as_bool(value, "available").unwrap_or(source_available);

    ManifestStatus {
        provider: "depotbox".to_string(),
        app_id,
        game_name: value_as_string(value, "name").or_else(|| value_as_string(value, "game_name")),
        status: Some(
            if available {
                "available"
            } else {
                "unavailable"
            }
            .to_string(),
        ),
        available,
        manifest_file_exists: available,
        update_in_progress: None,
        needs_update: None,
        file_size: None,
        file_modified: None,
        error: None,
    }
}

async fn poll_download(
    client: &reqwest::Client,
    api_key: &str,
    token: &str,
) -> Result<String, String> {
    for _ in 0..DOWNLOAD_POLL_LIMIT {
        let response = client
            .get(format!("https://depotbox.org/api/status/{token}"))
            .header("X-API-Key", api_key)
            .send()
            .await
            .map_err(|err| format!("检查下载进度失败：{err}"))?;

        let status_code = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| format!("读取下载进度失败：{err}"))?;
        if !status_code.is_success() {
            return Err(download_error_message(
                status_code,
                parse_api_error_detail(&text),
            ));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| format!("解析下载进度失败：{err}"))?;
        match value_as_string(&json, "status").as_deref() {
            Some("completed") => {
                return value_as_string(&json, "download_link")
                    .ok_or_else(|| "下载完成但没有返回链接，请稍后重试。".to_string());
            }
            Some("failed") => return Err(failed_status_message(&json)),
            _ => tokio::time::sleep(Duration::from_millis(1500)).await,
        }
    }

    Err("清单打包超时，请稍后重试。".to_string())
}

async fn fetch_download(
    client: &reqwest::Client,
    api_key: &str,
    download_link: &str,
) -> Result<Vec<u8>, String> {
    let url = if download_link.starts_with("http://") || download_link.starts_with("https://") {
        download_link.to_string()
    } else {
        format!("https://depotbox.org{download_link}")
    };
    let response = client
        .get(url)
        .header("X-API-Key", api_key)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = api_error_detail(response).await;
        return Err(download_error_message(status_code, detail));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("读取清单失败：{err}"))
}

fn failed_status_message(json: &serde_json::Value) -> String {
    let reason = value_as_string(json, "failureReason").unwrap_or_default();
    let message = value_as_string(json, "message").unwrap_or_default();
    let log_text = json
        .get("logs")
        .and_then(serde_json::Value::as_array)
        .map(|logs| {
            logs.iter()
                .filter_map(|log| value_as_string(log, "message"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let lower = format!("{reason}\n{message}\n{log_text}").to_ascii_lowercase();

    if lower.contains("manifest_not_found")
        || lower.contains("no manifest")
        || lower.contains("main depot key")
        || lower.contains("app access token")
    {
        return "当前没有可下载清单。".to_string();
    }

    "清单下载失败，请稍后重试。".to_string()
}

fn download_error_message(status_code: reqwest::StatusCode, detail: Option<String>) -> String {
    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        return format!("Key 无效或无权限{}", detail_prefix(&detail));
    }
    if status_code == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return format!("请求过快，请稍后再试{}", detail_prefix(&detail));
    }

    let lower = detail.as_deref().unwrap_or_default().to_ascii_lowercase();
    if status_code == reqwest::StatusCode::NOT_FOUND
        || lower.contains("manifest_not_found")
        || lower.contains("manifest for the requested appid could not be found")
    {
        return "当前没有可下载清单。".to_string();
    }

    format!("下载清单失败：HTTP {status_code}{}", detail_prefix(&detail))
}
