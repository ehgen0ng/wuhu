use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppStore {
    pub(crate) settings: AppSettings,
    pub(crate) packages: Vec<PackageItem>,
    #[serde(default)]
    pub(crate) tickets: Vec<TicketItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    #[serde(default)]
    pub(crate) steam_path: Option<String>,
    #[serde(default)]
    pub(crate) hubcap_api_key: Option<String>,
    #[serde(default)]
    pub(crate) depotbox_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageItem {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) app_id: Option<u32>,
    pub(crate) lua_file_name: String,
    pub(crate) manifest_files: Vec<String>,
    pub(crate) source_zip_name: String,
    pub(crate) enabled: bool,
    pub(crate) imported_at: u64,
    #[serde(default)]
    pub(crate) manifest_updated_at: Option<String>,
    #[serde(default)]
    pub(crate) manifest_file_size: Option<u64>,
    #[serde(default)]
    pub(crate) image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TicketItem {
    pub(crate) app_id: u32,
    pub(crate) title: String,
    pub(crate) has_app_ticket: bool,
    pub(crate) has_e_ticket: bool,
    pub(crate) extracted_at: u64,
    pub(crate) expires_at: Option<u64>,
    #[serde(default)]
    pub(crate) source_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppState {
    pub(crate) settings: AppSettings,
    pub(crate) packages: Vec<PackageItem>,
    pub(crate) tickets: Vec<TicketItem>,
    pub(crate) install_status: InstallStatus,
    pub(crate) package_sync_supported: bool,
    pub(crate) steam_client: SteamClientStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallStatus {
    pub(crate) installed: bool,
    pub(crate) supported: bool,
    pub(crate) launch_required: bool,
    pub(crate) launched_via_wuhu: bool,
    pub(crate) update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamClientStatus {
    pub(crate) version: Option<String>,
    pub(crate) client_build_date: Option<u64>,
    pub(crate) locked: bool,
    pub(crate) lock_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamSearchItem {
    #[serde(rename = "type", alias = "itemType", default)]
    pub(crate) item_type: String,
    pub(crate) name: String,
    pub(crate) id: u32,
    #[serde(rename = "tiny_image", alias = "tinyImage", default)]
    pub(crate) tiny_image: Option<String>,
    #[serde(default)]
    pub(crate) price: Option<SteamSearchPrice>,
    #[serde(default)]
    pub(crate) platforms: Option<SteamSearchPlatforms>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamSearchPrice {
    #[serde(default)]
    pub(crate) currency: String,
    #[serde(default)]
    pub(crate) initial: u32,
    #[serde(rename = "final")]
    #[serde(default)]
    pub(crate) final_price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamSearchPlatforms {
    pub(crate) windows: Option<bool>,
    pub(crate) mac: Option<bool>,
    pub(crate) linux: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestStatus {
    pub(crate) provider: String,
    pub(crate) app_id: u32,
    pub(crate) game_name: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) available: bool,
    pub(crate) manifest_file_exists: bool,
    pub(crate) update_in_progress: Option<bool>,
    pub(crate) needs_update: Option<bool>,
    pub(crate) file_size: Option<u64>,
    pub(crate) file_modified: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HubcapQuota {
    pub(crate) daily_usage: u64,
    pub(crate) daily_limit: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppRelease {
    pub(crate) version: String,
    pub(crate) name: Option<String>,
    pub(crate) url: Option<String>,
}
