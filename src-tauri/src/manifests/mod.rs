mod depotbox;
mod hubcap;
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

pub(crate) async fn download_preferred_manifest(
    store: &AppStore,
    app_id: u32,
) -> Result<(Vec<u8>, ManifestStatus), String> {
    let hubcap_key = trimmed_api_key(store.settings.hubcap_api_key.as_deref());
    let depotbox_key = trimmed_api_key(store.settings.depotbox_api_key.as_deref());

    if hubcap_key.is_none() && depotbox_key.is_none() {
        return Err("请先在设置里保存 Key".to_string());
    }

    let mut hubcap_error = None;
    if let Some(api_key) = hubcap_key.as_deref() {
        let client = hubcap::client()?;
        match hubcap::download(&client, api_key, app_id).await {
            Ok(result) => return Ok(result),
            Err(err) => hubcap_error = Some(err),
        }
    }

    if let Some(api_key) = depotbox_key.as_deref() {
        let client = depotbox::client()?;
        return depotbox::download(&client, api_key, app_id).await;
    }

    Err(hubcap_error.unwrap_or_else(|| "当前没有可用清单".to_string()))
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
