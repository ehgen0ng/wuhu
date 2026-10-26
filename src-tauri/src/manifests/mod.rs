mod depotbox;
mod hubcap;
pub(crate) mod luatools;
mod shared;

use crate::models::{AppStore, HubcapQuota, ManifestStatus};

pub(crate) async fn check_hubcap_manifest_statuses(
    store: &AppStore,
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let api_key = api_key(store.settings.hubcap_api_key.as_deref())?;
    let client = hubcap::client()?;
    let mut statuses = Vec::new();

    for app_id in app_ids.into_iter().filter(|app_id| *app_id > 0).take(24) {
        statuses.push(hubcap::fetch_status(&client, &api_key, app_id).await?);
    }

    Ok(statuses)
}

pub(crate) async fn check_depotbox_manifest_statuses(
    store: &AppStore,
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let api_key = api_key(store.settings.depotbox_api_key.as_deref())?;
    let client = depotbox::client()?;
    depotbox::fetch_statuses(&client, &api_key, app_ids).await
}

pub(crate) async fn get_hubcap_quota(store: &AppStore) -> Result<HubcapQuota, String> {
    let api_key = api_key(store.settings.hubcap_api_key.as_deref())?;
    let client = hubcap::client()?;
    hubcap::fetch_quota(&client, &api_key).await
}

// Search and download share this selection policy, so the displayed source matches the download.
async fn candidates(store: &AppStore, app_id: u32, include_depotbox: bool) -> Vec<ManifestStatus> {
    let mut statuses = Vec::new();
    if let Some(key) = trimmed_api_key(store.settings.hubcap_api_key.as_deref()) {
        let result = match hubcap::client() {
            Ok(client) => hubcap::fetch_status(&client, &key, app_id).await,
            Err(error) => Err(error),
        };
        statuses.push(result.unwrap_or_else(|error| failed_status("hubcap", app_id, error)));
    }
    if crate::luatools_auth::state().ok().flatten().is_some() {
        let result = match crate::luatools_auth::client() {
            Ok(client) => luatools::fetch_status(&client, app_id).await,
            Err(error) => Err(error),
        };
        statuses.push(result.unwrap_or_else(|error| failed_status("luatools", app_id, error)));
    }
    if let Some(key) =
        trimmed_api_key(store.settings.depotbox_api_key.as_deref()).filter(|_| include_depotbox)
    {
        let result = match depotbox::client() {
            Ok(client) => depotbox::fetch_statuses(&client, &key, vec![app_id])
                .await
                .and_then(|mut statuses| {
                    statuses
                        .pop()
                        .ok_or_else(|| "没有返回清单状态。".to_string())
                }),
            Err(error) => Err(error),
        };
        statuses.push(result.unwrap_or_else(|error| failed_status("depotbox", app_id, error)));
    }
    statuses
}

fn failed_status(provider: &str, app_id: u32, error: String) -> ManifestStatus {
    ManifestStatus {
        provider: provider.to_string(),
        source: None,
        app_id,
        game_name: None,
        status: None,
        available: false,
        manifest_file_exists: false,
        update_in_progress: None,
        needs_update: None,
        file_size: None,
        file_modified: None,
        error: Some(error),
    }
}

fn timestamp(status: &ManifestStatus) -> Option<i64> {
    let value = status.file_modified.as_deref()?;
    // Hubcap reports UTC ISO timestamps, sometimes without the explicit timezone suffix.
    let normalized = value.replace(' ', "T");
    let parsed =
        time::OffsetDateTime::parse(&normalized, &time::format_description::well_known::Rfc3339)
            .or_else(|_| {
                time::OffsetDateTime::parse(
                    &format!("{normalized}Z"),
                    &time::format_description::well_known::Rfc3339,
                )
            });
    parsed.ok().map(|value| value.unix_timestamp())
}

