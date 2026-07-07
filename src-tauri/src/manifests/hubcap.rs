use std::time::Duration;

use crate::{
    manifests::shared::{
        api_error_detail, detail_prefix, parse_api_error_detail, value_as_bool, value_as_string,
        value_as_u32, value_as_u64,
    },
    models::{HubcapQuota, ManifestStatus},
    net::HTTP_USER_AGENT,
};

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建清单请求失败：{err}"))
}

pub(crate) async fn fetch_status(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<ManifestStatus, String> {
    let response = client
        .get(format!("https://hubcapmanifest.com/api/v1/status/{app_id}"))
        .bearer_auth(api_key)
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

    if !status_code.is_success() {
        return Ok(ManifestStatus {
            provider: "hubcap".to_string(),
            app_id,
            game_name: None,
            status: Some(status_code.to_string()),
            available: false,
            manifest_file_exists: false,
            update_in_progress: None,
            needs_update: None,
            file_size: None,
            file_modified: None,
            error: parse_api_error_detail(&text),
        });
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析清单状态失败：{err}"))?;
    let manifest_file_exists = value_as_bool(&json, "manifest_file_exists").unwrap_or(false);
    let status = value_as_string(&json, "status");
    let update_in_progress = value_as_bool(&json, "update_in_progress");

    Ok(ManifestStatus {
        provider: "hubcap".to_string(),
        app_id: value_as_u32(&json, "app_id").unwrap_or(app_id),
        game_name: value_as_string(&json, "game_name")
            .or_else(|| value_as_string(&json, "app_name")),
        available: manifest_file_exists
            && status
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("available"))
                .unwrap_or(false)
            && !update_in_progress.unwrap_or(false),
        status,
        manifest_file_exists,
        update_in_progress,
        needs_update: value_as_bool(&json, "needs_update"),
        file_size: value_as_u64(&json, "file_size"),
        file_modified: value_as_string(&json, "file_modified")
            .or_else(|| value_as_string(&json, "updated_at"))
            .or_else(|| value_as_string(&json, "last_modified")),
        error: None,
    })
}

pub(crate) async fn download(
    client: &reqwest::Client,
    api_key: &str,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let status = fetch_status(client, api_key, app_id).await?;
    if !status.available || status.update_in_progress.unwrap_or(false) {
        return Err("当前没有可用清单".to_string());
    }

    let response = client
        .get(format!(
            "https://hubcapmanifest.com/api/v1/manifest/{app_id}"
        ))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("下载清单失败：{err}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        let detail = api_error_detail(response).await;
        return Err(format!(
            "下载清单失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取清单失败：{err}"))?
        .to_vec();

    Ok((bytes, status))
}

pub(crate) async fn fetch_quota(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<HubcapQuota, String> {
    let response = client
        .get("https://hubcapmanifest.com/api/v1/user/stats")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("读取额度失败：{err}"))?;

    let status_code = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("读取额度失败：{err}"))?;

    if status_code == reqwest::StatusCode::UNAUTHORIZED
        || status_code == reqwest::StatusCode::FORBIDDEN
    {
        let detail = parse_api_error_detail(&text);
        return Err(format!("Key 无效或无权限{}", detail_prefix(&detail)));
    }

    if !status_code.is_success() {
        let detail = parse_api_error_detail(&text);
        return Err(format!(
            "读取额度失败：HTTP {status_code}{}",
            detail_prefix(&detail)
        ));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|err| format!("解析额度失败：{err}"))?;

    Ok(HubcapQuota {
        daily_usage: value_as_u64(&json, "daily_usage").unwrap_or(0),
        daily_limit: value_as_u64(&json, "daily_limit").unwrap_or(0),
    })
}
