use serde::Serialize;

use crate::{
    luatools_auth::{self as auth, ANON_KEY, API_URL, AUTH_URL},
    models::ManifestStatus,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LuaToolsQuota {
    pub(crate) daily_usage: u64,
    pub(crate) daily_limit: Option<u64>,
}

pub(crate) async fn fetch_status(
    client: &reqwest::Client,
    app_id: u32,
) -> Result<ManifestStatus, String> {
    let response = auth::send(
        client
            .get(format!("{API_URL}/api/manifest/check"))
            .query(&[("appid", app_id)]),
    )
    .await?;
    auth::check_response(&response)?;
    let sources: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "解析 LuaTools 来源失败".to_string())?;
    status_from_sources(app_id, &sources)
}

fn status_from_sources(app_id: u32, sources: &serde_json::Value) -> Result<ManifestStatus, String> {
    let sources = sources
        .as_object()
        .ok_or_else(|| "解析 LuaTools 来源失败".to_string())?;
    let mut available: Vec<_> = sources
        .iter()
        .filter(|(name, status)| {
            status.as_str() == Some("available") && name.as_str() != "Sadie (Morrenus)"
        })
        .map(|(name, _)| name.clone())
        .collect();
    // The website's source metadata marks Luie as recommended; the API only reports availability.
    available.sort_by_key(|name| (name != "Luie", name.clone()));
    let source = available.into_iter().next();
    let available = source.is_some();
    Ok(ManifestStatus {
        provider: "luatools".to_string(),
        source,
        app_id,
        game_name: None,
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
        // /api/steam/details.releaseDate is the game's release date, not a manifest timestamp.
        file_modified: None,
        error: None,
    })
}

pub(crate) async fn download(
    client: &reqwest::Client,
    mut status: ManifestStatus,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let source = status
        .source
        .as_deref()
        .ok_or_else(|| "当前没有可用清单".to_string())?;
    let response = auth::send(
        client
            .get(format!("{API_URL}/api/manifest/download"))
            .query(&[
                ("appid", status.app_id.to_string()),
                ("source", source.to_string()),
            ]),
    )
    .await?;
    auth::check_response(&response)?;
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "读取清单失败".to_string())?
        .to_vec();
    status.file_size = Some(bytes.len() as u64);
    Ok((bytes, status))
}

pub(crate) async fn fetch_quota() -> Result<LuaToolsQuota, String> {
    let client = auth::client()?;
    let today = time::OffsetDateTime::now_utc().date();
    let start = format!(
        "gte.{:04}-{:02}-{:02}T00:00:00Z",
        today.year(),
        today.month() as u8,
        today.day()
    );
    let response = auth::send(
        client
            .head(format!("{AUTH_URL}/rest/v1/user_downloads"))
            .query(&[("select", "appid"), ("downloaded_at", start.as_str())])
            .header("apikey", ANON_KEY)
            .header("Prefer", "count=exact"),
    )
    .await?;
    auth::check_response(&response)?;
    let used = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .and_then(|range| range.rsplit_once('/'))
        .and_then(|(_, count)| count.parse().ok())
        .ok_or_else(|| "LuaTools 没有返回当天用量".to_string())?;
    let response = auth::send(client.get(format!("{API_URL}/api/me/supporter-status"))).await?;
    auth::check_response(&response)?;
    let supporter: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "解析 LuaTools 额度失败".to_string())?;
    let supporter = supporter
        .get("isSupporter")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| "解析 LuaTools 额度失败".to_string())?;
    // Matches LuaTools' current client/web limit; the counting endpoint has no limit field.
    Ok(LuaToolsQuota {
        daily_usage: used,
        daily_limit: if supporter { None } else { Some(25) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_source_wins_and_no_date_is_invented() {
        let status = status_from_sources(
            1086940,
            &serde_json::json!({"Ryuu":"available","Luie":"available"}),
        )
        .unwrap();
        assert_eq!(status.source.as_deref(), Some("Luie"));
        assert!(status.available);
        assert!(status.file_modified.is_none());
        let fallback = status_from_sources(
            1086940,
            &serde_json::json!({"Luie":"unavailable","Ryuu":"available"}),
        )
        .unwrap();
        assert_eq!(fallback.source.as_deref(), Some("Ryuu"));
        assert!(
            !status_from_sources(1086940, &serde_json::json!({"Luie":"unavailable"}))
                .unwrap()
                .available
        );
        assert!(status_from_sources(1086940, &serde_json::json!([])).is_err());
    }
}
