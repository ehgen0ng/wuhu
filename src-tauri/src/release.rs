use std::time::Duration;

use serde::Deserialize;

use crate::{models::AppRelease, net::HTTP_USER_AGENT};

const RELEASE_REPOSITORY: &str = "ehgen0ng/wuhu";

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

pub(crate) async fn get_latest_app_release() -> Result<AppRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("创建版本检查请求失败：{err}"))?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{RELEASE_REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("检查版本失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("检查版本失败：HTTP {}", response.status()));
    }

    let release = response
        .json::<GithubReleaseResponse>()
        .await
        .map_err(|err| format!("解析版本信息失败：{err}"))?;
    let version = release.tag_name.trim().trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err("版本信息为空".to_string());
    }

    Ok(AppRelease {
        version,
        name: release.name,
        url: release.html_url,
    })
}