fn best_index(statuses: &[ManifestStatus]) -> Option<usize> {
    let mut best = None;
    for (index, candidate) in statuses.iter().enumerate() {
        if !candidate.available
            || !candidate.manifest_file_exists
            || candidate.update_in_progress == Some(true)
        {
            continue;
        }
        match best {
            None => best = Some(index),
            Some(current) => {
                // An unknown timestamp is not evidence that a file is older or newer.
                if let (Some(candidate_time), Some(current_time)) =
                    (timestamp(candidate), timestamp(&statuses[current]))
                {
                    if candidate_time > current_time {
                        best = Some(index);
                    }
                }
            }
        }
    }
    best
}

pub(crate) async fn check_preferred_manifest_statuses(
    store: &AppStore,
    app_ids: Vec<u32>,
) -> Result<Vec<ManifestStatus>, String> {
    let mut ids = Vec::new();
    for id in app_ids.into_iter().filter(|id| *id > 0).take(24) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut results = Vec::new();
    // Bound upstream concurrency; credentials may refresh once under the shared session lock.
    for batch in ids.chunks(4) {
        let mut tasks = tokio::task::JoinSet::new();
        for &app_id in batch {
            let store = store.clone();
            tasks.spawn(async move {
                let mut statuses = candidates(&store, app_id, false).await;
                let index = best_index(&statuses)
                    .or_else(|| statuses.iter().position(|s| s.error.is_some()))
                    .unwrap_or(0);
                if statuses.is_empty() {
                    return Ok::<_, String>(failed_status(
                        "depotbox",
                        app_id,
                        "当前没有可用清单".to_string(),
                    ));
                }
                Ok(statuses.remove(index))
            });
        }
        while let Some(result) = tasks.join_next().await {
            results.push(result.map_err(|_| "检查清单失败".to_string())??);
        }
    }
    if let Some(key) = trimmed_api_key(store.settings.depotbox_api_key.as_deref()) {
        let missing: Vec<_> = results
            .iter()
            .filter(|status| !status.available)
            .map(|status| status.app_id)
            .collect();
        if !missing.is_empty() {
            let fallback = depotbox::fetch_statuses(&depotbox::client()?, &key, missing).await;
            match fallback {
                Ok(statuses) => {
                    for status in statuses {
                        if let Some(current) = results
                            .iter_mut()
                            .find(|current| current.app_id == status.app_id)
                        {
                            if status.available || current.provider == "depotbox" {
                                *current = status;
                            }
                        }
                    }
                }
                Err(error) => {
                    for current in results
                        .iter_mut()
                        .filter(|status| status.provider == "depotbox")
                    {
                        current.error = Some(error.clone());
                    }
                }
            }
        }
    }
    Ok(results)
}

pub(crate) async fn download_preferred_manifest(
    store: &AppStore,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let statuses = candidates(store, app_id, true).await;
    download_with_fallback(statuses, app_id, |status| async move {
        match status.provider.as_str() {
            "luatools" => luatools::download(&crate::luatools_auth::client()?, status).await,
            "hubcap" => {
                hubcap::download(
                    &hubcap::client()?,
                    &api_key(store.settings.hubcap_api_key.as_deref())?,
                    app_id,
                )
                .await
            }
            "depotbox" => {
                depotbox::download(
                    &depotbox::client()?,
                    &api_key(store.settings.depotbox_api_key.as_deref())?,
                    app_id,
                )
                .await
            }
            _ => Err("当前没有可用清单".to_string()),
        }
    })
    .await
}

async fn download_with_fallback<F, Fut>(
    mut statuses: Vec<ManifestStatus>,
    app_id: u32,
    mut download: F,
) -> Result<(Vec<u8>, ManifestStatus), String>
where
    F: FnMut(ManifestStatus) -> Fut,
    Fut: std::future::Future<Output = Result<(Vec<u8>, ManifestStatus), String>>,
{
    let mut error = statuses
        .iter()
        .find_map(|status| status.error.clone())
        .unwrap_or_else(|| "当前没有可用清单".to_string());
    while let Some(index) = best_index(&statuses) {
        let status = statuses.remove(index);
        // Quota exhaustion is a provider failure too: continue with the next configured candidate.
        match download(status).await {
            Ok((bytes, status)) => {
                match crate::packages::normalize_archive(&format!("{app_id}.lua"), bytes) {
                    Ok(bytes) => return Ok((bytes, status)),
                    Err(detail) => error = detail,
                }
            }
            Err(detail) => error = detail,
        }
    }
    Err(error)
}

fn api_key(value: Option<&str>) -> Result<String, String> {
    trimmed_api_key(value).ok_or_else(|| "请先在设置里保存 Key".to_string())
}

fn trimmed_api_key(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(provider: &str, date: Option<&str>) -> ManifestStatus {
        let mut status = failed_status(provider, 1086940, String::new());
        status.error = None;
        status.status = Some("available".to_string());
        status.available = true;
        status.manifest_file_exists = true;
        status.file_modified = date.map(str::to_string);
        status
    }

    #[test]
    fn newest_known_timestamp_wins_even_with_different_offsets() {
        let statuses = vec![
            available("luatools", Some("2026-10-26T10:00:00+08:00")),
            available("hubcap", Some("2026-10-26T03:00:00Z")),
        ];
        assert_eq!(best_index(&statuses), Some(1));
        assert_eq!(
            timestamp(&available("hubcap", Some("2026-10-26 03:00:00"))),
            timestamp(&statuses[1])
        );
    }

    #[test]
    fn missing_or_invalid_dates_preserve_priority_instead_of_claiming_freshness() {
        for date in [None, Some("unknown")] {
            assert_eq!(
                best_index(&[
                    available("luatools", date),
                    available("hubcap", Some("2026-10-26T03:00:00Z"))
                ]),
                Some(0)
            );
        }
    }

    #[test]
    fn failed_or_updating_sources_fall_back_to_available_candidates() {
        let mut updating = available("luatools", None);
        updating.update_in_progress = Some(true);
        let failed = failed_status("hubcap", 1086940, "network error".to_string());
        assert_eq!(
            best_index(&[updating, failed, available("depotbox", None)]),
            Some(2)
        );
        assert_eq!(best_index(&[]), None);
    }
    #[test]
    fn quota_exhaustion_falls_back_in_order_and_stops_after_success() {
        let calls = std::cell::RefCell::new(Vec::new());
        let statuses = vec![
            available("hubcap", None),
            available("luatools", None),
            available("depotbox", None),
        ];
        let result =
            tauri::async_runtime::block_on(download_with_fallback(statuses, 1086940, |status| {
                calls.borrow_mut().push(status.provider.clone());
                let result = if status.provider != "depotbox" {
                    Err("HTTP 429: daily quota exhausted".to_string())
                } else {
                    Ok((b"addappid(1086940)".to_vec(), status))
                };
                std::future::ready(result)
            }))
            .unwrap();
        assert_eq!(*calls.borrow(), vec!["hubcap", "luatools", "depotbox"]);
        assert_eq!(result.1.provider, "depotbox");
        let calls = std::cell::RefCell::new(Vec::new());
        let statuses = vec![available("hubcap", None), available("luatools", None)];
        tauri::async_runtime::block_on(download_with_fallback(statuses, 1086940, |status| {
            calls.borrow_mut().push(status.provider.clone());
            std::future::ready(Ok((b"addappid(1086940)".to_vec(), status)))
        }))
        .unwrap();
        assert_eq!(*calls.borrow(), vec!["hubcap"]);
    }

    #[test]
    fn all_quotas_exhausted_returns_an_error_instead_of_importing_anything() {
        let statuses = vec![available("hubcap", None), available("luatools", None)];
        let result =
            tauri::async_runtime::block_on(download_with_fallback(statuses, 1086940, |_| {
                std::future::ready(Err("HTTP 429: daily quota exhausted".to_string()))
            }));
        assert!(result.is_err());
    }
}
